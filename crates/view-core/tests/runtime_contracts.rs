use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};
use view_core::{
    Arena, CoreError, Description, Dirty, NodeId, ResourceKind, Revision, Runtime, Structure,
    Trace, Visibility,
};

type Ui = Runtime<Vec<i32>, i32>;
fn item(key: &str, value: i32) -> Description<Vec<i32>, i32> {
    Description::new(key, value, |state, model: &mut Vec<i32>, action| {
        *state += action;
        model.push(action);
        Dirty::BUILD
    })
}
fn setup(structure: Structure, capacity: usize) -> (Ui, view_core::WindowId, NodeId) {
    let mut ui = Ui::new(capacity).unwrap();
    let (window, root) = ui
        .open_window(item("root", 0).structure(structure))
        .unwrap();
    (ui, window, root)
}

#[test]
fn arena_reuse_rejects_stale_foreign_and_fabricated_handles() {
    let mut arena = Arena::new().unwrap();
    let old = arena.insert(10).unwrap();
    assert_eq!(arena.remove(old), Ok(10));
    let current = arena.insert(20).unwrap();
    assert_eq!(old.index(), current.index());
    assert_ne!(old.generation(), current.generation());
    assert!(matches!(arena.get(old), Err(CoreError::StaleHandle { .. })));
    assert!(matches!(
        arena.remove(old),
        Err(CoreError::StaleHandle { .. })
    ));
    let foreign = Arena::<i32>::new().unwrap().insert(30).unwrap();
    assert!(matches!(
        arena.get(foreign),
        Err(CoreError::WrongArena { .. })
    ));
    let fake = view_core::ArenaHandle::from_parts(arena.id(), u32::MAX, current.generation());
    assert!(matches!(
        arena.get(fake),
        Err(CoreError::StaleHandle { .. })
    ));
    assert_eq!(arena.get(current), Ok(&20));
}

#[test]
fn seeded_arena_lifecycle_matches_independent_live_map() {
    // Fixed reproducible LCG seed; expected occupancy comes from a separate map.
    let mut seed = 0x5eed_cafe_u64;
    let mut arena = Arena::new().unwrap();
    let mut live = HashMap::new();
    let mut issued = Vec::new();
    for value in 0..2048 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        if seed & 3 == 0 && !issued.is_empty() {
            let id = issued[(seed as usize >> 8) % issued.len()];
            if let Some(expected) = live.remove(&id) {
                assert_eq!(arena.remove(id), Ok(expected));
            } else {
                assert!(arena.get(id).is_err());
            }
        } else {
            let id = arena.insert(value).unwrap();
            live.insert(id, value);
            issued.push(id);
        }
    }
    assert_eq!(arena.iter().count(), live.len());
    for id in issued {
        assert_eq!(arena.get(id).ok(), live.get(&id));
    }
}

#[test]
fn invalid_parent_and_structural_owner_operations_leave_tree_unchanged() {
    let (mut ui, _, root) = setup(Structure::Retained, 8);
    let child = ui.mount(root, item("child", 1)).unwrap();
    ui.unmount(child).unwrap();
    assert!(ui.mount(child, item("orphan", 2)).is_err());
    assert_eq!(
        ui.reconcile(root, vec![]),
        Err(CoreError::StructuralOwnership)
    );
    let (_, _, other_root) = setup(Structure::Retained, 8);
    assert!(matches!(
        ui.mount(other_root, item("foreign", 3)),
        Err(CoreError::WrongArena { .. })
    ));
    ui.flush(&mut vec![]).unwrap();
    assert_eq!(ui.snapshot().nodes.len(), 1);
    assert!(ui.snapshot().nodes[0].children.is_empty());
}

