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
