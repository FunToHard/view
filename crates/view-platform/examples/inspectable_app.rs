//! Minimal inspectable native application demonstrating M1B architecture contracts:
//! queued actions, keyed identity, coherent snapshots, unmounting, and idle quiescence.
//!
//! Run with: `cargo run -p view-platform --example inspectable_app -- --run-hidden`

use std::env;
use view_core::{
    Description, Dirty, LogicalRect, LogicalSize, NodeId, Role, Runtime, SemanticAction, Structure,
    WindowId,
};
use view_platform::{
    AppConfig, PlatformApp, PlatformHandler, PlatformShell, PlatformWindow, PlatformWindowEvent,
};
use winit::event_loop::ActiveEventLoop;

#[derive(Debug, Default)]
struct AppModel {
    counter: i32,
    items: Vec<String>,
}

#[derive(Debug)]
enum AppAction {
    Increment,
    RemoveItem(usize),
}

struct InspectableAppHandler {
    runtime: Runtime<AppModel, AppAction>,
    model: AppModel,
    window_id: Option<WindowId>,
    root_id: Option<NodeId>,
    run_hidden: bool,
    steps_completed: bool,
    idle_ticks: u32,
}

impl InspectableAppHandler {
    fn new(run_hidden: bool) -> Result<Self, Box<dyn std::error::Error>> {
        let runtime = Runtime::new(16)?;
        let model = AppModel {
            counter: 0,
            items: vec!["item-a".into(), "item-b".into(), "item-c".into()],
        };

        Ok(Self {
            runtime,
            model,
            window_id: None,
            root_id: None,
            run_hidden,
            steps_completed: false,
            idle_ticks: 0,
        })
    }

    fn find_node_key(&self, key: &str) -> Option<NodeId> {
        self.runtime
            .snapshot()
            .nodes
            .iter()
            .find(|n| n.key == key)
            .map(|n| n.id)
    }

    fn run_contract_steps(
        &mut self,
        shell: &mut PlatformShell,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let win_id = self.window_id.expect("window must be opened");
        let root = self.root_id.expect("root must be mounted");

        println!("[1/5] Testing queued action execution...");
        let action_node = self.find_node_key("btn-inc").unwrap_or(root);
        self.runtime
            .enqueue(action_node, AppAction::Increment)
            .map_err(|r| r.error)?;
        let flush_receipt = self.runtime.flush(&mut self.model)?;
        assert_eq!(self.model.counter, 1);
        assert_eq!(flush_receipt.dispatched, 1);
        println!(
            "      Action executed once: counter={}, committed={:?}",
            self.model.counter, flush_receipt.committed
        );

        println!("[2/5] Testing keyed identity and reconciliation...");
        let snap_before = self.runtime.snapshot();
        assert!(snap_before.nodes.iter().any(|n| n.key == "item-a"));
        assert!(snap_before.nodes.iter().any(|n| n.key == "item-b"));
        println!("      Found keyed nodes item-a and item-b in committed snapshot.");

        println!("[3/5] Testing unmount and structural reconciliation...");
        self.runtime
            .enqueue(root, AppAction::RemoveItem(1))
            .map_err(|r| r.error)?;
        let _ = self.runtime.flush(&mut self.model)?;

        // Reconcile list children to reflect removed item
        let list_node = self.find_node_key("item-list").unwrap_or(root);
        let updated_item_descriptions: Vec<_> = self
            .model
            .items
            .iter()
            .map(|name| {
                Description::new(name.clone(), (), |_, _, _: AppAction| Dirty::NONE)
                    .role(Role::StaticText)
                    .name(name.clone())
            })
            .collect();
        self.runtime
            .reconcile(list_node, updated_item_descriptions)?;
        let _ = self.runtime.flush(&mut self.model)?;

        let snap_after = self.runtime.snapshot();
        assert!(!snap_after.nodes.iter().any(|n| n.key == "item-b"));
        println!(
            "      Item item-b cleanly unmounted; remaining items={:?}",
            self.model.items
        );

        println!("[4/5] Testing coherent tree, semantic and platform window snapshots...");
        let semantic_snap = self.runtime.semantic_snapshot(win_id)?;
        assert!(!semantic_snap.nodes.is_empty());
        let window_snap = shell.snapshot_window(win_id).expect("window snapshot");
        let native = shell
            .get_window(win_id)
            .expect("native window")
            .raw_window();
        let client = native.inner_position()?;
        assert_eq!(window_snap.client_screen_origin, (client.x, client.y));
        let dpi = shell.get_window(win_id).expect("window").dpi();
        assert_eq!(
            dpi.client_physical_to_screen(view_core::PhysicalPoint::new(0.0, 0.0)),
            (client.x, client.y)
        );
        println!(
            "      Window snapshot: title='{}', logical_size={:?}, scale={}",
            window_snap.title,
            window_snap.logical_size,
            window_snap.scale_factor.get()
        );

        println!("[5/5] Testing idle behavior quiescence...");
        let before_counters = self.runtime.counters();
        for _ in 0..10 {
            self.runtime.flush(&mut self.model)?;
        }
        let after_counters = self.runtime.counters();
        assert_eq!(
            before_counters.builds, after_counters.builds,
            "Idle runtime must not trigger rebuilds"
        );
        assert_eq!(
            before_counters.commits, after_counters.commits,
            "Idle runtime must not trigger commits"
        );
        println!("      Idle confirmed: 0 redundant rebuilds or commits across 10 flushes.");

        self.steps_completed = true;
        Ok(())
    }
}