#[test]
fn visibility_preserves_state_resources_and_unmount_revokes_subtree() {
    let (mut ui, window, root) = setup(Structure::Retained, 8);
    let child = ui.mount(root, item("child", 7)).unwrap();
    let grandchild = ui.mount(child, item("grandchild", 9)).unwrap();
    let cancellations = Rc::new(Cell::new(0));
    let mut tokens = Vec::new();
    for (owner, kind) in [
        (child, ResourceKind::Subscription),
        (child, ResourceKind::Callback),
        (grandchild, ResourceKind::Task),
    ] {
        let count = cancellations.clone();
        tokens.push(
            ui.own(owner, kind, move || count.set(count.get() + 1))
                .unwrap(),
        );
    }
    for visibility in [
        Visibility::Hidden,
        Visibility::Clipped,
        Visibility::Suspended,
        Visibility::Visible,
    ] {
        ui.set_visibility(child, visibility).unwrap();
        ui.flush(&mut vec![]).unwrap();
        assert_eq!(ui.state::<i32>(child), Ok(&7));
        assert_eq!(cancellations.get(), 0);
    }
    ui.enqueue(grandchild, 100).unwrap();
    ui.unmount(child).unwrap();
    assert_eq!(cancellations.get(), 3);
    assert!(tokens.iter().all(|token| token.cancellation().is_revoked()));
    let mut model = vec![];
    assert_eq!(ui.flush(&mut model).unwrap().rejected.len(), 1);
    assert!(model.is_empty());
    ui.close_window(window).unwrap();
    assert!(ui.has_work());
    ui.flush(&mut model).unwrap();
    assert!(ui.snapshot().nodes.is_empty());
    assert!(!ui.has_work());
}

#[test]
fn keyed_reorder_retains_state_descendants_and_replaces_handlers() {
    let (mut ui, _, root) = setup(Structure::Declarative, 8);
    let children = ui
        .reconcile(root, vec![item("a", 10), item("b", 20)])
        .unwrap();
    let retained = ui.mount(children[0], item("retained", 30)).unwrap();
    ui.update::<i32, _>(children[0], Dirty::PAINT, |value| *value = 99)
        .unwrap();
    let replacement = Description::new("a", 0_i32, |state, model: &mut Vec<i32>, action| {
        *state += action * 2;
        model.push(action * 2);
        Dirty::NONE
    });
    let reordered = ui.reconcile(root, vec![item("b", 0), replacement]).unwrap();
    assert_eq!(reordered, vec![children[1], children[0]]);
    assert_eq!(ui.state::<i32>(children[0]), Ok(&99));
    assert_eq!(ui.state::<i32>(retained), Ok(&30));
    ui.enqueue(children[0], 2).unwrap();
    let mut model = vec![];
    ui.flush(&mut model).unwrap();
    assert_eq!(model, vec![4]);
    assert_eq!(ui.state::<i32>(children[0]), Ok(&103));
    assert_eq!(ui.unmount(children[0]), Err(CoreError::StructuralOwnership));
}

#[test]
fn duplicates_are_atomic_and_type_changes_dispose_old_state() {
    let (mut ui, _, root) = setup(Structure::Declarative, 8);
    let child = ui.reconcile(root, vec![item("a", 42)]).unwrap()[0];
    ui.flush(&mut vec![]).unwrap();
    let before = ui.snapshot().clone();
    assert_eq!(
        ui.reconcile(root, vec![item("a", 1), item("a", 2)]),
        Err(CoreError::DuplicateKey)
    );
    assert_eq!(ui.snapshot(), &before);
    assert_eq!(ui.state::<i32>(child), Ok(&42));
    let token = ui.start_request(child, 0, || {}).unwrap();
    let next = ui
        .reconcile(
            root,
            vec![Description::new(
                "a",
                String::from("new"),
                |_, _, _: i32| Dirty::NONE,
            )],
        )
        .unwrap()[0];
    assert_ne!(next, child);
    assert!(ui.state::<i32>(child).is_err());
    assert_eq!(ui.state::<String>(next).unwrap(), "new");
    assert!(token.cancellation().is_revoked());
    assert_eq!(ui.state::<i32>(next), Err(CoreError::StateTypeMismatch));
}

