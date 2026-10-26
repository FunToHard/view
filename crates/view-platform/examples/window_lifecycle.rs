//! Hidden Windows native window lifecycle check; it sends no desktop input.
//!
//! Run on a Windows desktop with:
//! `cargo run -p view-platform --example window_lifecycle --locked`

#[cfg(windows)]
mod windows_test {
    use std::time::{Duration, Instant};

    use view_core::{ArenaHandle, ArenaId, Generation, LogicalSize, WindowId};
    use view_platform::{
        AppConfig, PlatformApp, PlatformError, PlatformHandler, PlatformShell, PlatformWindow,
        PlatformWindowEvent,
    };
    use winit::{
        event_loop::{ActiveEventLoop, ControlFlow},
        window::WindowAttributes,
    };

    fn window_id() -> WindowId {
        WindowId::from_handle(ArenaHandle::from_parts(
            ArenaId::new(std::num::NonZeroU64::new(41).unwrap()),
            0,
            Generation::new(std::num::NonZeroU64::new(1).unwrap()),
        ))
    }

    #[derive(Default)]
    struct LifecycleCheck {
        stage: u8,
        started: Option<Instant>,
    }

    impl LifecycleCheck {
        fn tick(
            &mut self,
            shell: &mut PlatformShell,
            event_loop: &ActiveEventLoop,
        ) -> Result<(), PlatformError> {
            let Some(window) = shell.get_window(window_id()) else {
                return Err(PlatformError::EventLoop(
                    "hidden lifecycle window disappeared".into(),
                ));
            };
            let raw = window.shared_window();
            let started = *self.started.get_or_insert_with(Instant::now);
            if started.elapsed() > Duration::from_secs(8) {
                return Err(PlatformError::EventLoop(format!(
                    "native window lifecycle timed out at stage {}",
                    self.stage
                )));
            }

            match self.stage {
                0 if raw.is_minimized() == Some(true) => {
                    raw.set_minimized(false);
                    raw.set_maximized(true);
                    self.stage = 1;
                    println!("native window minimized and restored from minimized state");
                }
                1 if raw.is_maximized() => {
                    raw.set_maximized(false);
                    self.stage = 2;
                    println!("native window maximized; restore requested");
                }
                2 if !raw.is_maximized() && raw.is_minimized() == Some(false) => {
                    if raw.is_visible() != Some(false) {
                        return Err(PlatformError::EventLoop(
                            "lifecycle fixture unexpectedly became visible".into(),
                        ));
                    }
                    self.stage = 3;
                    println!(
                        "PASS hidden native window: decorated={}, resizable={}, minimized/maximized/restored=true",
                        raw.is_decorated(),
                        raw.is_resizable()
                    );
                    event_loop.exit();
                }
                3 => {}
                _ => event_loop.set_control_flow(ControlFlow::WaitUntil(
                    Instant::now() + Duration::from_millis(50),
                )),
            }
            Ok(())
        }
    }

    impl PlatformHandler for LifecycleCheck {
        fn on_ready(
            &mut self,
            shell: &mut PlatformShell,
            event_loop: &ActiveEventLoop,
        ) -> Result<(), PlatformError> {
            let attributes: WindowAttributes = PlatformWindow::default_attributes(
                "view hidden native lifecycle fixture",
                LogicalSize::new(320.0, 240.0),
            )
            .with_visible(false);
            let window = shell.create_window(event_loop, window_id(), attributes)?;
            let raw = window.shared_window();
            if raw.is_visible() != Some(false) {
                return Err(PlatformError::EventLoop(
                    "fixture window was not hidden".into(),
                ));
            }
            if !raw.is_decorated() || !raw.is_resizable() {
                return Err(PlatformError::EventLoop(format!(
                    "native frame properties mismatch: decorated={}, resizable={}",
                    raw.is_decorated(),
                    raw.is_resizable()
                )));
            }
            raw.set_minimized(true);
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                Instant::now() + Duration::from_millis(50),
            ));
            Ok(())
        }

        fn on_window_event(
            &mut self,
            _shell: &mut PlatformShell,
            event_loop: &ActiveEventLoop,
            _window_id: WindowId,
            event: PlatformWindowEvent,
        ) -> Result<(), PlatformError> {
            if matches!(event, PlatformWindowEvent::CloseRequested) {
                event_loop.exit();
                return Err(PlatformError::EventLoop(
                    "hidden lifecycle window received a close request".into(),
                ));
            }
            Ok(())
        }

        fn on_idle(
            &mut self,
            shell: &mut PlatformShell,
            event_loop: &ActiveEventLoop,
        ) -> Result<(), PlatformError> {
            self.tick(shell, event_loop)
        }
    }

    pub fn run() -> Result<(), PlatformError> {
        let app = PlatformApp::new(
            LifecycleCheck::default(),
            AppConfig {
                run_hidden: true,
                max_iterations: None,
            },
        )?;
        app.run()
    }
}

#[cfg(windows)]
fn main() -> Result<(), view_platform::PlatformError> {
    windows_test::run()
}

#[cfg(not(windows))]
fn main() {
    eprintln!("window_lifecycle requires Windows native window support");
    std::process::exit(2);
}
