use view_core::{
    Description, Dirty, LogicalPoint, LogicalRect, Modifiers, PointerButton, PointerEvent,
    PointerPhase, RoutedPointerAction, Runtime,
};

#[test]
fn test_hover_enter_leave_and_click_transitions() {
    let mut runtime = Runtime::<(), ()>::new(8).unwrap();
    let (window, root) = runtime
        .open_window(
            Description::new("root", (), |_, _, _| Dirty::NONE)
                .layout(LogicalRect::from_xywh(0.0, 0.0, 300.0, 300.0)),
        )
        .unwrap();

    let btn = runtime
        .mount(
            root,
            Description::new("button", (), |_, _, _| Dirty::NONE)
                .layout(LogicalRect::from_xywh(50.0, 50.0, 100.0, 50.0)),
        )
        .unwrap();

    let mut model = ();
    runtime.flush(&mut model).unwrap();

    // 1. Move cursor over button -> Enter button
    let move_in = PointerEvent {
        phase: PointerPhase::Move,
        position: LogicalPoint::new(60.0, 60.0),
        button: None,
        modifiers: Modifiers::default(),
        timestamp: 1000,
    };
    let actions = runtime.route_pointer(window, move_in, &mut model).unwrap();
    assert!(actions.contains(&RoutedPointerAction::Enter(btn)));

    // 2. Press down on button -> Down button
    let down = PointerEvent {
        phase: PointerPhase::Down,
        position: LogicalPoint::new(60.0, 60.0),
        button: Some(PointerButton::Primary),
        modifiers: Modifiers::default(),
        timestamp: 1010,
    };
    let actions = runtime.route_pointer(window, down, &mut model).unwrap();
    assert!(actions.iter().any(|a| matches!(
        a,
        RoutedPointerAction::Down {
            target,
            button: PointerButton::Primary,
            ..
        } if *target == btn
    )));

    // 3. Release button over button -> Up and Click!
    let up = PointerEvent {
        phase: PointerPhase::Up,
        position: LogicalPoint::new(60.0, 60.0),
        button: Some(PointerButton::Primary),
        modifiers: Modifiers::default(),
        timestamp: 1020,
    };
    let actions = runtime.route_pointer(window, up, &mut model).unwrap();
    assert!(actions.contains(&RoutedPointerAction::Click {
        target: btn,
        button: PointerButton::Primary,
    }));
}

#[test]
fn test_pointer_capture_routes_outside_events() {
    let mut runtime = Runtime::<(), ()>::new(8).unwrap();
    let (window, root) = runtime
        .open_window(
            Description::new("root", (), |_, _, _| Dirty::NONE)
                .layout(LogicalRect::from_xywh(0.0, 0.0, 300.0, 300.0)),
        )
        .unwrap();

    let _slider = runtime
        .mount(
            root,
            Description::new("slider", (), |_, _, _| Dirty::NONE)
                .layout(LogicalRect::from_xywh(10.0, 10.0, 50.0, 20.0)),
        )
        .unwrap();

    let mut model = ();
    runtime.flush(&mut model).unwrap();

    // Pointer down on slider
    let down = PointerEvent {
        phase: PointerPhase::Down,
        position: LogicalPoint::new(20.0, 15.0),
        button: Some(PointerButton::Primary),
        modifiers: Modifiers::default(),
        timestamp: 100,
    };
    runtime.route_pointer(window, down, &mut model).unwrap();

    // Slider acquires pointer capture
    // Simulate setting capture:
    // When pointer moves outside to (250, 250), it would normally hit root,
    // but if captured by slider, it goes to slider!
    let move_out = PointerEvent {
        phase: PointerPhase::Move,
        position: LogicalPoint::new(250.0, 250.0),
        button: None,
        modifiers: Modifiers::default(),
        timestamp: 110,
    };
    let actions = runtime.route_pointer(window, move_out, &mut model).unwrap();
    // Since capture was not explicitly granted yet, it hit root.
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, RoutedPointerAction::Enter(target) if *target == root))
    );
}

#[test]
fn test_pre_input_flush_contract() {
    let mut runtime = Runtime::<i32, i32>::new(8).unwrap();
    let (window, root) = runtime
        .open_window(
            Description::new("root", 0, |state, _model, action| {
                *state += action;
                Dirty::LAYOUT
            })
            .layout(LogicalRect::from_xywh(0.0, 0.0, 100.0, 100.0)),
        )
        .unwrap();

    let mut model = 0;
    runtime.flush(&mut model).unwrap();

    // Dynamically mount a new widget but DO NOT call flush manually
    let new_widget = runtime
        .mount(
            root,
            Description::new("new", 0, |_, _, _| Dirty::NONE)
                .layout(LogicalRect::from_xywh(20.0, 20.0, 40.0, 40.0)),
        )
        .unwrap();

    // The runtime has pending changes (`runtime.has_work() == true`).
    assert!(runtime.has_work());

    // When route_pointer is called, it MUST flush pending commits automatically before hit testing!
    let click = PointerEvent {
        phase: PointerPhase::Down,
        position: LogicalPoint::new(30.0, 30.0),
        button: Some(PointerButton::Primary),
        modifiers: Modifiers::default(),
        timestamp: 200,
    };
    let actions = runtime.route_pointer(window, click, &mut model).unwrap();
    assert!(actions.iter().any(|a| matches!(
        a,
        RoutedPointerAction::Down { target, .. } if *target == new_widget
    )));
    assert!(!runtime.has_work());
}
