//! Explicit native clipboard round-trip in a private noninteractive window station.
//! Never attaches to the interactive desktop; no input injection or user clipboard.

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use windows::{
        Win32::System::StationsAndDesktops::{
            CreateDesktopW, CreateWindowStationW, DESKTOP_CONTROL_FLAGS, SetProcessWindowStation,
            SetThreadDesktop,
        },
        core::{PCWSTR, w},
    };
    if !std::env::args().any(|arg| arg == "--isolated") {
        return Err("requires --isolated; no desktop clipboard access is permitted".into());
    }
    // SAFETY: standalone fixture process, before any window/event loop exists.
    // A unique create-only station isolates the clipboard. The desktop is never
    // switched to the foreground. Handles intentionally live until process exit,
    // which releases them after winit's hidden windows and thread resources.
    unsafe {
        // Let Windows choose the noninteractive station name: specifying a name
        // requires Administrator membership. CWF_CREATE_ONLY rejects an existing
        // station, so this fixture can never reuse another process's clipboard.
        let station = CreateWindowStationW(PCWSTR::null(), 1, 0x000f037f, None)
            .map_err(|e| format!("create station: {e}"))?;
        SetProcessWindowStation(station).map_err(|e| format!("select station: {e}"))?;
        let desktop = CreateDesktopW(
            w!("view-clipboard-fixture"),
            PCWSTR::null(),
            None,
            DESKTOP_CONTROL_FLAGS(0),
            0x000f01ff,
            None,
        )
        .map_err(|e| format!("create desktop: {e}"))?;
        SetThreadDesktop(desktop).map_err(|e| format!("select desktop: {e}"))?;
    }
    use view_text::Clipboard;
    use winit::{
        application::ApplicationHandler,
        event::WindowEvent,
        event_loop::{ActiveEventLoop, EventLoop},
        window::{Window, WindowId},
    };
    #[derive(Default)]
    struct App {
        result: Option<Result<(), String>>,
    }
    impl ApplicationHandler for App {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            self.result = Some((|| {
                let window = event_loop
                    .create_window(Window::default_attributes().with_visible(false))
                    .map_err(|e| e.to_string())?;
                let mut clipboard = view_platform::text::NativeClipboard::new(&window)
                    .map_err(|e| e.to_string())?;
                for value in ["a😀日本\r\nsecond line", "", "replacement"] {
                    clipboard.write(value).map_err(|e| e.to_string())?;
                    if clipboard.read().map_err(|e| e.to_string())? != value {
                        return Err("native clipboard round trip mismatch".into());
                    }
                }
                if clipboard.write("bad\0text").is_ok() {
                    return Err("embedded NUL accepted".into());
                }
                if clipboard.read().map_err(|e| e.to_string())? != "replacement" {
                    return Err("failed write changed clipboard".into());
                }
                Ok(())
            })());
            event_loop.exit();
        }
        fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}
    }
    let mut app = App::default();
    EventLoop::new()?.run_app(&mut app)?;
    app.result.ok_or("native callback never executed")??;
    println!("Verified native clipboard in isolated noninteractive window station");
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    println!("Windows clipboard fixture unsupported on this host");
}