impl PlatformHandler for InspectableAppHandler {
    fn on_ready(
        &mut self,
        shell: &mut PlatformShell,
        event_loop: &ActiveEventLoop,
    ) -> Result<(), view_platform::PlatformError> {
        let (win_id, root_id) = self
            .runtime
            .open_window(
                Description::new(
                    "app-root",
                    (),
                    |_, model: &mut AppModel, action: AppAction| match action {
                        AppAction::Increment => {
                            model.counter += 1;
                            Dirty::BUILD
                        }
                        AppAction::RemoveItem(idx) => {
                            if idx < model.items.len() {
                                model.items.remove(idx);
                            }
                            Dirty::BUILD
                        }
                    },
                )
                .structure(Structure::Declarative)
                .role(Role::Container)
                .name("Inspectable Application"),
            )
            .map_err(view_platform::PlatformError::Core)?;

        self.window_id = Some(win_id);
        self.root_id = Some(root_id);

        let initial_items = self.model.items.clone();
        self.runtime
            .reconcile(
                root_id,
                vec![
                    Description::new(
                        "btn-inc",
                        (),
                        |_, model: &mut AppModel, action: AppAction| {
                            if let AppAction::Increment = action {
                                model.counter += 1;
                                Dirty::BUILD
                            } else {
                                Dirty::NONE
                            }
                        },
                    )
                    .role(Role::Button)
                    .name("Increment Counter")
                    .semantic_actions(vec![SemanticAction::Click])
                    .focusable(true)
                    .tab_index(0)
                    .layout(LogicalRect::from_xywh(10.0, 10.0, 120.0, 32.0)),
                    Description::new("item-list", (), |_, _, _: AppAction| Dirty::NONE)
                        .structure(Structure::Declarative)
                        .role(Role::Container)
                        .layout(LogicalRect::from_xywh(10.0, 50.0, 300.0, 200.0)),
                ],
            )
            .map_err(view_platform::PlatformError::Core)?;

        // Flush so item-list node gets created and assigned NodeId
        self.runtime
            .flush(&mut self.model)
            .map_err(view_platform::PlatformError::Core)?;

        let list_node = self.find_node_key("item-list").unwrap_or(root_id);
        let item_descriptions: Vec<_> = initial_items
            .into_iter()
            .map(|name| {
                Description::new(name.clone(), (), |_, _, _: AppAction| Dirty::NONE)
                    .role(Role::StaticText)
                    .name(name)
            })
            .collect();
        self.runtime
            .reconcile(list_node, item_descriptions)
            .map_err(view_platform::PlatformError::Core)?;

        self.runtime
            .flush(&mut self.model)
            .map_err(view_platform::PlatformError::Core)?;

        let attributes = PlatformWindow::default_attributes(
            "Inspectable App (M1B)",
            LogicalSize::new(800.0, 600.0),
        )
        .with_visible(!self.run_hidden);

        shell.create_window(event_loop, win_id, attributes)?;

        println!(
            "Inspectable app initialized; window_id={:?}, root_id={:?}, hidden={}",
            win_id, root_id, self.run_hidden
        );

        Ok(())
    }

    fn on_window_event(
        &mut self,
        _shell: &mut PlatformShell,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: PlatformWindowEvent,
    ) -> Result<(), view_platform::PlatformError> {
        match event {
            PlatformWindowEvent::CloseRequested => {
                println!("Close requested by OS/user; exiting cleanly.");
                event_loop.exit();
            }
            PlatformWindowEvent::Pointer(pe) => {
                if let Err(e) = self.runtime.route_pointer(window_id, pe, &mut self.model) {
                    eprintln!("Error routing pointer: {e}");
                }
            }
            PlatformWindowEvent::Keyboard(ke) => {
                let _ = self.runtime.route_keyboard(window_id, ke, &mut self.model);
            }
            _ => {}
        }
        Ok(())
    }

    fn on_idle(
        &mut self,
        shell: &mut PlatformShell,
        event_loop: &ActiveEventLoop,
    ) -> Result<(), view_platform::PlatformError> {
        self.idle_ticks += 1;

        if !self.steps_completed
            && let Err(e) = self.run_contract_steps(shell)
        {
            eprintln!("Contract verification failed: {e}");
            event_loop.exit();
            return Err(view_platform::PlatformError::EventLoop(e.to_string()));
        }

        if self.run_hidden && self.steps_completed {
            println!("All M1B contract verifications passed in hidden mode! Exiting.");
            event_loop.exit();
        }

        Ok(())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let run_hidden = args.iter().any(|arg| arg == "--run-hidden");

    let config = AppConfig {
        run_hidden,
        max_iterations: if run_hidden { Some(20) } else { None },
    };

    println!("Starting Inspectable App (run_hidden={run_hidden})...");
    let handler = InspectableAppHandler::new(run_hidden)?;
    let app = PlatformApp::new(handler, config)?;
    app.run()?;

    println!("Inspectable App exited successfully.");
    Ok(())
}
