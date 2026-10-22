use view::{authoring::*, *};
use view_markdown::*;
#[test]
fn fences_empty_blocks_and_malformed_links_preserve_plain_text() {
    let p = MarkdownPreview::new(
        "````rust\n\na\n```\n````\n# After\n[Open](https://example.test)",
        Limits::default(),
    )
    .unwrap();
    assert_eq!(p.plain_text(), "\na\n```\nAfter\nOpen");
    assert_eq!(p.blocks()[0].plain_range, 0..0);
    assert_eq!(p.blocks()[1].plain_range, 1..2);
    assert_eq!(p.blocks()[3].kind, Kind::Heading(1));
    assert!(p.activate(4, p.revision()).is_ok());
    for source in ["[a](https://x) and [b](https://y)", "[a](https://x y)"] {
        let p = MarkdownPreview::new(source, Limits::default()).unwrap();
        assert_eq!(p.blocks()[0].kind, Kind::Paragraph);
        assert_eq!(p.plain_text(), source);
    }
}
#[test]
fn syntax_selection_links_images_and_atomic_updates_follow_declared_profile() {
    let mut p = MarkdownPreview::new("# Title\nhello 🦀\n[Open](https://example.test)\n![Alt](https://example.test/a.bmp)\n<script>x</script>",Limits::default()).unwrap();
    assert_eq!(p.blocks()[0].kind, Kind::Heading(1));
    assert!(matches!(p.blocks()[3].kind, Kind::Image(_)));
    p.select(0..p.plain_text().len()).unwrap();
    assert!(p.copy().contains("Title\nhello 🦀\nOpen\nAlt"));
    let crab = p.plain_text().find('🦀').unwrap();
    assert!(p.select(crab..crab + 1).is_err());
    let revision = p.revision();
    assert_eq!(
        p.activate(2, revision).unwrap().target,
        "https://example.test"
    );
    p.update("replacement", revision).unwrap();
    assert!(p.copy().is_empty());
    assert_eq!(p.activate(0, revision), Err(Error::Stale));
    let old = p.plain_text().to_string();
    assert!(p.update(&"x".repeat(300000), p.revision()).is_err());
    assert_eq!(p.plain_text(), old);
    let unsafe_link =
        MarkdownPreview::new("[Run](javascript:alert(1))", Limits::default()).unwrap();
    assert_eq!(unsafe_link.blocks()[0].kind, Kind::Paragraph);
}
#[test]
fn public_composition_has_heading_link_semantics_and_app_owned_activation() {
    let p =
        MarkdownPreview::new("# Heading\n[Link](https://example.test)", Limits::default()).unwrap();
    let mut ui = Runtime::<usize, usize>::new(8).unwrap();
    let (_, root) = ui
        .open_window(
            Description::new("root", (), |_, _, _| Dirty::NONE).structure(Structure::Declarative),
        )
        .unwrap();
    reconcile(
        &mut ui,
        root,
        (p.element("markdown", |m, a| {
            *m += a;
            Dirty::PAINT
        }),),
    )
    .unwrap();
    let mut count = 0;
    ui.flush(&mut count).unwrap();
    let link = ui
        .snapshot()
        .nodes
        .iter()
        .find(|n| n.name.as_deref() == Some("Link"))
        .unwrap()
        .id;
    ui.enqueue(link, 1).unwrap();
    ui.flush(&mut count).unwrap();
    assert_eq!(count, 1);
    assert!(
        ui.snapshot()
            .semantic_snapshot()
            .nodes
            .iter()
            .any(|n| n.role == Role::Custom("heading".into()))
    );
}
