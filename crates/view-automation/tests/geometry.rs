use view_automation::*;
use view_core::*;
use view_testing::Harness;
#[test]
fn clips_overlays_scrollers_reuse_and_ambiguity_never_produce_guessed_targets() {
    let mut rt = Runtime::<(), ()>::new(16).unwrap();
    let (w, root) = rt
        .open_window(
            Description::new("root", (), |_, _, _| Dirty::NONE).size(LogicalSize::new(300., 200.)),
        )
        .unwrap();
    rt.set_window_size(w, LogicalSize::new(300., 200.)).unwrap();
    let scroll = rt
        .mount(
            root,
            Description::new("scroll", (), |_, _, _| Dirty::NONE)
                .layout(LogicalRect::from_xywh(20., 20., 100., 60.)),
        )
        .unwrap();
    rt.set_scroll_state(
        scroll,
        ScrollState::new(LogicalSize::new(100., 200.), LogicalSize::new(100., 60.)),
    )
    .unwrap();
    let item = rt
        .mount(
            scroll,
            Description::new("item", (), |_, _, _| Dirty::NONE)
                .name("row")
                .layout(LogicalRect::from_xywh(0., 80., 80., 20.)),
        )
        .unwrap();
    rt.flush(&mut ()).unwrap();
    let mut s = Session::new(
        "geometry".into(),
        Harness::new(rt, (), 8),
        vec![Scope::Inspect],
        32,
    )
    .unwrap();
    let selector = Selector {
        name: Some("row".into()),
        ..Default::default()
    };
    let t = s.resolve(&selector).unwrap();
    assert_eq!(
        s.actionable_point(&t).unwrap_err().code,
        Code::NotActionable
    );
    s.harness
        .runtime_mut()
        .scroll_by(scroll, LogicalPoint::new(0., 70.))
        .unwrap();
    s.harness.step().unwrap();
    assert_eq!(s.actionable_point(&t).unwrap_err().code, Code::Stale);
    let t = s.resolve(&selector).unwrap();
    let (_, p) = s.actionable_point(&t).unwrap();
    assert_eq!(p, LogicalPoint::new(60., 40.)); // Independent analytic expectation: 20 + 80 - 70 + 10.
    let overlay = s
        .harness
        .runtime_mut()
        .mount(
            root,
            Description::new("overlay", (), |_, _, _| Dirty::NONE)
                .layout(LogicalRect::from_xywh(20., 20., 100., 60.)),
        )
        .unwrap();
    s.harness.step().unwrap();
    assert_eq!(
        s.actionable_point(&s.resolve(&selector).unwrap())
            .unwrap_err()
            .code,
        Code::NotActionable
    );
    s.harness.runtime_mut().unmount(overlay).unwrap();
    s.harness.runtime_mut().unmount(item).unwrap();
    s.harness
        .runtime_mut()
        .mount(
            scroll,
            Description::new("replacement", (), |_, _, _| Dirty::NONE).name("row"),
        )
        .unwrap();
    s.harness.step().unwrap();
    let mut stale = t;
    stale.revision = s.revision();
    assert_eq!(s.actionable_point(&stale).unwrap_err().code, Code::Stale);
    s.harness
        .runtime_mut()
        .mount(
            root,
            Description::new("duplicate", (), |_, _, _| Dirty::NONE).name("row"),
        )
        .unwrap();
    s.harness.step().unwrap();
    assert_eq!(s.resolve(&selector).unwrap_err().code, Code::Ambiguous);
}

#[test]
fn realization_is_explicit_setup_and_inspector_escapes_labels() {
    let mut rt = Runtime::<(), ()>::new(8).unwrap();
    let (w, root) = rt
        .open_window(Description::new("root", (), |_, _, _| Dirty::NONE))
        .unwrap();
    rt.set_window_size(w, LogicalSize::new(100., 100.)).unwrap();
    rt.flush(&mut ()).unwrap();
    let mut session = Session::new(
        "realize".into(),
        Harness::new(rt, (), 8),
        vec![Scope::Inspect, Scope::Test],
        16,
    )
    .unwrap();
    session.set_realizer(move |h, selector| {
        h.runtime_mut().mount(
            root,
            Description::new("virtual-row", (), |_, _, _| Dirty::NONE)
                .role(Role::Button)
                .name(selector.name.as_deref().unwrap_or(""))
                .layout(LogicalRect::from_xywh(10., 10., 20., 20.)),
        )?;
        Ok(())
    });
    let request = Request {
        version: VERSION,
        session: "realize".into(),
        id: 1,
        operation: Operation::Realize {
            selector: Selector {
                name: Some("<script>alert(1)</script>".into()),
                ..Default::default()
            },
        },
    };
    let response = session.execute(request);
    assert_eq!(
        response.result.unwrap()["provenance"],
        "semantic setup; not physical scrolling"
    );
    let response = session.execute(Request {
        version: VERSION,
        session: "realize".into(),
        id: 2,
        operation: Operation::Inspector,
    });
    let value = response.result.unwrap();
    let html = value["html"].as_str().unwrap();
    assert!(!html.contains("<script>"));
    assert!(html.contains("&lt;script&gt;"));
    assert!(html.contains("Accumulated clip"));
    assert!(html.contains("Verified hit point"));
    let row = value["snapshot"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["key"] == "virtual-row")
        .unwrap();
    assert_eq!(row["actionable_point"], serde_json::json!([20.0, 20.0]));
}
