//! Hidden native docking lifecycle proof. Never injects input or activates windows.
//! Run with `cargo run -p view-controls --example docking --features docking -- --verify-hidden`.
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use view_controls::{
    DockCommand, DockLayout, DockNode, DockPanelHandle, DockSnapshot, DockWindow, DockWorkArea,
};
use view_core::{Description, Dirty, ResourceKind, RoutedPointerAction, Runtime};
use winit::{
    application::ApplicationHandler,
    dpi::{PhysicalPosition, PhysicalSize},
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
type NativeWindows = HashMap<String, (Window, view_core::WindowId, view_core::NodeId)>;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Document {
    text: String,
    caret: usize,
    revision: u64,
}

fn panel(tab: &str) -> Description<(), ()> {
    Description::new(tab, tab.to_owned(), |_, _, _| Dirty::NONE).focusable(true)
}
fn native_window(event_loop: &ActiveEventLoop, spec: &DockWindow) -> Result<Window> {
    // Persist outer bounds in physical virtual-desktop pixels. Winit's size
    // builder is client size, so subtract measured non-client decoration below.
    let [x, y, width, height] = spec.bounds;
    let window = event_loop.create_window(
        Window::default_attributes()
            .with_title(format!("view dock fixture: {}", spec.id))
            .with_visible(false)
            .with_active(false)
            .with_position(PhysicalPosition::new(x.round() as i32, y.round() as i32))
            .with_inner_size(PhysicalSize::new(
                width.round() as u32,
                height.round() as u32,
            )),
    )?;
    let inner = window.inner_size();
    let outer = window.outer_size();
    let decorations = [
        outer.width.saturating_sub(inner.width),
        outer.height.saturating_sub(inner.height),
    ];
    let _ = window.request_inner_size(PhysicalSize::new(
        (width.round() as u32).saturating_sub(decorations[0]).max(1),
        (height.round() as u32)
            .saturating_sub(decorations[1])
            .max(1),
    ));
    window.set_outer_position(PhysicalPosition::new(x.round() as i32, y.round() as i32));
    Ok(window)
}
fn tabs(node: &DockNode, out: &mut Vec<String>) {
    match node {
        DockNode::Stack { tabs, .. } => out.extend(tabs.iter().cloned()),
        DockNode::Split { first, second, .. } => {
            tabs(first, out);
            tabs(second, out);
        }
    }
}
fn mount_layout(
    event_loop: &ActiveEventLoop,
    layout: &DockLayout,
    runtime: &mut Runtime<(), ()>,
) -> Result<(NativeWindows, HashMap<String, DockPanelHandle>)> {
    let mut windows = HashMap::new();
    let mut panels = HashMap::new();
    for spec in layout.windows() {
        let native = native_window(event_loop, spec)?;
        let (window, root) =
            runtime.open_window(Description::new(&spec.id, (), |_, _, _| Dirty::NONE))?;
        let mut members = Vec::new();
        tabs(&spec.root, &mut members);
        for tab in members {
            panels.insert(
                tab.clone(),
                DockPanelHandle {
                    window,
                    root: runtime.mount(root, panel(&tab))?,
                },
            );
        }
        windows.insert(spec.id.clone(), (native, window, root));
    }
    layout.reconcile_runtime(runtime, &panels)?;
    Ok((windows, panels))
}
fn actual_bounds(window: &Window) -> Result<[f32; 4]> {
    let origin = window.outer_position()?;
    let size = window.outer_size();
    Ok([
        origin.x as f32,
        origin.y as f32,
        size.width as f32,
        size.height as f32,
    ])
}
fn verify_windows(windows: &NativeWindows, layout: &DockLayout, area: DockWorkArea) -> Result<()> {
    assert_eq!(windows.len(), 2);
    let mut native_ids: Vec<_> = windows.values().map(|(window, _, _)| window.id()).collect();
    native_ids.dedup();
    assert_eq!(native_ids.len(), 2);
    for spec in layout.windows() {
        let native = &windows[&spec.id].0;
        assert_eq!(native.is_visible(), Some(false));
        let bounds = actual_bounds(native)?;
        assert_eq!(bounds, spec.bounds, "actual native outer bounds restored");
        assert!(bounds[0] >= area.bounds[0] && bounds[1] >= area.bounds[1]);
        assert!(bounds[0] + bounds[2] <= area.bounds[0] + area.bounds[2]);
        assert!(bounds[1] + bounds[3] <= area.bounds[1] + area.bounds[3]);
    }
    Ok(())
}

#[derive(Default)]
struct App {
    result: Option<std::result::Result<(), String>>,
}
impl App {
    fn exercise(&self, event_loop: &ActiveEventLoop) -> Result<()> {
        let area = DockWorkArea {
            bounds: view_platform::monitor::work_area_at([0, 0])?.map(|n| n as f32),
        };
        assert!(area.bounds[2] > 0.0 && area.bounds[3] > 0.0);
        let [x, y, _, _] = area.bounds;
        let initial = DockSnapshot {
            windows: vec![DockWindow {
                id: "main".into(),
                bounds: [x + 20.0, y + 20.0, 640.0, 400.0],
                root: DockNode::Stack {
                    id: "editor".into(),
                    tabs: vec!["doc-a".into(), "doc-b".into()],
                    active: "doc-a".into(),
                },
            }],
        };
        let mut layout = DockLayout::restore_in_work_areas(initial, &[area], [360.0, 240.0])?;
        // Unsaved document and caret state live outside the disposable UI tree.
        let documents = HashMap::from([
            (
                "doc-a",
                Document {
                    text: "Unsaved α document".into(),
                    caret: 9,
                    revision: 17,
                },
            ),
            (
                "doc-b",
                Document {
                    text: "Second document".into(),
                    caret: 0,
                    revision: 3,
                },
            ),
        ]);
        let original = documents.clone();
        let mut runtime = Runtime::new(16)?;
        let (mut windows, mut panels) = mount_layout(event_loop, &layout, &mut runtime)?;
        let old = panels["doc-a"];
        runtime.set_focus(old.window, Some(old.root))?;
        runtime.capture_pointer(old.window, old.root)?;
        let revoked = Arc::new(AtomicUsize::new(0));
        let revocation = revoked.clone();
        let lease = runtime.own(old.root, ResourceKind::Task, move || {
            revocation.fetch_add(1, Ordering::SeqCst);
        })?;
        let transition = layout.plan(DockCommand::TearOff {
            tab: "doc-a".into(),
            window: "detached".into(),
            bounds: [x + 70.0, y + 70.0, 480.0, 300.0],
        })?;
        assert_eq!(transition.migrations().len(), 1);
        assert_eq!(transition.migrations()[0].source_window, "main");
        assert_eq!(transition.migrations()[0].target_window, "detached");
        let restored = DockLayout::restore_in_work_areas(
            transition.snapshot().clone(),
            &[area],
            [360.0, 240.0],
        )?;
        let spec = restored
            .windows()
            .iter()
            .find(|spec| spec.id == "detached")
            .unwrap();
        // All fallible creation is staged before retiring the old mount. If a
        // stage fails, its new native/logical owners are disposed, keeping old UI.
        let native = native_window(event_loop, spec)?;
        let (window, root) =
            runtime.open_window(Description::new("detached", (), |_, _, _| Dirty::NONE))?;
        let new_root = match runtime.mount(root, panel("doc-a")) {
            Ok(root) => root,
            Err(error) => {
                runtime.close_window(window)?;
                return Err(error.into());
            }
        };
        let cancellations = runtime.cancel_pointer(old.window)?;
        assert!(cancellations.contains(&RoutedPointerAction::Cancel(old.root)));
        // A real control host delivers these cancellations to its behavior engine
        // before unmount. This fixture has no active OS mouse capture or IME.
        runtime.set_focus(old.window, None)?;
        runtime.unmount(old.root)?;
        assert_eq!(revoked.load(Ordering::SeqCst), 1);
        assert!(lease.cancellation().is_revoked());
        assert!(runtime.node_window(old.root).is_err());
        layout.commit(transition)?;
        // Apply the same work-area policy to the committed model used for native creation.
        layout = DockLayout::restore_in_work_areas(layout.snapshot(), &[area], [360.0, 240.0])?;
        windows.insert("detached".into(), (native, window, root));
        panels.insert(
            "doc-a".into(),
            DockPanelHandle {
                window,
                root: new_root,
            },
        );
        layout.reconcile_runtime(&mut runtime, &panels)?;
        runtime.set_focus(window, Some(new_root))?;
        assert_eq!(runtime.focused(old.window)?, None);
        assert_eq!(runtime.captured(old.window)?, None);
        assert_eq!(runtime.captured(window)?, None);
        assert_eq!(runtime.focused(window)?, Some(new_root));
        assert_eq!(documents, original);
        verify_windows(&windows, &layout, area)?;

        let mut saved = layout.snapshot();
        for spec in &mut saved.windows {
            spec.bounds = actual_bounds(&windows[&spec.id].0)?;
        }
        let path = std::path::Path::new("target/docking-layout.vdk");
        std::fs::create_dir_all("target")?;
        std::fs::write(path, saved.encode()?)?;
        for (_, (_, window, _)) in windows.drain() {
            runtime.close_window(window)?;
        }
        assert!(runtime.node_window(new_root).is_err());
        let restored = DockLayout::restore_in_work_areas(
            DockSnapshot::decode(&std::fs::read(path)?)?,
            &[area],
            [360.0, 240.0],
        )?;
        assert_eq!(restored.snapshot(), saved);
        let (mut windows, panels) = mount_layout(event_loop, &restored, &mut runtime)?;
        verify_windows(&windows, &restored, area)?;
        let restored_panel = panels["doc-a"];
        assert_ne!(restored_panel.root, new_root);
        assert_eq!(runtime.state::<String>(restored_panel.root)?, "doc-a");
        runtime.set_focus(restored_panel.window, Some(restored_panel.root))?;
        assert_eq!(
            runtime.focused(restored_panel.window)?,
            Some(restored_panel.root)
        );
        assert_eq!(runtime.captured(restored_panel.window)?, None);
        assert_eq!(documents, original);
        for (_, (_, window, _)) in windows.drain() {
            runtime.close_window(window)?;
        }
        runtime.flush(&mut ())?;
        assert!(runtime.snapshot().nodes.is_empty());
        println!(
            "PASS hidden native docking: 2 windows, physical work-area bounds, tear-off, capture cancellation, lease revocation, remount, persisted restore, app state and cleanup; fixture {}",
            path.display()
        );
        Ok(())
    }
}
impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.result = Some(self.exercise(event_loop).map_err(|error| error.to_string()));
        event_loop.exit();
    }
    fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}
}
fn main() -> Result<()> {
    if std::env::args().nth(1).as_deref() != Some("--verify-hidden") {
        return Err("explicit --verify-hidden required".into());
    }
    let mut app = App::default();
    EventLoop::new()?.run_app(&mut app)?;
    app.result
        .ok_or("native callback did not run")?
        .map_err(Into::into)
}
