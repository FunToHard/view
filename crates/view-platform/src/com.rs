//! COM library initialization and thread affinity containment for Windows.

#[cfg(windows)]
use std::thread::ThreadId;

#[cfg(windows)]
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
#[cfg(windows)]
use windows::core::HRESULT;

use crate::error::PlatformError;

/// RAII guard ensuring single-threaded COM apartment initialization on Windows.
pub struct ComGuard {
    #[cfg(windows)]
    thread_id: ThreadId,
    #[cfg(windows)]
    needs_uninit: bool,
}

impl ComGuard {
    /// Initialize COM with single-threaded apartment concurrency on the calling thread.
    pub fn new() -> Result<Self, PlatformError> {
        #[cfg(windows)]
        {
            let current_thread = std::thread::current().id();
            // SAFETY: CoInitializeEx initializes the COM library for the current thread.
            // pvReserved must be null. We handle S_OK, S_FALSE, and RPC_E_CHANGED_MODE safely.
            let hr: HRESULT = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };

            // S_OK (0) or S_FALSE (1): successfully initialized or already initialized on this thread.
            let needs_uninit = if hr.is_ok() {
                true
            } else if hr.0 == 1 {
                // S_FALSE: already initialized on this thread. CoUninitialize still balances it.
                true
            } else if hr.0 as u32 == 0x80010106 {
                // RPC_E_CHANGED_MODE: thread was already initialized in another concurrency model (e.g. MTA).
                // Do not uninitialize in drop since we did not initialize it.
                false
            } else {
                return Err(PlatformError::ComError(format!(
                    "CoInitializeEx failed with HRESULT 0x{:08X}",
                    hr.0 as u32
                )));
            };

            Ok(Self {
                thread_id: current_thread,
                needs_uninit,
            })
        }

        #[cfg(not(windows))]
        {
            Ok(Self {})
        }
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        #[cfg(windows)]
        {
            if self.needs_uninit {
                // Verify thread affinity before uninitializing.
                if std::thread::current().id() == self.thread_id {
                    // SAFETY: CoUninitialize closes the COM library on the current thread,
                    // balancing a previous successful call to CoInitializeEx.
                    unsafe {
                        CoUninitialize();
                    }
                }
            }
        }
    }
}
