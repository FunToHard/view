//! Platform qualification probe for Windows windowing dependencies.
//!
//! Validates `winit` (0.30.13) and Microsoft `windows` (0.62.2) bindings
//! for Windows platform integration under Rust 1.98.1.
//!
//! This probe compiles version-specific windowing and platform contracts
//! without creating an on-screen window, calling runtime Win32 functions with
//! invalid handles, or injecting physical input.

#[cfg(windows)]
mod windows_probe {
    use std::num::NonZeroIsize;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
    use windows::Win32::Graphics::Dwm::{
        DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWINDOWATTRIBUTE, DwmDefWindowProc,
        DwmExtendFrameIntoClientArea, DwmSetWindowAttribute,
    };
    use windows::Win32::UI::Controls::MARGINS;
    use windows::Win32::UI::HiDpi::GetDpiForWindow;
    use windows::Win32::UI::WindowsAndMessaging::DefWindowProcW;
    use windows::core::BOOL;
    use winit::event_loop::EventLoop;
    use winit::platform::windows::EventLoopBuilderExtWindows;
    use winit::raw_window_handle::{RawWindowHandle, Win32WindowHandle};

    /// Converts a Win32 raw handle into a typed Microsoft `HWND`.
    #[must_use]
    pub fn raw_handle_to_hwnd(handle: &RawWindowHandle) -> Option<HWND> {
        match handle {
            RawWindowHandle::Win32(win32_handle) => {
                let ptr = win32_handle.hwnd.get() as *mut std::ffi::c_void;
                Some(HWND(ptr))
            }
            _ => None,
        }
    }

    /// Converts a typed Microsoft `HWND` to a `raw_window_handle` representation.
    #[must_use]
    pub fn hwnd_to_raw_handle(hwnd: HWND) -> Option<RawWindowHandle> {
        let addr = hwnd.0 as isize;
        let non_zero = NonZeroIsize::new(addr)?;
        let mut handle = Win32WindowHandle::new(non_zero);
        handle.hinstance = None;
        Some(RawWindowHandle::Win32(handle))
    }

    /// Verifies compile-time type signatures of Win32 and DWM APIs required
    /// for native window integration without calling the OS with invalid handles.
    #[inline]
    pub fn assert_win32_signatures_compile() {
        let _def_window_proc: unsafe fn(HWND, u32, WPARAM, LPARAM) -> LRESULT = DefWindowProcW;
        let _dwm_def_window_proc: unsafe fn(HWND, u32, WPARAM, LPARAM, *mut LRESULT) -> BOOL =
            DwmDefWindowProc;
        let _dwm_extend_frame: unsafe fn(HWND, *const MARGINS) -> windows::core::Result<()> =
            DwmExtendFrameIntoClientArea;
        let _dwm_set_window_attribute: unsafe fn(
            HWND,
            DWMWINDOWATTRIBUTE,
            *const std::ffi::c_void,
            u32,
        ) -> windows::core::Result<()> = DwmSetWindowAttribute;
        let _get_dpi_for_window: unsafe fn(HWND) -> u32 = GetDpiForWindow;

        let _ = (
            _def_window_proc,
            _dwm_def_window_proc,
            _dwm_extend_frame,
            _dwm_set_window_attribute,
            _get_dpi_for_window,
        );
    }

    /// Validates memory layout and constant values for platform integration structs.
    pub fn check_layout_and_constants() -> Result<(), String> {
        if std::mem::size_of::<MARGINS>() != 16 {
            return Err(format!(
                "Unexpected MARGINS struct size: {}",
                std::mem::size_of::<MARGINS>()
            ));
        }
        if std::mem::align_of::<MARGINS>() != 4 {
            return Err(format!(
                "Unexpected MARGINS struct alignment: {}",
                std::mem::align_of::<MARGINS>()
            ));
        }
        if std::mem::size_of::<RECT>() != 16 {
            return Err(format!(
                "Unexpected RECT struct size: {}",
                std::mem::size_of::<RECT>()
            ));
        }
        if std::mem::size_of::<HWND>() != std::mem::size_of::<usize>() {
            return Err(format!(
                "Unexpected HWND struct size: {}",
                std::mem::size_of::<HWND>()
            ));
        }

        // Verify DWMWA_USE_IMMERSIVE_DARK_MODE attribute code (20)
        let dark_mode_code = DWMWA_USE_IMMERSIVE_DARK_MODE.0;
        if dark_mode_code != 20 {
            return Err(format!(
                "Unexpected DWMWA_USE_IMMERSIVE_DARK_MODE attribute code: {dark_mode_code}"
            ));
        }

        Ok(())
    }

    /// Probes whether Windows event loop building can be configured without seizing the UI thread.
    pub fn check_winit_event_loop_builder() -> Result<(), String> {
        let mut builder = EventLoop::builder();
        builder.with_any_thread(true);
        Ok(())
    }
}

#[cfg(windows)]
pub use windows_probe::*;

#[cfg(not(windows))]
pub fn check_layout_and_constants() -> Result<(), String> {
    Err("Windows platform probe is not supported on non-Windows host OS.".into())
}

#[cfg(not(windows))]
pub fn check_winit_event_loop_builder() -> Result<(), String> {
    Err("Windows platform probe is not supported on non-Windows host OS.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    use std::num::NonZeroIsize;
    #[cfg(windows)]
    use winit::raw_window_handle::{RawWindowHandle, Win32WindowHandle};

    #[test]
    #[cfg(windows)]
    fn test_handle_conversion_round_trip() {
        let raw_addr = 0x1234_5678_isize;
        let non_zero = NonZeroIsize::new(raw_addr).unwrap();
        let win32_handle = Win32WindowHandle::new(non_zero);
        let raw_handle = RawWindowHandle::Win32(win32_handle);

        let hwnd = raw_handle_to_hwnd(&raw_handle).expect("Conversion to HWND failed");
        assert_eq!(hwnd.0 as isize, raw_addr);

        let round_trip =
            hwnd_to_raw_handle(hwnd).expect("Conversion back to RawWindowHandle failed");
        if let RawWindowHandle::Win32(rt_win32) = round_trip {
            assert_eq!(rt_win32.hwnd.get(), raw_addr);
        } else {
            panic!("Expected Win32 handle type");
        }
    }

    #[test]
    #[cfg(windows)]
    fn test_platform_type_layout_and_constants() {
        assert!(check_layout_and_constants().is_ok());
    }

    #[test]
    #[cfg(windows)]
    fn test_compile_time_win32_signatures() {
        assert_win32_signatures_compile();
    }

    #[test]
    #[cfg(windows)]
    fn test_event_loop_builder_with_any_thread() {
        assert!(check_winit_event_loop_builder().is_ok());
    }

    #[test]
    #[cfg(not(windows))]
    fn test_non_windows_skipped() {
        assert!(check_layout_and_constants().is_err());
        assert!(check_winit_event_loop_builder().is_err());
    }
}
