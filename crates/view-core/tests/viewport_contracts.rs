use std::time::Duration;
use view_core::{host::*, viewport::*, *};

#[test]
fn independent_frames_preserve_both_inspectors_and_suspend_with_ancestor() {
    let mut ui = Runtime::<(), ()>::new(16).unwrap();
    let (window, root) = ui
        .open_window(
            Description::new("root", (), |_, _, _| Dirty::NONE)
                .layout(LogicalRect::from_xywh(0., 0., 400., 400.)),
        )
        .unwrap();
    let retained = ui
        .mount(
            root,
            Description::new("retained", 7usize, |_, _, _| Dirty::NONE),
        )
        .unwrap();
    let immediate = ui
        .mount(
            root,
            Description::new("immediate", (), |_, _, _| Dirty::NONE)
                .structure(Structure::Immediate),
        )
        .unwrap();
    ui.set_region_builder(immediate, |_, _| {
        vec![Description::new("label", 42usize, |_, _, _| Dirty::NONE)]
    })
    .unwrap();
    let viewport = ui
        .mount(
            root,
            Description::new("viewport", (), |_, _, _| Dirty::NONE)
                .layout(LogicalRect::from_xywh(0., 0., 100., 100.)),
        )
        .unwrap();
    ui.flush(&mut ()).unwrap();
    let before = ui.counters();
    let committed = ui.snapshot().clone();
    let mut frames = ViewportScheduler::new(1);
    frames
        .register(viewport, FramePolicy::Continuous(Duration::from_millis(16)))
        .unwrap();
    for tick in 0..120 {
        assert_eq!(
            frames
                .poll(ui.snapshot(), Duration::from_millis(tick * 16), |_| true)
                .unwrap(),
            vec![viewport]
        );
        ui.request_viewport(window).unwrap();
        assert!(ui.take_viewport_request(window).unwrap());
        ui.flush(&mut ()).unwrap();
    }
    assert_eq!(ui.counters().builds, before.builds);
    assert_eq!(ui.counters().layouts, before.layouts);
    assert_eq!(ui.counters().commits, before.commits);
    assert_eq!(ui.snapshot(), &committed);
    assert_eq!(*ui.state::<usize>(retained).unwrap(), 7);
    ui.set_visibility(root, Visibility::Hidden).unwrap();
    ui.flush(&mut ()).unwrap();
    assert!(
        frames
            .poll(ui.snapshot(), Duration::from_secs(10), |_| true)
            .unwrap()
            .is_empty()
    );
    assert_eq!(frames.deadline(), None);
    ui.set_visibility(root, Visibility::Visible).unwrap();
    ui.flush(&mut ()).unwrap();
    assert_eq!(
        frames
            .poll(ui.snapshot(), Duration::from_secs(11), |_| true)
            .unwrap(),
        vec![viewport]
    );
    ui.unmount(viewport).unwrap();
    ui.flush(&mut ()).unwrap();
    assert!(
        frames
            .poll(ui.snapshot(), Duration::from_secs(12), |_| true)
            .unwrap()
            .is_empty()
    );
    assert!(frames.is_empty());
}

#[test]
fn demand_coalesces_and_minimization_keeps_pending_work_without_busy_wakeup() {
    let mut ui = Runtime::<(), ()>::new(4).unwrap();
    let (_, root) = ui
        .open_window(
            Description::new("v", (), |_, _, _| Dirty::NONE)
                .layout(LogicalRect::from_xywh(0., 0., 20., 20.)),
        )
        .unwrap();
    ui.flush(&mut ()).unwrap();
    let mut frames = ViewportScheduler::new(1);
    assert!(
        frames
            .register(root, FramePolicy::Continuous(Duration::ZERO))
            .is_err()
    );
    frames.register(root, FramePolicy::OnDemand).unwrap();
    frames.request(root).unwrap();
    frames.request(root).unwrap();
    assert!(
        frames
            .poll(ui.snapshot(), Duration::ZERO, |_| false)
            .unwrap()
            .is_empty()
    );
    assert_eq!(frames.deadline(), None);
    assert_eq!(
        frames
            .poll(ui.snapshot(), Duration::from_secs(1), |_| true)
            .unwrap(),
        vec![root]
    );
    assert!(
        frames
            .poll(ui.snapshot(), Duration::from_secs(1), |_| true)
            .unwrap()
            .is_empty()
    );
    assert_eq!(frames.deadline(), None);
    assert!(
        frames
            .poll(ui.snapshot(), Duration::ZERO, |_| true)
            .is_err()
    );
}

#[derive(Default)]
struct Document {
    value: i32,
    undo: Vec<i32>,
    revision: Revision,
}
fn pointer(phase: PointerPhase, x: f32) -> PointerEvent {
    PointerEvent {
        phase,
        position: LogicalPoint::new(x, 5.),
        button: Some(PointerButton::Primary),
        modifiers: Modifiers::default(),
        timestamp: 0,
    }
}

