//! Run with `cargo run -p view-testing --example headless --locked`.
use std::time::Duration;
use view_core::{Description, Dirty, Runtime, Structure};
use view_testing::{Harness, Input};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut runtime = Runtime::new(8)?;
    let (window, root) = runtime.open_window(
        Description::new("inspector", (), |_, model: &mut i32, delta: i32| {
            *model += delta;
            Dirty::BUILD
        })
        .structure(Structure::Immediate),
    )?;
    runtime.set_region_builder(root, |_, _| {
        vec![Description::new(
            "selection",
            String::from("persistent local state"),
            |_, _, _: i32| Dirty::NONE,
        )]
    })?;
    let mut harness = Harness::new(runtime, 0, 8);
    harness.step()?;
    assert!(
        harness
            .schedule(Duration::from_millis(10), Input::Action(root, 5))
            .is_ok()
    );
    harness
        .advance(Duration::from_millis(10))
        .expect("bounded fixture time");
    let receipt = harness.step()?;
    assert_eq!(*harness.model(), 5);
    assert_eq!(receipt.flush.dispatched, 1);
    let before = harness.runtime().counters();
    for _ in 0..100 {
        harness.step()?;
    }
    let after = harness.runtime().counters();
    assert_eq!(before.builds, after.builds);
    assert_eq!(before.commits, after.commits);
    assert!(!harness.runtime().demand(window)?.ui);
    println!(
        "headless: model={}, builds={}, commits={}, idle_polls={}",
        harness.model(),
        after.builds,
        after.commits,
        after.idle_polls
    );
    Ok(())
}
