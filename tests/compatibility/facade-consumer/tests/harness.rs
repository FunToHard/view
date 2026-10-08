#![cfg(feature = "harness")]

use std::time::Duration;
use view::{Description, Dirty, Runtime};
use view_testing::{Harness, Input};

#[test]
fn public_facade_and_optional_harness_share_one_runtime_contract() {
    let mut runtime = Runtime::new(2).unwrap();
    let (_, root) = runtime
        .open_window(Description::new(
            "root",
            (),
            |_, model: &mut i32, action: i32| {
                *model += action;
                Dirty::NONE
            },
        ))
        .unwrap();
    let mut harness = Harness::new(runtime, 0, 2);
    assert!(
        harness
            .schedule(Duration::ZERO, Input::Action(root, 7))
            .is_ok()
    );
    let result = harness.step().unwrap();
    assert_eq!(result.flush.dispatched, 1);
    assert_eq!(*harness.model(), 7);
    assert_eq!(harness.runtime().snapshot().nodes.len(), 1);
}

#[cfg(feature = "text")]
#[test]
fn text_blink_uses_harness_time_without_scheduling_idle_work() {
    use view::text::{EditorConfig, PlainEditor};
    let runtime = Runtime::<(), ()>::new(2).unwrap();
    let mut harness = Harness::new(runtime, (), 2);
    let mut editor = PlainEditor::new("", EditorConfig::default()).unwrap();
    editor.set_focused(true, harness.now().as_millis() as u64);
    assert!(editor.caret_visible(harness.now().as_millis() as u64));
    harness.advance(Duration::from_millis(500)).unwrap();
    assert!(!editor.caret_visible(harness.now().as_millis() as u64));
    assert!(!harness.runtime().has_work());
    harness.advance(Duration::from_millis(500)).unwrap();
    assert!(editor.caret_visible(harness.now().as_millis() as u64));
}
