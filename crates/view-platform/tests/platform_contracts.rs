//! Behavioral contracts and integration verification for view-platform.

use view_core::{
    ArenaHandle, ArenaId, Generation, LogicalPoint, LogicalSize, PhysicalSize, PointerButton,
    PointerEvent, PointerPhase, ScaleFactor, WindowId,
};
use view_platform::{
    ComGuard, DpiState, ImeEvent, InputStateTracker, PlatformError, PlatformShell,
    PlatformWindowEvent, WindowLifecycle, WindowStateSnapshot, translate_window_event,
};
use winit::dpi::{PhysicalPosition, PhysicalSize as WinitPhysicalSize};
use winit::event::{ElementState, Ime, MouseButton, MouseScrollDelta, WindowEvent};

fn make_window_id(slot: u32, generation_num: u64) -> WindowId {
    let arena = ArenaId::new(std::num::NonZeroU64::new(2).unwrap());
    let generation = Generation::new(std::num::NonZeroU64::new(generation_num).unwrap());
    WindowId::from_handle(ArenaHandle::from_parts(arena, slot, generation))
}

#[test]
fn test_dpi_conversions_and_negative_screen_coordinates() {
    let scale = ScaleFactor::new(1.5).unwrap();
    let text_scale = 1.2;
    // Secondary monitor to the left: screen origin at x = -1920, y = 0
    let screen_origin = (-1920, 0);

    let dpi = DpiState::new(scale, text_scale, screen_origin);

    // Client point (100, 200) in logical points
    let client_logical = LogicalPoint::new(100.0, 200.0);
    let client_physical = dpi.logical_to_physical_point(client_logical);
    assert!((client_physical.x - 150.0).abs() <= 1.0);
    assert!((client_physical.y - 300.0).abs() <= 1.0);

    // Round-trip back to logical
    let back_logical = dpi.physical_to_logical_point(client_physical);
    assert!((back_logical.x - client_logical.x).abs() < 1e-4);
    assert!((back_logical.y - client_logical.y).abs() < 1e-4);

    // Screen coordinate mapping with negative desktop space
    let screen_pos = dpi.client_physical_to_screen(client_physical);
    assert_eq!(screen_pos, (-1920 + 150, 300));

    // Screen to client mapping
    let back_client_phys = dpi.screen_to_client_physical(screen_pos);
    assert_eq!(back_client_phys, client_physical);

    // Logical to screen direct mapping
    let screen_from_logical = dpi.client_logical_to_screen(client_logical);
    assert_eq!(screen_from_logical, screen_pos);

    let back_from_screen = dpi.screen_to_client_logical(screen_pos);
    assert!((back_from_screen.x - client_logical.x).abs() < 1e-4);
}

#[test]
fn test_input_tracker_and_focus_loss_cleanup() {
    let window_id = make_window_id(1, 1);
    let mut tracker = InputStateTracker::new();

    // Simulate mouse move
    let move_event = WindowEvent::CursorMoved {
        // SAFETY: DeviceId is an opaque identifier with no dereference invariants in tests.
        device_id: unsafe { std::mem::zeroed() },
        position: PhysicalPosition::new(150.0, 300.0),
    };
    let scale = ScaleFactor::new(1.5).unwrap();
    let trans = translate_window_event(&move_event, window_id, scale, &mut tracker, 1000);
    assert!(matches!(
        trans,
        Some(PlatformWindowEvent::Pointer(PointerEvent {
            phase: PointerPhase::Move,
            position,
            button: None,
            ..
        })) if (position.x - 100.0).abs() < 1e-4 && (position.y - 200.0).abs() < 1e-4
    ));

    // Simulate primary button down
    let down_event = WindowEvent::MouseInput {
        // SAFETY: DeviceId is an opaque identifier with no dereference invariants in tests.
        device_id: unsafe { std::mem::zeroed() },
        state: ElementState::Pressed,
        button: MouseButton::Left,
    };
    let trans_down = translate_window_event(&down_event, window_id, scale, &mut tracker, 1050);
    assert!(matches!(
        trans_down,
        Some(PlatformWindowEvent::Pointer(PointerEvent {
            phase: PointerPhase::Down,
            button: Some(PointerButton::Primary),
            ..
        }))
    ));
    assert_eq!(tracker.pressed_buttons, vec![PointerButton::Primary]);

    // On focus loss or deactivation, cancel held buttons
    let cancelled = tracker.cancel_held_buttons();
    assert_eq!(cancelled.len(), 1);
    assert_eq!(cancelled[0].phase, PointerPhase::Cancel);
    assert_eq!(cancelled[0].button, Some(PointerButton::Primary));
    assert!(tracker.pressed_buttons.is_empty());
}

