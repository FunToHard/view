use crate::*;
use std::{
    io::{Read, Write},
    process::{Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::{self, Receiver, SyncSender, TryRecvError},
    },
    thread,
    time::Duration,
};
#[derive(Clone, Debug, Default)]
pub struct ProcessReceipt {
    pub exited: bool,
    pub exit_code: Option<i32>,
    pub workers: usize,
    pub failure: Option<String>,
}
struct Shared {
    cancel: AtomicBool,
    workers: AtomicUsize,
    receipt: Mutex<ProcessReceipt>,
    diagnostics: Mutex<Vec<u8>>,
}
/// Dedicated pipes, bounded queues and owned child. No shell, input injection or global environment changes.
pub struct Process {
    out: Option<SyncSender<Vec<u8>>>,
    input: Receiver<Result<GuestEnvelope>>,
    shared: Arc<Shared>,
}
struct Worker(Arc<Shared>);
impl Drop for Worker {
    fn drop(&mut self) {
        self.0.workers.fetch_sub(1, Ordering::AcqRel);
    }
}
impl Process {
    pub fn spawn(manifest: &Manifest, capacity: usize) -> Result<Self> {
        manifest.validate()?;
        if capacity == 0 || capacity > 256 {
            return Err(Error::Limit);
        }
        let mut command = Command::new(&manifest.executable);
        command
            .args(&manifest.arguments)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command.spawn()?;
        let mut stdin = child.stdin.take().ok_or(Error::Disconnected)?;
        let mut stdout = child.stdout.take().ok_or(Error::Disconnected)?;
        let mut stderr = child.stderr.take().ok_or(Error::Disconnected)?;
        let shared = Arc::new(Shared {
            cancel: AtomicBool::new(false),
            workers: AtomicUsize::new(4),
            receipt: Mutex::new(ProcessReceipt::default()),
            diagnostics: Mutex::new(vec![]),
        });
        let (tx, input) = mpsc::sync_channel(capacity);
        let (out, rx) = mpsc::sync_channel::<Vec<u8>>(capacity);
        let s = shared.clone();
        thread::spawn(move || {
            let _worker = Worker(s.clone());
            loop {
                let result = read_frame(&mut stdout);
                let failed = result.is_err();
                if tx.try_send(result).is_err() {
                    s.receipt.lock().expect("receipt").failure = Some("inbound queue limit".into());
                    s.cancel.store(true, Ordering::Release);
                    break;
                }
                if failed {
                    break;
                }
            }
        });
        let s = shared.clone();
        thread::spawn(move || {
            let _worker = Worker(s.clone());
            while !s.cancel.load(Ordering::Acquire) {
                match rx.recv_timeout(Duration::from_millis(10)) {
                    Ok(bytes) => {
                        if stdin
                            .write_all(&(bytes.len() as u32).to_le_bytes())
                            .and_then(|_| stdin.write_all(&bytes))
                            .and_then(|_| stdin.flush())
                            .is_err()
                        {
                            s.cancel.store(true, Ordering::Release);
                            break;
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => (),
                    Err(_) => break,
                }
            }
        });
        let s = shared.clone();
        thread::spawn(move || {
            let _worker = Worker(s.clone());
            let mut bytes = [0; 1024];
            while let Ok(n) = stderr.read(&mut bytes) {
                if n == 0 {
                    break;
                }
                let mut log = s.diagnostics.lock().expect("diagnostics");
                let remaining = 8192usize.saturating_sub(log.len());
                log.extend_from_slice(&bytes[..n.min(remaining)]);
            }
        });
        let s = shared.clone();
        thread::spawn(move || {
            let _worker = Worker(s.clone());
            loop {
                if s.cancel.load(Ordering::Acquire) {
                    let _ = child.kill();
                }
                match child.try_wait() {
                    Ok(Some(status)) => {
                        let mut r = s.receipt.lock().expect("receipt");
                        r.exited = true;
                        r.exit_code = status.code();
                        break;
                    }
                    Err(e) => {
                        s.receipt.lock().expect("receipt").failure = Some(e.to_string());
                        let _ = child.kill();
                        let _ = child.wait();
                        break;
                    }
                    Ok(None) => thread::sleep(Duration::from_millis(5)),
                }
            }
            s.cancel.store(true, Ordering::Release);
        });
        Ok(Self {
            out: Some(out),
            input,
            shared,
        })
    }
    pub fn send(&self, message: &HostMessage) -> Result<()> {
        if self.shared.cancel.load(Ordering::Acquire) {
            return Err(Error::Disconnected);
        }
        self.out
            .as_ref()
            .ok_or(Error::Disconnected)?
            .try_send(encode(message)?)
            .map_err(|_| Error::Limit)
    }
    pub fn receive(&self) -> Result<Option<GuestEnvelope>> {
        match self.input.try_recv() {
            Ok(value) => value.map(Some),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(Error::Disconnected),
        }
    }
    pub fn cancel(&mut self) {
        self.shared.cancel.store(true, Ordering::Release);
        self.out.take();
    }
    pub fn receipt(&self) -> ProcessReceipt {
        let mut r = self.shared.receipt.lock().expect("receipt").clone();
        r.workers = self.shared.workers.load(Ordering::Acquire);
        r
    }
    pub fn diagnostics(&self) -> String {
        String::from_utf8_lossy(&self.shared.diagnostics.lock().expect("diagnostics")).into_owned()
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        self.cancel();
    }
}
