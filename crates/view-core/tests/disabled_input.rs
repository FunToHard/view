use view_core::*;
#[test]
fn disabled_subtree_blocks_focus_activation_and_click_through_but_keeps_semantics() {
    let mut rt = Runtime::<(), ()>::new(8).unwrap();
    let (w, root) = rt
        .open_window(Description::new("root", (), |_, _, _| Dirty::NONE))
        .unwrap();
    rt.set_window_size(w, LogicalSize::new(200., 100.)).unwrap();
    let button = || {
        Description::new("button", (), |_, _, _| Dirty::NONE)
            .role(Role::Button)
            .focusable(true)
            .layout(LogicalRect::from_xywh(0., 0., 100., 50.))
    };
    let back = rt.mount(root, button()).unwrap();
    let front = rt
        .mount(
            root,
            Description::new("disabled", (), |_, _, _| Dirty::NONE)
                .layout(LogicalRect::from_xywh(0., 0., 100., 50.)),
        )
        .unwrap();
    let child = rt.mount(front, button()).unwrap();
    rt.flush(&mut ()).unwrap();
    rt.set_focus(w, Some(child)).unwrap();
    rt.capture_pointer(w, child).unwrap();
    rt.set_enabled(front, false).unwrap();
    rt.flush(&mut ()).unwrap();
    assert_eq!(rt.focused(w).unwrap(), None);
    assert_eq!(rt.focus_next(w).unwrap(), Some(back));
    rt.flush(&mut ()).unwrap();
    let node = rt.snapshot().nodes.iter().find(|n| n.id == child).unwrap();
    assert!(!node.enabled);
    assert!(node.semantic.as_ref().unwrap().disabled);
    assert_eq!(
        rt.hit_test(w, LogicalPoint::new(20., 20.))
            .unwrap()
            .unwrap()
            .target,
        child
    );
    let routed = rt
        .route_pointer(
            w,
            PointerEvent {
                phase: PointerPhase::Down,
                position: LogicalPoint::new(20., 20.),
                button: Some(PointerButton::Primary),
                modifiers: Modifiers::default(),
                timestamp: 1,
            },
            &mut (),
        )
        .unwrap();
    assert!(routed.contains(&RoutedPointerAction::Cancel(child)));
    assert!(!routed.iter().any(|a| matches!(
        a,
        RoutedPointerAction::Down { .. } | RoutedPointerAction::Click { .. }
    )));
}
