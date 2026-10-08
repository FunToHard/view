//! Explicit hidden native regression for application callback failures.
use view_core::{Description, Dirty, LogicalSize, Runtime, WindowId};
use view_platform::{
    AppConfig, PlatformApp, PlatformError, PlatformHandler, PlatformShell, PlatformWindow,
    PlatformWindowEvent,
};
use winit::event_loop::ActiveEventLoop;

struct FailingHandler {
    stage: String,
    runtime: Runtime<(), ()>,
}
impl FailingHandler {
    fn fail(&self, stage: &str) -> Result<(), PlatformError> {
        if self.stage == stage {
            Err(PlatformError::EventLoop(format!("expected-{stage}")))
        } else {
            Ok(())
        }
    }
}
impl PlatformHandler for FailingHandler {
    fn on_ready(
        &mut self,
        shell: &mut PlatformShell,
        event_loop: &ActiveEventLoop,
    ) -> Result<(), PlatformError> {
        self.fail("ready")?;
        let (id, _) = self
            .runtime
            .open_window(Description::new("root", (), |_, _, _| Dirty::NONE))
            .expect("fixture root");
        shell.create_window(
            event_loop,
            id,
            PlatformWindow::default_attributes(
                "hidden callback regression",
                LogicalSize::new(100.0, 100.0),
            )
            .with_visible(false),
        )?;
        // Explicitly exit if the failure is swallowed: main must then reject Ok.
        Ok(())
    }
    fn on_window_event(
        &mut self,
        _: &mut PlatformShell,
        event_loop: &ActiveEventLoop,
        _: WindowId,
        _: PlatformWindowEvent,
    ) -> Result<(), PlatformError> {
        if self.stage == "event" {
            event_loop.exit();
        }
        self.fail("event")
    }
    fn on_idle(
        &mut self,
        _: &mut PlatformShell,
        event_loop: &ActiveEventLoop,
    ) -> Result<(), PlatformError> {
        event_loop.exit();
        self.fail("idle")
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if !args.iter().any(|arg| arg == "--run-hidden") {
        return Err("explicit --run-hidden required".into());
    }
    let stage = args
        .last()
        .filter(|s| ["ready", "idle", "event"].contains(&s.as_str()))
        .ok_or("expected ready/idle/event stage")?
        .clone();
    let handler = FailingHandler {
        stage: stage.clone(),
        runtime: Runtime::new(1)?,
    };
    let outcome = PlatformApp::new(
        handler,
        AppConfig {
            run_hidden: true,
            max_iterations: Some(5),
        },
    )?
    .run();
    match outcome {
        Err(PlatformError::EventLoop(message)) if message == format!("expected-{stage}") => {
            println!("Verified callback error propagation: {stage}");
            Ok(())
        }
        other => Err(format!("Expected callback failure, received {other:?}").into()),
    }
}
