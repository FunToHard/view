use view::host::*;
use view::{Description, Dirty, LogicalRect, Runtime, SemanticSnapshot};
struct Sink(usize);
impl HostServices for Sink {
    fn capabilities(&self) -> HostCapabilities {
        HostCapabilities {
            clipboard: ServiceSupport::Unavailable,
            ime: ServiceSupport::Unavailable,
            accessibility: ServiceSupport::Host,
        }
    }
    fn publish_semantics(&mut self, _: &SemanticSnapshot) -> Result<(), HostServiceError> {
        self.0 += 1;
        Ok(())
    }
    fn ime(&mut self, _: bool, _: LogicalRect) -> Result<(), HostServiceError> {
        Err(HostServiceError::Unsupported)
    }
    fn clipboard_read(&mut self) -> Result<String, HostServiceError> {
        Err(HostServiceError::Unsupported)
    }
    fn clipboard_write(&mut self, _: &str) -> Result<(), HostServiceError> {
        Err(HostServiceError::Unsupported)
    }
}
#[test]
fn default_facade_accepts_host_services_without_a_platform_or_gpu_dependency() {
    let mut ui = Runtime::<(), ()>::new(2).unwrap();
    ui.open_window(Description::new("host", (), |_, _, _| Dirty::NONE))
        .unwrap();
    ui.flush(&mut ()).unwrap();
    let mut bridge = HostBridge::new(Sink(0));
    assert!(bridge.publish(ui.snapshot()).unwrap());
    assert!(!bridge.publish(ui.snapshot()).unwrap());
    assert_eq!(bridge.services().0, 1);
}
