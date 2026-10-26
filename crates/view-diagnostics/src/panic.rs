use crate::PreparedReport;
use std::{
    io,
    panic::PanicHookInfo,
    sync::{Arc, RwLock},
};

/// The host owns storage, consent and failure behavior. `report` must be short,
/// must not panic, allocate heavily, acquire application locks, or upload data.
/// An OS fault/abort need not call it. Errors are intentionally not panicked.
pub trait Reporter: Send + Sync + 'static {
    fn report(&self, prepared: &[u8]) -> io::Result<()>;
}

/// Read-only panic-time snapshot with nonblocking access. A concurrent healthy
/// update or poisoned lock causes an omitted report, never a blocking wait.
#[derive(Clone)]
pub struct PanicReporter {
    prepared: Arc<RwLock<PreparedReport>>,
    reporter: Arc<dyn Reporter>,
}
impl PanicReporter {
    pub fn new(prepared: PreparedReport, reporter: Arc<dyn Reporter>) -> Self {
        Self {
            prepared: Arc::new(RwLock::new(prepared)),
            reporter,
        }
    }
    /// Healthy-operation publication only. This never installs a global hook.
    pub fn update(&self, prepared: PreparedReport) -> io::Result<()> {
        *self
            .prepared
            .write()
            .map_err(|_| io::Error::other("poisoned diagnostic snapshot"))? = prepared;
        Ok(())
    }
    /// Best effort, no panic payload collection. False means reporting failed or
    /// was skipped; the host must not interpret this as successful recovery.
    pub fn emit(&self) -> bool {
        self.prepared
            .try_read()
            .is_ok_and(|prepared| self.reporter.report(prepared.bytes()).is_ok())
    }
}

pub type PanicHook = Box<dyn Fn(&PanicHookInfo<'_>) + Send + Sync + 'static>;

/// Compose a hook without touching process globals. The host explicitly calls
/// `take_hook`/`set_hook` during serialized startup and owns lifetime/restoration.
/// The supplied prior hook always runs after best-effort prepared reporting.
/// No unwind is caught and no UI tree is resumed here.
pub fn chain_panic_hook(previous: PanicHook, reporter: PanicReporter) -> PanicHook {
    Box::new(move |info| {
        reporter.emit();
        previous(info);
    })
}
