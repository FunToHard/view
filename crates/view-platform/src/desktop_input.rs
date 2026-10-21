//! Native input is isolated here. Selection never changes the user's input language.
#[cfg(windows)]
pub use native::*;
#[cfg(windows)]
mod native {
    use std::{mem::size_of, sync::Arc};
    use view_automation::{
        Code, Error,
        desktop::{Driver, Event, Held, ScreenMap},
    };
    use windows::{
        Win32::{
            Foundation::*,
            Graphics::Gdi::ClientToScreen,
            System::Threading::*,
            UI::{HiDpi::GetDpiForWindow, Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
        },
        core::w,
    };
    use winit::{
        raw_window_handle::{HasWindowHandle, RawWindowHandle},
        window::Window,
    };

    /// Retains the selected window and a cross-process desktop semaphore.
    /// Raw numeric handles are private and validated for every input group.
    pub struct WindowsDriver {
        window: Arc<Window>,
        hwnd: isize,
        pid: u32,
        gate: usize,
    }
    impl WindowsDriver {
        /// Installed layout handles, for fixture prerequisites. Does not activate a layout.
        pub fn installed_layouts() -> Vec<u64> {
            // SAFETY: query count then fill a bounded owned output slice.
            unsafe {
                let count = GetKeyboardLayoutList(None).clamp(0, 256) as usize;
                let mut layouts = vec![HKL::default(); count];
                let written = GetKeyboardLayoutList(Some(&mut layouts)).max(0) as usize;
                layouts
                    .into_iter()
                    .take(written.min(count))
                    .map(|h| h.0 as usize as u64)
                    .collect()
            }
        }
        /// Attach only to the explicitly selected live foreground window.
        pub fn select(window: Arc<Window>, selected: bool) -> Result<Self, Error> {
            if !selected {
                return Err(Error::new(Code::Unauthorized, "desktop not selected"));
            }
            let hwnd = match window
                .window_handle()
                .map_err(|e| Error::new(Code::Host, e.to_string()))?
                .as_raw()
            {
                RawWindowHandle::Win32(h) => h.hwnd.get(),
                _ => return Err(Error::new(Code::Unsupported, "Win32 handle")),
            };
            // SAFETY: borrowed live winit window; scalar process output is valid.
            let pid = unsafe {
                let mut pid = 0;
                GetWindowThreadProcessId(HWND(hwnd as *mut _), Some(&mut pid));
                pid
            };
            // SAFETY: named process-independent semaphore has no borrowed storage.
            // Nonblocking acquisition, owned until Drop, can release on watchdog thread.
            let gate = unsafe { CreateSemaphoreW(None, 1, 1, w!("Local\\view-desktop-input-v1")) }
                .map_err(|e| Error::new(Code::Host, e.to_string()))?;
            // SAFETY: gate is a valid newly opened semaphore handle.
            if unsafe { WaitForSingleObject(gate, 0) } != WAIT_OBJECT_0 {
                // SAFETY: this handle has not been transferred or closed.
                let _ = unsafe { CloseHandle(gate) };
                return Err(Error::new(Code::Busy, "desktop owned by another runner"));
            }
            let driver = Self {
                window,
                hwnd,
                pid,
                gate: gate.0 as usize,
            };
            driver.check_target()?;
            if [0x10, 0x11, 0x12, 0x5b, 0x5c]
                .into_iter()
                .any(|key| driver.already_down(Held::Key(key)))
                || (0..3).any(|button| driver.already_down(Held::Button(button)))
            {
                return Err(Error::new(
                    Code::Busy,
                    "pre-existing modifiers or mouse buttons; runner will not release user input",
                ));
            }
            Ok(driver)
        }
        /// Query current client screen origin and DPI; caller revalidates the layout revision.
        pub fn screen_map(&self, revision: u64) -> Result<ScreenMap, Error> {
            self.check_target()?;
            let hwnd = HWND(self.hwnd as *mut _);
            let mut p = POINT::default();
            // SAFETY: target checked live; p is writable and lives through the call.
            unsafe {
                ClientToScreen(hwnd, &mut p)
                    .ok()
                    .map_err(|e| Error::new(Code::Host, e.to_string()))?;
                let dpi = GetDpiForWindow(hwnd);
                if dpi == 0 {
                    return Err(Error::new(Code::Host, "window DPI unavailable"));
                }
                Ok(ScreenMap {
                    origin: (p.x, p.y),
                    scale: dpi as f32 / 96.,
                    revision,
                })
            }
        }
        /// Active target keyboard layout, diagnostic only; never activates another layout.
        pub fn keyboard_layout(&self) -> u64 {
            // SAFETY: query-only calls; selected window is retained by Arc.
            unsafe {
                GetKeyboardLayout(GetWindowThreadProcessId(HWND(self.hwnd as *mut _), None)).0
                    as usize as u64
            }
        }
    }
    impl Drop for WindowsDriver {
        fn drop(&mut self) {
            // SAFETY: semaphore acquired exactly once in select; owned handle released/closed once.
            unsafe {
                let handle = HANDLE(self.gate as *mut _);
                let _ = ReleaseSemaphore(handle, 1, None);
                let _ = CloseHandle(handle);
            }
        }
    }
    impl Driver for WindowsDriver {
        fn check_target(&self) -> Result<(), Error> {
            let hwnd = HWND(self.hwnd as *mut _);
            let _keep_alive = &self.window;
            // SAFETY: Win32 query APIs accept handles and validate them; pid output is live.
            unsafe {
                let mut pid = 0;
                GetWindowThreadProcessId(hwnd, Some(&mut pid));
                if !IsWindow(Some(hwnd)).as_bool() || pid != self.pid {
                    return Err(Error::new(Code::Stale, "target closed or process changed"));
                }
                if GetForegroundWindow() != hwnd
                    || !IsWindowVisible(hwnd).as_bool()
                    || IsIconic(hwnd).as_bool()
                {
                    return Err(Error::new(
                        Code::Cancelled,
                        "foreground target changed or unavailable",
                    ));
                }
                if GetAsyncKeyState(VK_F12.0 as i32) < 0 {
                    return Err(Error::new(Code::Cancelled, "F12 stop requested"));
                }
            }
            Ok(())
        }
        fn already_down(&self, input: Held) -> bool {
            let vk = match input {
                Held::Key(k) => k,
                Held::Button(0) => VK_LBUTTON.0,
                Held::Button(1) => VK_RBUTTON.0,
                Held::Button(2) => VK_MBUTTON.0,
                Held::Button(_) => return true,
                Held::Unicode(_) => return false,
            };
            // SAFETY: scalar key-state query; no side effects.
            unsafe { GetAsyncKeyState(vk as i32) < 0 }
        }
        fn insert(&mut self, event: Event) -> Result<(), Error> {
            let mut input = INPUT::default();
            match event {
                Event::Move { x, y } => {
                    // SAFETY: read-only system metrics for the virtual desktop, including negative origins.
                    let (left, top, width, height) = unsafe {
                        (
                            GetSystemMetrics(SM_XVIRTUALSCREEN),
                            GetSystemMetrics(SM_YVIRTUALSCREEN),
                            GetSystemMetrics(SM_CXVIRTUALSCREEN),
                            GetSystemMetrics(SM_CYVIRTUALSCREEN),
                        )
                    };
                    if width <= 1
                        || height <= 1
                        || x < left
                        || y < top
                        || x as i64 >= left as i64 + width as i64
                        || y as i64 >= top as i64 + height as i64
                    {
                        return Err(Error::new(
                            Code::NotActionable,
                            "point outside virtual desktop",
                        ));
                    }
                    input.r#type = INPUT_MOUSE;
                    input.Anonymous.mi = MOUSEINPUT {
                        dx: (((x as i64 - left as i64) * 65535) / (width as i64 - 1)) as i32,
                        dy: (((y as i64 - top as i64) * 65535) / (height as i64 - 1)) as i32,
                        dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
                        ..Default::default()
                    };
                }
                Event::Wheel { horizontal, delta } => {
                    input.r#type = INPUT_MOUSE;
                    input.Anonymous.mi = MOUSEINPUT {
                        mouseData: delta as u32,
                        dwFlags: if horizontal {
                            MOUSEEVENTF_HWHEEL
                        } else {
                            MOUSEEVENTF_WHEEL
                        },
                        ..Default::default()
                    };
                }
                Event::Down(held) | Event::Up(held) => {
                    let up = matches!(event, Event::Up(_));
                    match held {
                        Held::Key(vk) => {
                            if vk == 0 || vk > 254 {
                                return Err(Error::new(Code::Invalid, "virtual key"));
                            }
                            input.r#type = INPUT_KEYBOARD;
                            input.Anonymous.ki = KEYBDINPUT {
                                wVk: VIRTUAL_KEY(vk),
                                dwFlags: if up {
                                    KEYEVENTF_KEYUP
                                } else {
                                    KEYBD_EVENT_FLAGS(0)
                                },
                                ..Default::default()
                            };
                        }
                        Held::Unicode(unit) => {
                            input.r#type = INPUT_KEYBOARD;
                            input.Anonymous.ki = KEYBDINPUT {
                                wScan: unit,
                                dwFlags: KEYEVENTF_UNICODE
                                    | if up {
                                        KEYEVENTF_KEYUP
                                    } else {
                                        KEYBD_EVENT_FLAGS(0)
                                    },
                                ..Default::default()
                            };
                        }
                        Held::Button(button) => {
                            let flags = match (button, up) {
                                (0, false) => MOUSEEVENTF_LEFTDOWN,
                                (0, true) => MOUSEEVENTF_LEFTUP,
                                (1, false) => MOUSEEVENTF_RIGHTDOWN,
                                (1, true) => MOUSEEVENTF_RIGHTUP,
                                (2, false) => MOUSEEVENTF_MIDDLEDOWN,
                                (2, true) => MOUSEEVENTF_MIDDLEUP,
                                _ => return Err(Error::new(Code::Invalid, "mouse button")),
                            };
                            input.r#type = INPUT_MOUSE;
                            input.Anonymous.mi = MOUSEINPUT {
                                dwFlags: flags,
                                ..Default::default()
                            };
                        }
                    }
                }
            }
            // SAFETY: exactly one initialized INPUT with matching active union/type and valid size.
            // Failure is not delivery. UIPI may reject without a specific diagnostic.
            if unsafe { SendInput(&[input], size_of::<INPUT>() as i32) } != 1 {
                return Err(Error::new(
                    Code::UnknownOutcome,
                    "SendInput rejected; integrity/desktop restrictions or interference possible",
                ));
            }
            Ok(())
        }
    }
}
