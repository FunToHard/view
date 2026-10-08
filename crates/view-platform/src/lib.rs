//! Native platform adapter for the `view` framework using `winit` and Windows bindings.
//!
//! Windows 11 x64 is the primary target; Linux and macOS are supported as secondary
//! platform targets via winit abstraction. Screen coordinates and COM/Win32 FFI are
//! strictly isolated within this crate and never leak into `view-core`.

#![deny(missing_docs)]

pub mod app;
pub mod com;
pub mod dpi;
pub mod error;
pub mod events;
pub mod services;
pub mod shell;
#[cfg(feature = "text")]
pub mod text;
pub mod window;

pub use app::{AppConfig, PlatformApp, PlatformHandler};
pub use com::ComGuard;
pub use dpi::DpiState;
pub use error::PlatformError;
pub use events::{ImeEvent, InputStateTracker, PlatformWindowEvent, translate_window_event};
pub use services::{CursorIcon, CursorService, ImeService};
pub use shell::{PlatformShell, WindowStateSnapshot};
pub use window::{PlatformWindow, WindowLifecycle};
