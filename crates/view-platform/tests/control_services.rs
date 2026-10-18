use view_core::{Description, Dirty, Runtime, ScaleFactor};
use view_platform::{InputStateTracker, PlatformWindowEvent, translate_window_event};
use winit::event::{Ime, WindowEvent};

#[test]
fn preedit_state_and_file_drop_are_translated_without_effects() {
    let mut runtime: Runtime<(), ()> = Runtime::new(4).unwrap();
    let (window, _) = runtime
        .open_window(Description::new("root", (), |_, _, _| Dirty::NONE))
        .unwrap();
    let mut tracker = InputStateTracker::new();
    translate_window_event(
        &WindowEvent::Ime(Ime::Preedit("日本".into(), Some((3, 6)))),
        window,
        ScaleFactor::ONE,
        &mut tracker,
        1,
    );
    assert!(tracker.composing);
    translate_window_event(
        &WindowEvent::Ime(Ime::Commit("日本".into())),
        window,
        ScaleFactor::ONE,
        &mut tracker,
        2,
    );
    assert!(!tracker.composing);
    let path = std::path::PathBuf::from("does-not-need-to-exist.txt");
    let event = translate_window_event(
        &WindowEvent::DroppedFile(path.clone()),
        window,
        ScaleFactor::ONE,
        &mut tracker,
        3,
    );
    assert_eq!(event, Some(PlatformWindowEvent::FileDropped(path)));
    assert_eq!(
        translate_window_event(
            &WindowEvent::HoveredFileCancelled,
            window,
            ScaleFactor::ONE,
            &mut tracker,
            4
        ),
        Some(PlatformWindowEvent::FileHoverCancelled)
    );
}

#[cfg(feature = "desktop-services")]
#[test]
fn shell_targets_are_explicit_and_reject_commandlike_inputs() {
    use view_platform::desktop::ShellTarget;
    assert!(
        ShellTarget::Https("https://example.invalid/asset".into())
            .validate()
            .is_ok()
    );
    for value in [
        "javascript:alert(1)",
        "file:///tmp/a",
        "https://",
        "https://example.invalid\0other",
        "https://example.invalid extra",
    ] {
        assert!(ShellTarget::Https(value.into()).validate().is_err());
    }
    assert!(ShellTarget::File("relative.txt".into()).validate().is_err());
    assert!(
        ShellTarget::File(std::env::current_dir().unwrap().join("not-opened.txt"))
            .validate()
            .is_ok()
    );
}
