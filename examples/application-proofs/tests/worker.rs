use std::time::{Duration, Instant};
use view_application_proofs::{Proofs, service::FakeService};
#[test]
fn worker_reordering_does_not_revert_catalog_and_idle_has_no_executor_work() {
    let mut app = Proofs::new().unwrap();
    let service = FakeService::default();
    let old = app.search("old").unwrap();
    let new = app.search("new").unwrap();
    service.start(old, Duration::from_millis(30)).unwrap();
    service.start(new, Duration::ZERO).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut accepted = 0;
    while service.pending() {
        if let Some((r, items)) = service.receive() {
            accepted += usize::from(app.reply(&r, items).unwrap());
        }
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert_eq!(accepted, 1);
    assert!(app.results.value().unwrap()[0].name.starts_with("new"));
}
