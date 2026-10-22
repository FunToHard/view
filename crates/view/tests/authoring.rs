use view::{authoring::*, *};
#[test]
fn tuples_branches_and_keyed_reorder_preserve_local_state_but_replacement_does_not() {
    let mut ui = Runtime::<(), ()>::new(4).unwrap();
    let (window, root) = ui
        .open_window(
            Description::new("root", (), |_, _, _| Dirty::NONE).structure(Structure::Declarative),
        )
        .unwrap();
    let row = |key: &str| {
        Description::new(key, 42usize, |_, _, _| Dirty::NONE)
            .focusable(true)
            .into_element()
    };
    reconcile(
        &mut ui,
        root,
        (
            Column::new("list", vec![row("a"), row("b")]),
            Text::new("status", "Ready"),
        ),
    )
    .unwrap();
    ui.flush(&mut ()).unwrap();
    let a = ui
        .snapshot()
        .nodes
        .iter()
        .find(|n| n.key == "a")
        .unwrap()
        .id;
    ui.update(a, Dirty::NONE, |s: &mut usize| *s = 9).unwrap();
    ui.set_focus(window, Some(a)).unwrap();
    reconcile(
        &mut ui,
        root,
        (
            Column::new("list", keyed(["b", "a"], row)),
            Text::new("status", "Refreshing"),
        ),
    )
    .unwrap();
    ui.flush(&mut ()).unwrap();
    assert_eq!(*ui.state::<usize>(a).unwrap(), 9);
    assert_eq!(ui.focused(window).unwrap(), Some(a));
    let empty: Element<(), ()> = Text::new("empty", "No results").into_element();
    reconcile(&mut ui, root, (empty,)).unwrap();
    ui.flush(&mut ()).unwrap();
    assert!(ui.state::<usize>(a).is_err());
    assert_eq!(ui.focused(window).unwrap(), None);
}
#[test]
fn nested_duplicates_fail_before_mutation_and_retained_islands_survive() {
    let mut ui = Runtime::<(), ()>::new(4).unwrap();
    let (_, root) = ui
        .open_window(
            Description::new("root", (), |_, _, _| Dirty::NONE).structure(Structure::Declarative),
        )
        .unwrap();
    let island = || Description::new("island", (), |_, _, _| Dirty::NONE).into_element();
    let id = reconcile(&mut ui, root, (island(),)).unwrap()[0];
    let child = ui
        .mount(id, Description::new("owned", 5, |_, _, _| Dirty::NONE))
        .unwrap();
    reconcile(&mut ui, root, (island(),)).unwrap();
    assert_eq!(*ui.state::<i32>(child).unwrap(), 5);
    let bad = Column::new("bad", (Text::new("x", "1"), Text::new("x", "2")));
    assert_eq!(
        reconcile(&mut ui, root, (bad,)),
        Err(CoreError::DuplicateKey)
    );
    assert!(ui.state::<i32>(child).is_ok());
    let invalid = Description::new("invalid", (), |_, _, _| Dirty::NONE)
        .transform(Transform2D::translation(f32::NAN, 0.));
    assert_eq!(
        reconcile(&mut ui, root, (invalid,)),
        Err(CoreError::InvalidGeometry)
    );
    assert!(ui.state::<i32>(child).is_ok());
}
#[test]
fn immediate_island_skips_without_replaying_actions() {
    let mut ui = Runtime::<usize, usize>::new(4).unwrap();
    let (_, root) = ui
        .open_window(
            Description::new("root", (), |_, _, _| Dirty::NONE).structure(Structure::Declarative),
        )
        .unwrap();
    let region = Description::new("immediate", (), |_, m: &mut usize, a| {
        *m += a;
        Dirty::BUILD
    })
    .structure(Structure::Immediate);
    let id = reconcile(&mut ui, root, (region,)).unwrap()[0];
    ui.set_region_builder(id, |m, _| {
        vec![Description::new("value", *m, |_, _, _| Dirty::NONE)]
    })
    .unwrap();
    let mut model = 0;
    ui.flush(&mut model).unwrap();
    let before = ui.counters();
    for _ in 0..20 {
        ui.flush(&mut model).unwrap();
    }
    assert_eq!(ui.counters().builds, before.builds);
    ui.enqueue(id, 1).unwrap();
    ui.flush(&mut model).unwrap();
    ui.flush(&mut model).unwrap();
    assert_eq!(model, 1);
}
