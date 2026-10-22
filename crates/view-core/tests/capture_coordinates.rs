use view_core::*;
#[test]
fn capture_outside_hit_bounds_uses_translated_scaled_scrolled_owner_coordinates() {
    let mut ui = Runtime::<(), ()>::new(8).unwrap();
    let (window, root) = ui
        .open_window(
            Description::new("root", (), |_, _, _| Dirty::NONE)
                .layout(LogicalRect::from_xywh(10., 20., 500., 500.)),
        )
        .unwrap();
    let parent = ui
        .mount(
            root,
            Description::new("parent", (), |_, _, _| Dirty::NONE)
                .layout(LogicalRect::from_xywh(50., 30., 200., 200.))
                .transform(Transform2D::scale(2., 2.)),
        )
        .unwrap();
    let child = ui
        .mount(
            parent,
            Description::new("child", (), |_, _, _| Dirty::NONE)
                .layout(LogicalRect::from_xywh(100., 50., 10., 10.)),
        )
        .unwrap();
    let mut scroll = ScrollState::new(LogicalSize::new(400., 400.), LogicalSize::new(200., 200.));
    scroll.scroll_to(LogicalPoint::new(5., 10.));
    ui.set_scroll_state(parent, scroll).unwrap();
    ui.flush(&mut ()).unwrap();
    ui.capture_pointer(window, child).unwrap();
    let actions = ui
        .route_pointer(
            window,
            PointerEvent {
                phase: PointerPhase::Move,
                position: LogicalPoint::new(300., 190.),
                button: Some(PointerButton::Primary),
                modifiers: Modifiers::default(),
                timestamp: 0,
            },
            &mut (),
        )
        .unwrap();
    assert!(actions.iter().any(|a|matches!(a,RoutedPointerAction::Move{target,local} if *target==child && *local==LogicalPoint::new(25.,30.))));
}
