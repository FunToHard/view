use std::time::Duration;
use view_core::{Description, Dirty, Runtime};
use view_testing::{Harness, Input, ScheduleFailure, ServiceFixture};

fn fixture(capacity: usize) -> (Harness<Vec<i32>, i32>, view_core::NodeId) {
    let mut runtime = Runtime::new(capacity).unwrap();
    let (_, root) = runtime
        .open_window(Description::new(
            "root",
            (),
            |_, model: &mut Vec<i32>, action| {
                model.push(action);
                Dirty::NONE
            },
        ))
        .unwrap();
    (Harness::new(runtime, vec![], 8), root)
}

#[test]
fn virtual_clock_orders_deadlines_and_preserves_equal_time_fifo_under_backpressure() {
    let (mut harness, root) = fixture(1);
    assert!(
        harness
            .schedule(Duration::from_secs(2), Input::Action(root, 3))
            .is_ok()
    );
    assert!(
        harness
            .schedule(Duration::from_secs(1), Input::Action(root, 1))
            .is_ok()
    );
    assert!(
        harness
            .schedule(Duration::from_secs(1), Input::Action(root, 2))
            .is_ok()
    );
    harness.step().unwrap();
    assert!(harness.model().is_empty());
    harness.advance(Duration::from_secs(1)).unwrap();
    assert!(harness.model().is_empty());
    let step = harness.step().unwrap();
    assert_eq!(step.delivered, 1);
    assert!(step.backpressure);
    assert_eq!(harness.model(), &vec![1]);
    harness.step().unwrap();
    assert_eq!(harness.model(), &vec![1, 2]);
    harness.advance(Duration::from_secs(1)).unwrap();
    harness.step().unwrap();
    assert_eq!(harness.model(), &vec![1, 2, 3]);
    assert_eq!(harness.pending_inputs(), 0);
    let revision = harness.runtime().snapshot().revision;
    harness.step().unwrap();
    assert_eq!(harness.runtime().snapshot().revision, revision);
}

#[test]
fn controlled_completion_reordering_uses_runtime_request_validation() {
    let (mut harness, root) = fixture(4);
    let old = harness.runtime_mut().start_request(root, 1, || {}).unwrap();
    let latest = harness.runtime_mut().start_request(root, 1, || {}).unwrap();
    assert!(
        harness
            .schedule(Duration::ZERO, Input::Completion(latest, 2))
            .is_ok()
    );
    assert!(
        harness
            .schedule(Duration::from_secs(1), Input::Completion(old, 1))
            .is_ok()
    );
    harness.step().unwrap();
    harness.advance(Duration::from_secs(1)).unwrap();
    assert_eq!(
        harness.step().unwrap().rejected,
        vec![view_core::CoreError::InactiveRequest]
    );
    assert_eq!(harness.model(), &vec![2]);
}

#[test]
fn overflow_and_fixture_capacity_fail_without_losing_values() {
    let (mut harness, root) = fixture(1);
    harness.advance(Duration::MAX).unwrap();
    assert_eq!(
        harness.advance(Duration::from_nanos(1)),
        Err(ScheduleFailure::TimeExhausted)
    );
    assert_eq!(harness.now(), Duration::MAX);
    let error = harness
        .schedule(Duration::from_nanos(1), Input::Action(root, 9))
        .err()
        .unwrap();
    assert_eq!(error.reason, ScheduleFailure::TimeExhausted);
    assert!(matches!(error.input, Input::Action(_, 9)));
    let mut service = ServiceFixture::new(1);
    service.supply(Ok::<_, &'static str>(42)).unwrap();
    assert_eq!(service.supply(Err("offline")), Err(Err("offline")));
    assert_eq!(service.take(), Some(Ok(42)));
    service.supply(Err("offline")).unwrap();
    assert_eq!(service.take(), Some(Err("offline")));
    assert_eq!(service.take(), None);
}