#[test]
fn captured_drag_queues_one_document_transaction_and_cancel_has_no_effect() {
    let mut ui = Runtime::<Document, DragCommit<i32>>::new(8).unwrap();
    let (window, owner) = ui
        .open_window(
            Description::new(
                "gizmo",
                (),
                |_, model: &mut Document, commit: DragCommit<i32>| {
                    if model.revision == commit.revision {
                        model.undo.push(commit.before);
                        model.value = commit.after;
                        model.revision = model.revision.checked_next().unwrap();
                    }
                    Dirty::VIEWPORT
                },
            )
            .layout(LogicalRect::from_xywh(0., 0., 20., 20.)),
        )
        .unwrap();
    let mut doc = Document::default();
    ui.flush(&mut doc).unwrap();
    ui.route_pointer(window, pointer(PointerPhase::Down, 5.), &mut doc)
        .unwrap();
    ui.capture_pointer(window, owner).unwrap();
    let mut drag = DragSession::new(owner, 9, doc.revision, doc.value);
    for x in [10., 30., 60.] {
        for action in ui
            .route_pointer(window, pointer(PointerPhase::Move, x), &mut doc)
            .unwrap()
        {
            assert!(drag.route(&action, doc.revision, |p| p.x as i32).is_none());
        }
    }
    assert_eq!(*drag.preview(), 60);
    assert_eq!(doc.value, 0);
    let actions = ui
        .route_pointer(window, pointer(PointerPhase::Up, 70.), &mut doc)
        .unwrap();
    for a in &actions {
        if let Some(commit) = drag.route(a, doc.revision, |p| p.x as i32) {
            ui.enqueue(owner, commit).unwrap();
        }
    }
    for a in &actions {
        assert!(drag.route(a, doc.revision, |p| p.x as i32).is_none());
    }
    ui.flush(&mut doc).unwrap();
    assert_eq!(doc.value, 70);
    assert_eq!(doc.undo, vec![0]);
    doc.value = doc.undo.pop().unwrap();
    assert_eq!(doc.value, 0);
    let mut cancelled = DragSession::new(owner, 9, doc.revision, doc.value);
    cancelled.route(
        &RoutedPointerAction::Move {
            target: owner,
            local: LogicalPoint::new(50., 5.),
        },
        doc.revision,
        |p| p.x as i32,
    );
    ui.capture_pointer(window, owner).unwrap();
    for a in ui.cancel_pointer(window).unwrap() {
        assert!(cancelled.route(&a, doc.revision, |p| p.x as i32).is_none());
    }
    assert_eq!(*cancelled.preview(), 0);
    assert!(cancelled.finished());
    assert!(doc.undo.is_empty());
    let mut removed = DragSession::new(owner, 9, doc.revision, 0);
    ui.set_visibility(owner, Visibility::Hidden).unwrap();
    ui.flush(&mut doc).unwrap();
    removed.reconcile(ui.snapshot());
    assert!(removed.finished());
}

struct Services {
    fail: bool,
    publications: Vec<SemanticSnapshot>,
}
impl HostServices for Services {
    fn capabilities(&self) -> HostCapabilities {
        HostCapabilities {
            clipboard: ServiceSupport::Unavailable,
            ime: ServiceSupport::Unavailable,
            accessibility: ServiceSupport::Host,
        }
    }
    fn publish_semantics(&mut self, s: &SemanticSnapshot) -> Result<(), HostServiceError> {
        if self.fail {
            self.fail = false;
            return Err(HostServiceError::Failed("retry".into()));
        }
        self.publications.push(s.clone());
        Ok(())
    }
    fn ime(&mut self, _: bool, _: LogicalRect) -> Result<(), HostServiceError> {
        Err(HostServiceError::Unsupported)
    }
    fn clipboard_read(&mut self) -> Result<String, HostServiceError> {
        Err(HostServiceError::Unsupported)
    }
    fn clipboard_write(&mut self, _: &str) -> Result<(), HostServiceError> {
        Err(HostServiceError::Unsupported)
    }
}
#[test]
fn host_services_publish_commits_once_retry_failures_and_report_unavailable() {
    let mut ui = Runtime::<(), ()>::new(4).unwrap();
    let (_, root) = ui
        .open_window(
            Description::new("host", (), |_, _, _| Dirty::NONE)
                .role(Role::Button)
                .name("Host action"),
        )
        .unwrap();
    ui.flush(&mut ()).unwrap();
    let mut host = HostBridge::new(Services {
        fail: true,
        publications: vec![],
    });
    assert!(host.publish(ui.snapshot()).is_err());
    assert!(host.publish(ui.snapshot()).unwrap());
    assert!(!host.publish(ui.snapshot()).unwrap());
    assert_eq!(
        host.services().publications[0].nodes[0].name.as_deref(),
        Some("Host action")
    );
    assert_eq!(
        host.services().clipboard_read(),
        Err(HostServiceError::Unsupported)
    );
    ui.set_visibility(root, Visibility::Hidden).unwrap();
    ui.flush(&mut ()).unwrap();
    assert!(host.publish(ui.snapshot()).unwrap());
    assert_eq!(host.services().publications.len(), 2);
}
