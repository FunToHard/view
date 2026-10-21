//! Opt-in desktop ownership with watchdog cleanup. No driver is opened by default.
use crate::{Code, Error};
use std::{
    collections::BTreeSet,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Held {
    Key(u16),
    Button(u8),
    Unicode(u16),
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Move { x: i32, y: i32 },
    Down(Held),
    Up(Held),
    Wheel { horizontal: bool, delta: i32 },
}
/// Driver reports insertion only. Applications separately report observed routing/results.
pub trait Driver: Send + 'static {
    fn check_target(&self) -> Result<(), Error>;
    fn already_down(&self, input: Held) -> bool;
    fn insert(&mut self, event: Event) -> Result<(), Error>;
}
static OWNER: AtomicBool = AtomicBool::new(false);
type Work = (Event, mpsc::SyncSender<Result<(), Error>>);
pub struct DesktopSession {
    sender: mpsc::SyncSender<Work>,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<Result<(), Error>>>,
}
impl DesktopSession {
    /// Explicit selection is required. Deadline is capped at five minutes.
    pub fn start<D: Driver>(driver: D, selected: bool, timeout: Duration) -> Result<Self, Error> {
        if !selected {
            return Err(Error::new(
                Code::Unauthorized,
                "desktop session not explicitly selected",
            ));
        }
        if timeout.is_zero() || timeout > Duration::from_secs(300) {
            return Err(Error::new(Code::Invalid, "desktop timeout"));
        }
        driver.check_target()?;
        if OWNER.swap(true, Ordering::AcqRel) {
            return Err(Error::new(Code::Busy, "desktop already owned"));
        }
        let (sender, receiver) = mpsc::sync_channel::<Work>(8);
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let deadline = Instant::now() + timeout;
        let worker = thread::spawn(move || {
            struct ReleaseOwner;
            impl Drop for ReleaseOwner {
                fn drop(&mut self) {
                    OWNER.store(false, Ordering::Release);
                }
            }
            let _owner = ReleaseOwner;
            let mut driver = driver;
            let mut held = BTreeSet::new();
            let outcome = (|| {
                loop {
                    if Instant::now() >= deadline {
                        return Err(Error::new(Code::Timeout, "desktop deadline"));
                    }
                    if flag.load(Ordering::Acquire) {
                        return Ok(());
                    }
                    driver.check_target()?;
                    match receiver.recv_timeout(Duration::from_millis(20)) {
                        Ok((event, reply)) => {
                            let result = (|| {
                                if Instant::now() >= deadline {
                                    return Err(Error::new(Code::Timeout, "desktop deadline"));
                                }
                                if flag.load(Ordering::Acquire) {
                                    return Err(Error::new(Code::Cancelled, "desktop stopped"));
                                }
                                driver.check_target()?;
                                match event {
                                    Event::Down(input)
                                        if !held.contains(&input) && driver.already_down(input) =>
                                    {
                                        return Err(Error::new(
                                            Code::Busy,
                                            "input already held outside runner",
                                        ));
                                    }
                                    Event::Down(input) if held.contains(&input) => {
                                        return Err(Error::new(Code::Invalid, "duplicate down"));
                                    }
                                    Event::Up(input) if !held.contains(&input) => {
                                        return Err(Error::new(
                                            Code::Invalid,
                                            "runner does not own input",
                                        ));
                                    }
                                    _ => {}
                                }
                                driver.insert(event)?;
                                match event {
                                    Event::Down(input) => {
                                        held.insert(input);
                                    }
                                    Event::Up(input) => {
                                        held.remove(&input);
                                    }
                                    _ => {}
                                }
                                Ok(())
                            })();
                            let failed = result.is_err();
                            let _ = reply.send(result);
                            if failed {
                                return Err(Error::new(Code::Cancelled, "input group failed"));
                            }
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                        Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
                    }
                }
            })();
            // Cleanup deliberately bypasses foreground checks, and only releases our set.
            let mut cleanup_error = None;
            for input in held.into_iter().rev() {
                if let Err(e) = driver.insert(Event::Up(input)) {
                    cleanup_error = Some(e);
                }
            }
            if let Some(e) = cleanup_error {
                Err(e)
            } else {
                outcome
            }
        });
        Ok(Self {
            sender,
            stop,
            worker: Some(worker),
        })
    }
    pub fn stop_handle(&self) -> Arc<AtomicBool> {
        self.stop.clone()
    }
    pub fn send(&self, event: Event) -> Result<(), Error> {
        let (tx, rx) = mpsc::sync_channel(1);
        self.sender
            .try_send((event, tx))
            .map_err(|_| Error::new(Code::Busy, "desktop stopped or queue full"))?;
        rx.recv_timeout(Duration::from_secs(1))
            .map_err(|_| Error::new(Code::UnknownOutcome, "input insertion receipt timed out"))?
    }
    pub fn finish(mut self) -> Result<(), Error> {
        self.stop.store(true, Ordering::Release);
        self.worker
            .take()
            .expect("worker")
            .join()
            .map_err(|_| Error::new(Code::Host, "desktop worker panicked"))?
    }
}
impl Drop for DesktopSession {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenMap {
    pub origin: (i32, i32),
    pub scale: f32,
    pub revision: u64,
}
impl ScreenMap {
    pub fn convert(
        self,
        point: view_core::LogicalPoint,
        revision: u64,
    ) -> Result<(i32, i32), Error> {
        if revision != self.revision {
            return Err(Error::new(
                Code::Stale,
                "layout changed before screen conversion",
            ));
        }
        if !point.is_valid() || !self.scale.is_finite() || self.scale <= 0. {
            return Err(Error::new(Code::Invalid, "DPI/point"));
        }
        let x = self.origin.0 as f64 + point.x as f64 * self.scale as f64;
        let y = self.origin.1 as f64 + point.y as f64 * self.scale as f64;
        if x < i32::MIN as f64 || x > i32::MAX as f64 || y < i32::MIN as f64 || y > i32::MAX as f64
        {
            return Err(Error::new(Code::Invalid, "screen overflow"));
        }
        Ok((x.round() as i32, y.round() as i32))
    }
}