#[test]
fn actions_execute_once_and_measurement_observes_only_committed_state() {
    let (mut ui, window, root) = setup(Structure::Retained, 8);
    let mut model = vec![];
    ui.flush(&mut model).unwrap();
    let old = ui.snapshot().clone();
    let a = ui.enqueue(root, 1).unwrap().sequence;
    let b = ui.enqueue(root, 2).unwrap().sequence;
    assert!(b > a);
    for _ in 0..10 {
        assert_eq!(ui.measure(|snapshot| snapshot.clone()), old);
    }
    assert!(model.is_empty());
    let receipt = ui.flush(&mut model).unwrap();
    assert_eq!(receipt.dispatched, 2);
    assert_eq!(model, vec![1, 2]);
    assert_eq!(ui.state::<i32>(root), Ok(&3));
    assert_eq!(ui.presented(window), Ok(Revision::INITIAL));
    let committed = receipt.committed.unwrap();
    ui.acknowledge_presentation(window, committed).unwrap();
    assert_eq!(
        ui.acknowledge_presentation(window, Revision::INITIAL),
        Err(CoreError::InvalidPresentation)
    );
    assert_eq!(
        ui.acknowledge_presentation(window, committed.checked_next().unwrap()),
        Err(CoreError::InvalidPresentation)
    );
    assert_eq!(ui.flush(&mut model).unwrap().dispatched, 0);
    assert_eq!(model, vec![1, 2]);
}

#[test]
fn clean_regions_preserve_retained_mounts_and_consume_ordered_responses_once() {
    let (mut ui, window, root) = setup(Structure::Immediate, 8);
    let observed = Rc::new(RefCell::new(Vec::new()));
    let captured = observed.clone();
    ui.set_region_builder(root, move |_, responses| {
        captured.borrow_mut().push(responses.to_vec());
        vec![item("mount", 10)]
    })
    .unwrap();
    let mut model = vec![];
    ui.flush(&mut model).unwrap();
    let mount = ui.snapshot().nodes[0].children[0];
    let child = ui.mount(mount, item("local", 20)).unwrap();
    ui.flush(&mut model).unwrap();
    let before = ui.counters();
    for _ in 0..100 {
        assert_eq!(ui.flush(&mut model).unwrap().committed, None);
    }
    assert_eq!(ui.counters().builds, before.builds);
    assert_eq!(ui.counters().commits, before.commits);
    assert_eq!(ui.state::<i32>(child), Ok(&20));
    assert!(!ui.demand(window).unwrap().ui);
    let first = ui.enqueue(root, 1).unwrap().sequence;
    let second = ui.enqueue(root, 2).unwrap().sequence;
    ui.flush(&mut model).unwrap();
    assert_eq!(*observed.borrow(), vec![vec![], vec![first, second]]);
    assert_eq!(ui.state::<i32>(child), Ok(&20));
    ui.invalidate(root, Dirty::BUILD).unwrap();
    ui.flush(&mut model).unwrap();
    assert!(observed.borrow().last().unwrap().is_empty());
    assert_eq!(model, vec![1, 2]);
}

#[test]
fn suspended_ancestor_delays_build_without_disposing_children() {
    let (mut ui, window, root) = setup(Structure::Retained, 8);
    let region = ui
        .mount(root, item("region", 0).structure(Structure::Immediate))
        .unwrap();
    ui.set_region_builder(region, |_, _| vec![item("child", 5)])
        .unwrap();
    ui.set_visibility(root, Visibility::Suspended).unwrap();
    ui.flush(&mut vec![]).unwrap();
    assert_eq!(ui.counters().builds, 0);
    assert!(!ui.demand(window).unwrap().ui);
    ui.set_visibility(root, Visibility::Visible).unwrap();
    ui.flush(&mut vec![]).unwrap();
    assert_eq!(ui.counters().builds, 1);
    assert_eq!(ui.snapshot().nodes.len(), 3);
}

#[test]
fn bounded_queues_preserve_required_actions_and_report_preview_replacement() {
    let (mut ui, _, root) = setup(Structure::Retained, 3);
    ui.enqueue_preview(root, 7, 10).unwrap();
    ui.enqueue(root, 20).unwrap();
    ui.enqueue(root, 30).unwrap();
    let failed = ui.enqueue(root, 40).unwrap_err();
    assert_eq!(failed.error, CoreError::QueueFull);
    assert_eq!(failed.action, 40);
    assert_eq!(ui.enqueue_preview(root, 7, 11).unwrap().replaced, Some(10));
    let mut model = vec![];
    ui.flush(&mut model).unwrap();
    assert_eq!(model, vec![20, 30, 11]);
    ui.enqueue(root, failed.action).unwrap();
    ui.flush(&mut model).unwrap();
    assert_eq!(model, vec![20, 30, 11, 40]);
}

