#[cfg(test)]
mod tests {
    use view_core::*;
    fn desc(key: &str, x: f32, y: f32) -> Description<(), ()> {
        Description::new(key, (), |_, _, _| Dirty::NONE)
            .layout(LogicalRect::from_xywh(x, y, 20., 20.))
            .focusable(true)
            .name(key)
    }
    fn ui() -> (Runtime<(), ()>, WindowId, NodeId) {
        let mut r = Runtime::new(8).unwrap();
        let (w, n) = r.open_window(desc("root", 0., 0.)).unwrap();
        (r, w, n)
    }
    #[test]
    fn disjoint_clips_reject_all_points() {
        let clip = ClipChain::new(LogicalRect::from_xywh(0., 0., 10., 10.)).intersect(
            Some(LogicalRect::from_xywh(20., 20., 10., 10.)),
            &Transform2D::IDENTITY,
        );
        assert!(!clip.contains(LogicalPoint::new(25., 25.)));
    }
    #[test]
    fn parent_origin_offsets_child_geometry() {
        let (mut r, w, root) = ui();
        let p = r.mount(root, desc("p", 100., 100.)).unwrap();
        let c = r.mount(p, desc("c", 5., 5.)).unwrap();
        r.flush(&mut ()).unwrap();
        assert_eq!(
            r.global_rect(c).unwrap().origin,
            LogicalPoint::new(105., 105.)
        );
        assert_eq!(
            r.hit_test(w, LogicalPoint::new(106., 106.))
                .unwrap()
                .unwrap()
                .target,
            c
        );
    }
    #[test]
    fn scroll_moves_committed_content() {
        let (mut r, w, root) = ui();
        let c = r.mount(root, desc("child", 0., 100.)).unwrap();
        r.set_scroll_state(
            root,
            ScrollState::new(LogicalSize::new(100., 300.), LogicalSize::new(100., 100.)),
        )
        .unwrap();
        r.flush(&mut ()).unwrap();
        r.scroll_by(root, LogicalPoint::new(0., 100.)).unwrap();
        r.flush(&mut ()).unwrap();
        assert_eq!(
            r.hit_test(w, LogicalPoint::new(5., 5.))
                .unwrap()
                .unwrap()
                .target,
            c
        );
    }
    #[test]
    fn modal_blocks_outside_shortcut() {
        let (mut r, w, root) = ui();
        let modal = r.mount(root, desc("modal", 30., 30.)).unwrap();
        r.register_shortcut(
            w,
            Shortcut::new(Key::Named(NamedKey::Enter), Modifiers::default()),
            root,
        )
        .unwrap();
        r.set_modal_scope(w, Some(modal)).unwrap();
        r.set_focus(w, Some(modal)).unwrap();
        let result = r
            .route_keyboard(
                w,
                KeyboardEvent {
                    phase: KeyPhase::Down,
                    key: Key::Named(NamedKey::Enter),
                    modifiers: Modifiers::default(),
                    timestamp: 0,
                },
                &mut (),
            )
            .unwrap();
        assert_ne!(result, Some(root));
    }
    #[test]
    fn semantics_do_not_change_before_commit() {
        let (mut r, w, root) = ui();
        r.flush(&mut ()).unwrap();
        let before = r.semantic_snapshot(w).unwrap();
        r.mount(root, desc("new", 5., 5.)).unwrap();
        let after = r.semantic_snapshot(w).unwrap();
        assert_eq!(after.revision, before.revision);
        assert_eq!(after.nodes.len(), before.nodes.len());
    }
    #[test]
    fn hidden_ancestor_excludes_focusable_child() {
        let (mut r, w, root) = ui();
        let p = r.mount(root, desc("hidden", 0., 0.)).unwrap();
        let c = r.mount(p, desc("child", 0., 0.)).unwrap();
        r.set_visibility(p, Visibility::Hidden).unwrap();
        r.set_focus(w, Some(root)).unwrap();
        assert_ne!(r.focus_next(w).unwrap(), Some(c));
    }
    #[test]
    fn transform_chain_applies_child_before_parent() {
        let chain = TransformChain::new(Transform2D::translation(100., 0.))
            .then(&Transform2D::scale(2., 2.));
        assert_eq!(
            chain.transform().transform_point(LogicalPoint::new(5., 0.)),
            LogicalPoint::new(110., 0.)
        );
    }
}
