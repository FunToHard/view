//! Opt-in native HWND lifecycle smoke, separate from ordinary CPU tests.
#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use winit::application::ApplicationHandler;
    use winit::event::WindowEvent;
    use winit::event_loop::{ActiveEventLoop, EventLoop};
    use winit::raw_window_handle::HasWindowHandle;
    use winit::window::{Window, WindowId};

    if std::env::args().nth(1).as_deref() != Some("--run-hidden") {
        return Err("Use --run-hidden to opt into creating and closing a native window".into());
    }
    #[derive(Default)]
    struct App {
        window: Option<Window>,
        failure: Option<String>,
        created: bool,
    }
    impl ApplicationHandler for App {
        fn resumed(&mut self, events: &ActiveEventLoop) {
            match events.create_window(
                Window::default_attributes()
                    .with_title("view M0 native lifecycle probe")
                    .with_decorations(true)
                    .with_visible(false)
                    .with_active(false),
            ) {
                Ok(window) => {
                    match window.window_handle() {
                        Ok(handle)
                            if platform_probe::raw_handle_to_hwnd(&handle.as_raw()).is_some() =>
                        {
                            self.created = true;
                            println!(
                                "Native window created; decorated={}; scale={}; inner={:?}",
                                window.is_decorated(),
                                window.scale_factor(),
                                window.inner_size()
                            );
                        }
                        result => self.failure = Some(format!("Missing Win32 handle: {result:?}")),
                    }
                    self.window = Some(window);
                }
                Err(error) => self.failure = Some(error.to_string()),
            }
            events.exit();
        }
        fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}
        fn exiting(&mut self, _: &ActiveEventLoop) {
            self.window.take();
        }
    }
    let mut app = App::default();
    EventLoop::new()?.run_app(&mut app)?;
    if let Some(error) = app.failure {
        return Err(error.into());
    }
    if !app.created || app.window.is_some() {
        return Err("Incomplete window lifecycle".into());
    }
    println!(
        "PASS hidden native create/close; no displayed-frame, input, IME or UI runtime certification"
    );
    Ok(())
}

#[cfg(not(windows))]
fn main() -> std::process::ExitCode {
    eprintln!("SKIPPED: Windows-only native lifecycle smoke");
    std::process::ExitCode::from(2)
}
