#[test]
fn editor_storefront_and_ide_share_public_contracts() {
    let mut app = view_application_proofs::Proofs::new().unwrap();
    app.verify().unwrap();
    let before = app.document.source.clone();
    app.ui.close_window(app.detached).unwrap();
    assert_eq!(app.document.source, before);
    assert!(app.preview.begin("closed").is_err());
}

#[test]
fn captured_gizmo_commits_one_undo_and_stale_replay_does_not_edit() {
    use view::{viewport::*, *};
    use view_application_proofs::*;
    let mut app = Proofs::new().unwrap();
    app.dispatch(Action::Select(7)).unwrap();
    let base = app.document.revision;
    let mut drag = DragSession::new(app.gizmo, 7, base, app.document.x);
    app.ui.capture_pointer(app.editor, app.gizmo).unwrap();
    let action = app
        .ui
        .route_pointer(
            app.editor,
            PointerEvent {
                phase: PointerPhase::Up,
                position: LogicalPoint::new(440., 180.),
                button: Some(PointerButton::Primary),
                modifiers: Modifiers::default(),
                timestamp: 1,
            },
            &mut app.document,
        )
        .unwrap();
    for a in &action {
        if let Some(commit) = drag.route(a, base, |p| (p.x - 6.) / 80.) {
            app.commit_drag(commit).unwrap();
        }
    }
    assert_eq!(app.document.x, 0.5);
    app.dispatch(Action::Move { base, x: 99. }).unwrap();
    assert_eq!(app.document.x, 0.5);
    app.dispatch(Action::Undo).unwrap();
    assert_eq!(app.document.x, 0.);
    app.dispatch(Action::Undo).unwrap();
    assert_eq!(app.document.x, 0.);
}

#[test]
fn navigation_remote_refresh_and_detach_preserve_app_documents() {
    use view_application_proofs::*;
    let mut app = Proofs::new().unwrap();
    app.dispatch(Action::Navigate(Page::Downloads)).unwrap();
    assert!(app.ui.snapshot().nodes.iter().any(|n| n.key == "download"));
    app.dispatch(Action::Navigate(Page::Store)).unwrap();
    app.ui.close_window(app.editor).unwrap();
    app.dispatch(Action::Select(7)).unwrap();
    assert_eq!(app.document.selected, Some(7));
}
