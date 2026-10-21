use view::{Dirty, controls::TextField};
use view_application_proofs::{Proofs, service::SourceService};
#[test]
fn editing_cancels_stale_preview_and_worker_updates_preserve_source_lifetime() {
    let mut app = Proofs::new().unwrap();
    let field = app.editing_node(app.detached).unwrap();
    let old = app.preview.begin("source").unwrap();
    app.ui
        .update(field, Dirty::PAINT, |f: &mut TextField| {
            f.insert_text("new ", 0)
        })
        .unwrap()
        .unwrap();
    app.edited(app.detached).unwrap();
    assert!(!app.source_reply(&old, "stale".into()).unwrap());
    assert!(app.document.source.contains("new "));
    let worker = SourceService::default();
    let request = app.preview.begin("source").unwrap();
    worker.start(request, app.document.source.clone()).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while worker.pending() {
        if let Some((r, s)) = worker.receive() {
            assert!(app.source_reply(&r, s).unwrap());
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
    let document = app.document.source.clone();
    app.ui.close_window(app.detached).unwrap();
    assert_eq!(app.document.source, document);
}