#[test]
fn layout_propagation_is_window_scoped_and_viewports_do_not_rebuild_ui() {
    let (mut ui, window, root) = setup(Structure::Retained, 8);
    let child = ui.mount(root, item("child", 1)).unwrap();
    let sibling = ui.mount(root, item("sibling", 2)).unwrap();
    let (other, other_root) = ui.open_window(item("other", 0)).unwrap();
    ui.flush(&mut vec![]).unwrap();
    ui.invalidate(child, Dirty::LAYOUT).unwrap();
    for id in [root, child, sibling] {
        assert!(
            ui.dirty(id)
                .unwrap()
                .contains(Dirty::LAYOUT.union(Dirty::SEMANTICS))
        );
    }
    assert_eq!(ui.dirty(other_root), Ok(Dirty::NONE));
    assert!(!ui.demand(other).unwrap().ui);
    ui.flush(&mut vec![]).unwrap();
    let before = ui.counters();
    for _ in 0..120 {
        ui.request_viewport(window).unwrap();
    }
    assert!(!ui.has_work());
    assert!(ui.take_viewport_request(window).unwrap());
    assert!(!ui.take_viewport_request(window).unwrap());
    assert_eq!(ui.flush(&mut vec![]).unwrap().committed, None);
    assert_eq!(ui.counters().commits, before.commits);
    assert_eq!(ui.counters().builds, before.builds);
}

#[test]
fn requests_reject_duplicates_supersession_cancelled_queue_and_reused_owner() {
    let (mut ui, _, root) = setup(Structure::Retained, 8);
    let child = ui.mount(root, item("child", 0)).unwrap();
    let old = ui.start_request(child, 1, || {}).unwrap();
    let latest = ui.start_request(child, 1, || {}).unwrap();
    assert!(old.cancellation().is_revoked());
    assert_eq!(
        ui.complete(&old, 100).unwrap_err().error,
        CoreError::InactiveRequest
    );
    ui.complete(&latest, 2).unwrap();
    assert_eq!(
        ui.complete(&latest, 200).unwrap_err().error,
        CoreError::InactiveRequest
    );
    ui.cancel(&latest).unwrap();
    let mut model = vec![];
    assert_eq!(ui.flush(&mut model).unwrap().rejected.len(), 1);
    assert!(model.is_empty());
    let late = ui.start_request(child, 2, || {}).unwrap();
    ui.unmount(child).unwrap();
    let reused = ui.mount(root, item("reused", 7)).unwrap();
    assert_eq!(reused.handle().index(), child.handle().index());
    assert!(ui.complete(&late, 300).is_err());
    assert_eq!(ui.state::<i32>(reused), Ok(&7));
    let success = ui
        .start_request(reused, 2, || {
            panic!("successful consumption must not cancel")
        })
        .unwrap();
    ui.complete(&success, 3).unwrap();
    ui.flush(&mut model).unwrap();
    assert_eq!(model, vec![3]);
    assert!(success.cancellation().is_revoked());
}

#[test]
fn completion_backpressure_preserves_active_request_for_retry() {
    let (mut ui, _, root) = setup(Structure::Retained, 1);
    let token = ui.start_request(root, 1, || {}).unwrap();
    ui.enqueue(root, 1).unwrap();
    let rejected = ui.complete(&token, 2).unwrap_err();
    assert_eq!(rejected.error, CoreError::QueueFull);
    assert!(!token.cancellation().is_revoked());
    let mut model = vec![];
    ui.flush(&mut model).unwrap();
    ui.complete(&token, rejected.action).unwrap();
    ui.flush(&mut model).unwrap();
    assert_eq!(model, vec![1, 2]);
}

