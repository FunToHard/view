//! Monitor work-area queries for restoring native windows without covering taskbars.
use crate::PlatformError;

/// Query the nearest monitor's work area as `[x, y, width, height]` in physical
/// desktop pixels, including negative secondary-monitor coordinates. Windows only;
/// other platforms report unsupported rather than substitute full monitor bounds.
/// Call from a per-monitor-DPI-aware thread (as initialized by the winit shell).
/// Other awareness modes are rejected; this query never changes host DPI policy.
/// No window, monitor handle or pointer ownership is transferred to the caller.
pub fn work_area_at(point: [i32; 2]) -> Result<[i32; 4], PlatformError> {
    #[cfg(windows)]
    {
        use windows::Win32::{
            Foundation::POINT,
            Graphics::Gdi::{
                GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint,
            },
            UI::HiDpi::{
                DPI_AWARENESS_PER_MONITOR_AWARE, GetAwarenessFromDpiAwarenessContext,
                GetThreadDpiAwarenessContext,
            },
        };
        // SAFETY: These queries inspect only the calling thread's DPI context.
        // The borrowed context token is consumed immediately and no state changes.
        let awareness =
            unsafe { GetAwarenessFromDpiAwarenessContext(GetThreadDpiAwarenessContext()) };
        if awareness != DPI_AWARENESS_PER_MONITOR_AWARE {
            return Err(PlatformError::UnsupportedCapability(
                "physical monitor coordinates require a per-monitor-DPI-aware thread".into(),
            ));
        }
        // SAFETY: MonitorFromPoint accepts a by-value point and returns a borrowed
        // system monitor handle. It is only used for this immediate query and is
        // never closed, retained or treated as an owned graphics resource.
        let monitor = unsafe {
            MonitorFromPoint(
                POINT {
                    x: point[0],
                    y: point[1],
                },
                MONITOR_DEFAULTTONEAREST,
            )
        };
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        // SAFETY: info is correctly sized initialized writable storage; the
        // borrowed monitor handle comes from MonitorFromPoint above. Windows
        // writes only for the duration of this call; no pointer escapes.
        if !unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
            return Err(PlatformError::WindowCreation(
                "monitor work-area query failed".into(),
            ));
        }
        checked_bounds([
            info.rcWork.left,
            info.rcWork.top,
            info.rcWork.right,
            info.rcWork.bottom,
        ])
    }
    #[cfg(not(windows))]
    {
        let _ = point;
        Err(PlatformError::UnsupportedCapability(
            "monitor work-area query".into(),
        ))
    }
}

#[cfg(any(windows, test))]
fn checked_bounds(edges: [i32; 4]) -> Result<[i32; 4], PlatformError> {
    let width = edges[2].checked_sub(edges[0]).filter(|n| *n > 0);
    let height = edges[3].checked_sub(edges[1]).filter(|n| *n > 0);
    match (width, height) {
        (Some(width), Some(height)) => Ok([edges[0], edges[1], width, height]),
        _ => Err(PlatformError::WindowCreation(
            "invalid monitor work-area bounds".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn negative_monitor_and_invalid_edges_are_checked() {
        assert_eq!(
            checked_bounds([-1920, -200, 0, 840]).unwrap(),
            [-1920, -200, 1920, 1040]
        );
        assert!(checked_bounds([0, 0, 0, 10]).is_err());
        assert!(checked_bounds([0, 10, 100, 0]).is_err());
        assert!(checked_bounds([i32::MIN, 0, i32::MAX, 10]).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn unaware_test_thread_is_rejected_before_any_monitor_query() {
        use windows::Win32::UI::HiDpi::{
            DPI_AWARENESS_CONTEXT, DPI_AWARENESS_CONTEXT_UNAWARE, SetThreadDpiAwarenessContext,
        };
        struct Restore(DPI_AWARENESS_CONTEXT);
        impl Drop for Restore {
            fn drop(&mut self) {
                // SAFETY: The token was returned by the same thread's setter
                // below. Restore happens on that thread without pumping events
                // or constructing windows; no process-wide policy is changed.
                let _ = unsafe { SetThreadDpiAwarenessContext(self.0) };
            }
        }
        // SAFETY: Only this isolated test thread's context is changed. The guard
        // restores its borrowed prior token on success or unwinding. This test
        // creates no native window and needs no attached monitor or input.
        let previous = unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_UNAWARE) };
        assert!(!previous.0.is_null());
        let _restore = Restore(previous);
        assert!(matches!(
            work_area_at([0, 0]),
            Err(PlatformError::UnsupportedCapability(_))
        ));
    }
}
