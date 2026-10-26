//! Disposable application proof. This executable owns its document schema and
//! process policy; neither is imposed by the diagnostic library.
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::Path,
    sync::{Arc, Mutex},
};
use view_diagnostics::{
    Diagnostics, Identity, Limits, PanicReporter, Reporter, Revisions, chain_panic_hook,
};

#[path = "diagnostic-fixture/checkpoint.rs"]
mod checkpoint;

struct OpenReport(Mutex<File>);
impl Reporter for OpenReport {
    fn report(&self, bytes: &[u8]) -> io::Result<()> {
        let mut file = self
            .0
            .try_lock()
            .map_err(|_| io::Error::other("report file unavailable"))?;
        file.write_all(bytes)?;
        file.sync_data()
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.len() != 4 || args[1] != "--disposable-fixture" {
        return Err("only run under the disposable-process contract supervisor".into());
    }
    let mode = args[2].to_str().ok_or("invalid mode")?;
    let directory = Path::new(&args[3]);
    if !directory.is_absolute() || !directory.is_dir() {
        return Err("existing absolute fixture directory required".into());
    }
    if mode == "restore" || mode == "restore-accept" {
        let recovery = checkpoint::recover(directory)?;
        if mode == "restore-accept" {
            let candidate = recovery
                .candidate
                .as_ref()
                .ok_or("no valid recovery candidate")?;
            // Explicit application restore policy, on a fresh healthy process.
            fs::write(directory.join("restored.txt"), &candidate.text)?;
        }
        serde_json::to_writer(io::stdout(), &recovery)?;
        return Ok(());
    }
    if !matches!(mode, "panic" | "abort" | "native-fault" | "hang") {
        return Err("unknown fixture mode".into());
    }

    // Healthy operation protects document state before any hook is installed.
    checkpoint::save(directory, 1, "First completed edit")?;
    checkpoint::save(directory, 2, "Unsaved second edit survives process failure")?;
    // Simulate a third write interrupted before publication. Recovery ignores it.
    fs::write(
        directory.join("checkpoint-00000000000000000003.partial"),
        b"{\"schema\":1,",
    )?;

    let mut diagnostic = Diagnostics::new(
        Identity {
            application: "diagnostic-fixture".into(),
            build: "fixture-build-2026-10-25".into(),
            symbols: symbol_identity()?,
            framework: env!("CARGO_PKG_VERSION").into(),
            platform: std::env::consts::OS.into(),
            backend: "headless".into(),
        },
        Limits::default(),
    );
    diagnostic.capabilities(["prepared-panic-report", "app-checkpoint-v1"]);
    diagnostic.revisions(Revisions {
        document: 2,
        committed: 7,
        presented: 6,
        device_generation: 1,
    });
    diagnostic.record(10, "document.checkpoint.complete");
    let prepared = diagnostic.prepare(|_, value| Some(value.into()))?;
    fs::write(directory.join("healthy-context.json"), prepared.bytes())?;
    let report = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(directory.join("panic-report.json"))?;
    let reporter = PanicReporter::new(prepared, Arc::new(OpenReport(Mutex::new(report))));

    // The application, rather than a mounted widget/library import, decides to
    // replace the global hook and passes its existing policy for composition.
    let host_marker = directory.join("host-hook.txt");
    std::panic::set_hook(Box::new(move |_| {
        let _ = fs::write(&host_marker, b"host hook preserved");
    }));
    let previous = std::panic::take_hook();
    std::panic::set_hook(chain_panic_hook(previous, reporter));
    suppress_fault_ui()?;
    let mut ready = File::create(directory.join("ready"))?;
    ready.write_all(b"healthy checkpoint and reporting policy ready")?;
    ready.sync_all()?;
    match mode {
        "panic" => panic!("private panic payload must not enter prepared report"),
        "abort" => std::process::abort(),
        "native-fault" => native_fault(),
        "hang" => loop {
            std::thread::park();
        },
        _ => unreachable!(),
    }
}

fn symbol_identity() -> io::Result<String> {
    #[cfg(windows)]
    {
        Ok(
            view_diagnostics::read_pdb_identity(&std::env::current_exe()?)?
                .map(|identity| identity.symbol_key())
                .unwrap_or_else(|| "no-rsds-record".into()),
        )
    }
    #[cfg(not(windows))]
    {
        Ok("native-symbol-identity-unqualified".into())
    }
}

#[cfg(windows)]
fn suppress_fault_ui() -> Result<(), Box<dyn std::error::Error>> {
    use windows::Win32::System::ErrorReporting::{
        WER_FAULT_REPORTING, WER_FAULT_REPORTING_NO_UI, WerSetFlags,
    };
    // SAFETY: flags affect only this explicitly disposable process. No pointers,
    // callbacks or exception handlers are installed, and no registry is changed.
    unsafe {
        WerSetFlags(WER_FAULT_REPORTING(WER_FAULT_REPORTING_NO_UI))
            .map_err(|error| io::Error::other(error.to_string()))?;
    }
    Ok(())
}
#[cfg(not(windows))]
fn suppress_fault_ui() -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}

#[cfg(windows)]
fn native_fault() -> ! {
    // SAFETY: deliberate noncontinuable application-defined native SEH exception
    // in the supervised disposable executable. No pointers or Rust UB are used;
    // no handler resumes this Rust frame. The supervisor owns timeout/cleanup.
    // This exercises native exception dispatch, not every access violation/driver fault.
    unsafe {
        windows::Win32::System::Diagnostics::Debug::RaiseException(0xE042_5649, 1, None);
    }
    std::process::exit(99)
}
#[cfg(not(windows))]
fn native_fault() -> ! {
    std::process::exit(78)
}
