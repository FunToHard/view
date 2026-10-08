use view_core::{
    Description, Dirty, Key, KeyPhase, KeyboardEvent, LogicalRect, Modifiers, Runtime, Shortcut,
};

#[test]
fn test_tab_traversal_and_wraparound() {
    let mut runtime = Runtime::<(), ()>::new(8).unwrap();
    let (window, root) = runtime
        .open_window(
            Description::new("root", (), |_, _, _| Dirty::NONE)
                .layout(LogicalRect::from_xywh(0.0, 0.0, 300.0, 300.0)),
        )
        .unwrap();

    let btn1 = runtime
        .mount(
            root,
            Description::new("btn1", (), |_, _, _| Dirty::NONE)
                .focusable(true)
                .tab_index(1),
        )
        .unwrap();

    let btn2 = runtime
        .mount(
            root,
            Description::new("btn2", (), |_, _, _| Dirty::NONE)
                .focusable(true)
                .tab_index(2),
        )
        .unwrap();

    let mut model = ();
    runtime.flush(&mut model).unwrap();

    // 1. Initial focus_next -> btn1
    assert_eq!(runtime.focus_next(window).unwrap(), Some(btn1));
    assert_eq!(runtime.focused(window).unwrap(), Some(btn1));

    // 2. Next -> btn2
    assert_eq!(runtime.focus_next(window).unwrap(), Some(btn2));
    assert_eq!(runtime.focused(window).unwrap(), Some(btn2));

    // 3. Wraparound -> btn1
    assert_eq!(runtime.focus_next(window).unwrap(), Some(btn1));

    // 4. Backward (Shift+Tab) -> btn2
    assert_eq!(runtime.focus_prev(window).unwrap(), Some(btn2));
}

#[test]
fn test_modal_scope_and_focus_restoration() {
    let mut runtime = Runtime::<(), ()>::new(8).unwrap();
    let (window, root) = runtime
        .open_window(Description::new("root", (), |_, _, _| Dirty::NONE))
        .unwrap();

    let background_field = runtime
        .mount(
            root,
            Description::new("bg", (), |_, _, _| Dirty::NONE).focusable(true),
        )
        .unwrap();

    let modal_dialog = runtime
        .mount(root, Description::new("dialog", (), |_, _, _| Dirty::NONE))
        .unwrap();

    let dialog_btn = runtime
        .mount(
            modal_dialog,
            Description::new("dialog_btn", (), |_, _, _| Dirty::NONE).focusable(true),
        )
        .unwrap();

    let mut model = ();
    runtime.flush(&mut model).unwrap();

    // Focus background field first
    runtime.set_focus(window, Some(background_field)).unwrap();
    assert_eq!(runtime.focused(window).unwrap(), Some(background_field));

    // Open modal dialog
    runtime.set_modal_scope(window, Some(modal_dialog)).unwrap();

    // Tab traversal now confined to modal dialog!
    let next = runtime.focus_next(window).unwrap();
    assert_eq!(next, Some(dialog_btn));

    // Close modal dialog -> focus restores to background_field!
    runtime.set_modal_scope(window, None).unwrap();
    assert_eq!(runtime.focused(window).unwrap(), Some(background_field));
}

#[test]
fn test_shortcut_routing() {
    let mut runtime = Runtime::<(), ()>::new(8).unwrap();
    let (window, root) = runtime
        .open_window(Description::new("root", (), |_, _, _| Dirty::NONE))
        .unwrap();

    let save_btn = runtime
        .mount(root, Description::new("save", (), |_, _, _| Dirty::NONE))
        .unwrap();

    let mut model = ();
    runtime.flush(&mut model).unwrap();

    // Register Ctrl+S -> save_btn
    let ctrl_s = Shortcut::new(
        Key::Character("s".to_string()),
        Modifiers {
            ctrl: true,
            ..Default::default()
        },
    );
    runtime.register_shortcut(window, ctrl_s, save_btn).unwrap();

    let key_event = KeyboardEvent {
        phase: KeyPhase::Down,
        key: Key::Character("s".to_string()),
        modifiers: Modifiers {
            ctrl: true,
            ..Default::default()
        },
        timestamp: 500,
    };
    let target = runtime
        .route_keyboard(window, key_event, &mut model)
        .unwrap();
    assert_eq!(target, Some(save_btn));
}
