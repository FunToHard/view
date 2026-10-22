#![cfg(feature = "markdown")]
#[test]
fn independent_consumer_runs_declared_profile_adapter() {
    let mut preview =
        view_markdown::MarkdownPreview::new("# Heading\nHello 🦀", Default::default()).unwrap();
    view_markdown::conformance::check_selection(&mut preview, "Heading\nHello 🦀").unwrap();
}
use view::{authoring::*, *};
#[test]
fn separate_control_consumes_only_public_compatible_facade() {
    let preview = view_markdown::MarkdownPreview::new(
        "# Community\n[Open](https://example.test)",
        Default::default(),
    )
    .unwrap();
    let mut ui = Runtime::<(), ()>::new(4).unwrap();
    let (_, root) = ui
        .open_window(
            Description::new("root", (), |_, _, _| Dirty::NONE).structure(Structure::Declarative),
        )
        .unwrap();
    reconcile(
        &mut ui,
        root,
        (
            Text::new("title", "Independent consumer"),
            preview.element("preview", |_, _| Dirty::NONE),
        ),
    )
    .unwrap();
    ui.flush(&mut ()).unwrap();
    assert!(
        ui.snapshot()
            .semantic_snapshot()
            .nodes
            .iter()
            .any(|n| n.role == Role::Custom("link".into()))
    );
}
