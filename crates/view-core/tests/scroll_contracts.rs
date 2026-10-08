use view_core::{
    Description, Dirty, Key, LogicalPoint, LogicalRect, LogicalSize, NamedKey, Runtime,
    ScrollChaining, ScrollState, VirtualScroll,
};

#[test]
fn test_scroll_state_clamping_and_keyboard() {
    let mut state = ScrollState::new(
        LogicalSize::new(100.0, 500.0), // Content height 500
        LogicalSize::new(100.0, 200.0), // Viewport height 200
    );
    // max_offset is 500 - 200 = 300
    assert_eq!(state.max_offset(), LogicalPoint::new(0.0, 300.0));

    // Scroll down 50
    let (consumed, unconsumed) = state.scroll_by(LogicalPoint::new(0.0, 50.0));
    assert_eq!(consumed, LogicalPoint::new(0.0, 50.0));
    assert_eq!(unconsumed, LogicalPoint::ZERO);
    assert_eq!(state.offset, LogicalPoint::new(0.0, 50.0));

    // Keyboard scroll Down (line step 20)
    assert!(state.handle_key(&Key::Named(NamedKey::ArrowDown), 20.0));
    assert_eq!(state.offset, LogicalPoint::new(0.0, 70.0));

    // Keyboard scroll End -> jumps to max_offset (300)
    assert!(state.handle_key(&Key::Named(NamedKey::End), 20.0));
    assert_eq!(state.offset, LogicalPoint::new(0.0, 300.0));

    // Further scroll down produces unconsumed delta
    let (consumed_end, unconsumed_end) = state.scroll_by(LogicalPoint::new(0.0, 50.0));
    assert_eq!(consumed_end, LogicalPoint::ZERO);
    assert_eq!(unconsumed_end, LogicalPoint::new(0.0, 50.0));
}

#[test]
fn test_nested_scroll_chaining() {
    let mut runtime = Runtime::<(), ()>::new(8).unwrap();
    let (_window, root) = runtime
        .open_window(Description::new("root", (), |_, _, _| Dirty::NONE))
        .unwrap();

    let parent_scroll = runtime
        .mount(root, Description::new("parent", (), |_, _, _| Dirty::NONE))
        .unwrap();
    let mut parent_state = ScrollState::new(
        LogicalSize::new(200.0, 1000.0),
        LogicalSize::new(200.0, 400.0),
    );
    parent_state.chaining = ScrollChaining::Clamp;
    runtime
        .set_scroll_state(parent_scroll, parent_state)
        .unwrap();

    let child_scroll = runtime
        .mount(
            parent_scroll,
            Description::new("child", (), |_, _, _| Dirty::NONE),
        )
        .unwrap();
    let mut child_state = ScrollState::new(
        LogicalSize::new(200.0, 300.0),
        LogicalSize::new(200.0, 200.0),
    );
    child_state.chaining = ScrollChaining::Chain; // Unconsumed propagates up
    runtime.set_scroll_state(child_scroll, child_state).unwrap();

    let mut model = ();
    runtime.flush(&mut model).unwrap();

    // Child max offset is 100. Scroll by 150:
    // 100 consumed by child, remaining 50 propagated to parent!
    let total_consumed = runtime
        .scroll_by(child_scroll, LogicalPoint::new(0.0, 150.0))
        .unwrap();
    assert_eq!(total_consumed, LogicalPoint::new(0.0, 150.0));

    assert_eq!(
        runtime.scroll_state(child_scroll).unwrap().unwrap().offset,
        LogicalPoint::new(0.0, 100.0)
    );
    assert_eq!(
        runtime.scroll_state(parent_scroll).unwrap().unwrap().offset,
        LogicalPoint::new(0.0, 50.0)
    );
}

#[test]
fn test_reveal_target_contract() {
    let mut state = ScrollState::new(
        LogicalSize::new(200.0, 1000.0),
        LogicalSize::new(200.0, 200.0),
    );
    assert_eq!(state.offset, LogicalPoint::ZERO);

    // Target rect is at y = 350..390 (currently below viewport [0..200])
    let target = LogicalRect::from_xywh(0.0, 350.0, 100.0, 40.0);
    state.reveal_rect(target);

    // New viewport should contain y: 350..390. Bottom of viewport becomes 390 -> offset.y = 390 - 200 = 190
    assert_eq!(state.offset.y, 190.0);
}

#[test]
fn test_virtual_content_realization() {
    // 10,000 items, each 30px high
    let virtual_scroll = VirtualScroll::new(10_000, 30.0);
    assert_eq!(
        virtual_scroll.total_content_size(100.0),
        LogicalSize::new(100.0, 300_000.0)
    );

    // Viewport at offset 300, height 120 -> visible items are ~ index 10 to 14
    let range = virtual_scroll.visible_range(300.0, 120.0, 2);
    // with 2 buffer items:
    assert_eq!(range, 8..17);
    assert!(range.len() < 20); // Bounded realization!
}