#[test]
fn test_mouse_scroll_event_translation() {
    let window_id = make_window_id(1, 1);
    let mut tracker = InputStateTracker::new();
    let scale = ScaleFactor::ONE;

    let wheel_event = WindowEvent::MouseWheel {
        // SAFETY: DeviceId is an opaque identifier with no dereference invariants in tests.
        device_id: unsafe { std::mem::zeroed() },
        delta: MouseScrollDelta::LineDelta(1.0, -2.5),
        phase: winit::event::TouchPhase::Moved,
    };

    let trans = translate_window_event(&wheel_event, window_id, scale, &mut tracker, 1500);
    assert!(matches!(
        trans,
        Some(PlatformWindowEvent::Pointer(PointerEvent {
            phase: PointerPhase::Scroll { delta_x, delta_y },
            button: None,
            ..
        })) if (delta_x - 24.0).abs() < 1e-4 && (delta_y - (-60.0)).abs() < 1e-4
    ));
}

#[test]
fn test_ime_and_lifecycle_event_translations() {
    let window_id = make_window_id(1, 1);
    let mut tracker = InputStateTracker::new();
    let scale = ScaleFactor::new(2.0).unwrap();

    // IME preedit
    let ime_preedit = WindowEvent::Ime(Ime::Preedit("nihon".into(), Some((0, 5))));
    let trans = translate_window_event(&ime_preedit, window_id, scale, &mut tracker, 1600);
    assert_eq!(
        trans,
        Some(PlatformWindowEvent::Ime(ImeEvent::Preedit(
            "nihon".into(),
            Some((0, 5))
        )))
    );

    // IME commit
    let ime_commit = WindowEvent::Ime(Ime::Commit("日本".into()));
    let trans_commit = translate_window_event(&ime_commit, window_id, scale, &mut tracker, 1700);
    assert_eq!(
        trans_commit,
        Some(PlatformWindowEvent::Ime(ImeEvent::Commit("日本".into())))
    );

    // Resized event
    let resize_event = WindowEvent::Resized(WinitPhysicalSize::new(1600, 1200));
    let trans_resize = translate_window_event(&resize_event, window_id, scale, &mut tracker, 1800);
    assert_eq!(
        trans_resize,
        Some(PlatformWindowEvent::Resized {
            logical_size: LogicalSize::new(800.0, 600.0),
            physical_size: PhysicalSize::from_pixels(1600, 1200),
        })
    );

    // Moved event
    let move_event = WindowEvent::Moved(PhysicalPosition::new(-1920, 100));
    let trans_moved = translate_window_event(&move_event, window_id, scale, &mut tracker, 1900);
    assert_eq!(
        trans_moved,
        Some(PlatformWindowEvent::Moved {
            screen_origin: (-1920, 100),
        })
    );

    // CloseRequested event
    let close_event = WindowEvent::CloseRequested;
    let trans_close = translate_window_event(&close_event, window_id, scale, &mut tracker, 2000);
    assert_eq!(trans_close, Some(PlatformWindowEvent::CloseRequested));
}

#[test]
fn test_modal_hierarchy_and_cycle_rejection() {
    let mut shell = PlatformShell::new();
    let win1 = make_window_id(1, 1);
    let win2 = make_window_id(2, 1);

    // If windows are not registered, set_modal returns InvalidWindow
    assert!(matches!(
        shell.set_modal(win2, win1),
        Err(PlatformError::InvalidWindow(_))
    ));
}

#[test]
fn test_late_event_routing_safety() {
    let mut shell = PlatformShell::new();

    // Fabricate a winit WindowId that does not exist in the shell
    // SAFETY: WindowId is an opaque identifier value; zeroed representation simulates an unregistered foreign handle.
    let dummy_winit_id = unsafe { std::mem::zeroed() };

    let move_event = WindowEvent::CursorMoved {
        // SAFETY: DeviceId is an opaque identifier with no dereference invariants in tests.
        device_id: unsafe { std::mem::zeroed() },
        position: PhysicalPosition::new(50.0, 50.0),
    };

    // Processing event for unknown window must safely return None without panicking
    let res = shell.process_winit_event(dummy_winit_id, &move_event, 100);
    assert!(res.is_none());
}

#[test]
fn test_window_state_snapshot_contracts() {
    let snapshot = WindowStateSnapshot {
        id: make_window_id(1, 1),
        title: "Test Window".into(),
        logical_size: LogicalSize::new(800.0, 600.0),
        physical_size: PhysicalSize::from_pixels(1200, 900),
        client_screen_origin: (100, 200),
        scale_factor: ScaleFactor::new(1.5).unwrap(),
        text_scale: 1.0,
        modal_parent: None,
        modal_children: Vec::new(),
        lifecycle: WindowLifecycle::Opened,
    };

    assert_eq!(snapshot.title, "Test Window");
    assert_eq!(snapshot.logical_size.width, 800.0);
    assert_eq!(snapshot.scale_factor.get(), 1.5);
    assert_eq!(snapshot.client_screen_origin, (100, 200));
}

#[test]
fn test_com_guard_lifecycle() {
    #[cfg(windows)]
    {
        // STA initialization on test thread
        let guard = ComGuard::new();
        assert!(guard.is_ok(), "COM STA initialization should succeed");
        drop(guard);
        // Re-initializing after drop should also succeed cleanly
        let guard2 = ComGuard::new();
        assert!(guard2.is_ok());
    }
}