#[test]
fn optional_observer_is_bounded_and_drop_revokes_owned_work() {
    let (mut ui, _, root) = setup(Structure::Retained, 8);
    assert_eq!(ui.drain_trace().count(), 0);
    ui.observe(2);
    ui.enqueue(root, 1).unwrap();
    ui.flush(&mut vec![]).unwrap();
    let trace: Vec<_> = ui.drain_trace().collect();
    assert_eq!(trace.len(), 2);
    assert!(matches!(trace[0], Trace::Action(_, _)));
    assert!(matches!(trace[1], Trace::Committed(_)));
    let cancelled = Rc::new(Cell::new(false));
    let flag = cancelled.clone();
    let token = ui
        .own(root, ResourceKind::Callback, move || flag.set(true))
        .unwrap();
    drop(ui);
    assert!(cancelled.get());
    assert!(token.cancellation().is_revoked());
}

#[test]
fn failed_region_description_does_not_replay_dispatched_effects_or_responses() {
    let (mut ui, _, root) = setup(Structure::Immediate, 8);
    let calls = Rc::new(Cell::new(0));
    let counter = calls.clone();
    ui.set_region_builder(root, move |_, responses| {
        let call = counter.get();
        counter.set(call + 1);
        if call == 0 {
            assert_eq!(responses.len(), 1);
            vec![item("duplicate", 0), item("duplicate", 0)]
        } else {
            assert!(responses.is_empty());
            vec![item("ok", 0)]
        }
    })
    .unwrap();
    ui.enqueue(root, 5).unwrap();
    let mut model = vec![];
    assert_eq!(ui.flush(&mut model), Err(CoreError::DuplicateKey));
    assert_eq!(ui.snapshot().revision, Revision::INITIAL);
    assert_eq!(model, vec![5]);
    ui.flush(&mut model).unwrap();
    assert_eq!(model, vec![5]);
    assert_eq!(calls.get(), 2);
}

#[test]
fn child_actions_wake_enclosing_region_and_suspended_responses_apply_backpressure() {
    let (mut ui, _, root) = setup(Structure::Immediate, 1);
    let responses = Rc::new(RefCell::new(Vec::new()));
    let observed = responses.clone();
    ui.set_region_builder(root, move |_, received| {
        observed.borrow_mut().extend_from_slice(received);
        vec![item("button", 0)]
    })
    .unwrap();
    let mut model = vec![];
    ui.flush(&mut model).unwrap();
    let button = ui.snapshot().nodes[0].children[0];
    ui.set_visibility(root, Visibility::Suspended).unwrap();
    let first = ui.enqueue(button, 1).unwrap().sequence;
    ui.flush(&mut model).unwrap();
    let second = ui.enqueue(button, 2).unwrap().sequence;
    let blocked = ui.flush(&mut model).unwrap();
    assert!(blocked.backpressure);
    assert_eq!(model, vec![1]);
    assert_eq!(ui.pending_actions(), 1);
    ui.set_visibility(root, Visibility::Visible).unwrap();
    ui.flush(&mut model).unwrap();
    assert_eq!(*responses.borrow(), vec![first]);
    ui.flush(&mut model).unwrap();
    assert_eq!(*responses.borrow(), vec![first, second]);
    assert_eq!(model, vec![1, 2]);
    assert!(!ui.has_work());
}

#[test]
fn tokens_cross_worker_threads_but_runtime_and_model_stay_on_owner() {
    let (mut ui, _, root) = setup(Structure::Retained, 4);
    let token = ui.start_request(root, 1, || {}).unwrap();
    let (token, action) = std::thread::spawn(move || {
        assert!(!token.cancellation().is_revoked());
        (token, 42)
    })
    .join()
    .unwrap();
    ui.complete(&token, action).unwrap();
    let mut model = vec![];
    ui.flush(&mut model).unwrap();
    assert_eq!(model, vec![42]);
}

#[test]
fn structure_change_resets_state_and_window_generation_rejects_closed_id() {
    let (mut ui, window, root) = setup(Structure::Declarative, 4);
    let child = ui.reconcile(root, vec![item("a", 1)]).unwrap()[0];
    let replacement = ui
        .reconcile(root, vec![item("a", 2).structure(Structure::Declarative)])
        .unwrap()[0];
    assert_ne!(replacement, child);
    assert_eq!(ui.state::<i32>(replacement), Ok(&2));
    ui.close_window(window).unwrap();
    let (next, _) = ui.open_window(item("new", 0)).unwrap();
    assert_eq!(next.handle().index(), window.handle().index());
    assert_ne!(next, window);
    assert!(ui.request_viewport(window).is_err());
}
