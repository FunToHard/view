//! Error types for native platform integration.

use std::fmt;
use view_core::{CoreError, WindowId};

/// Errors encountered in the platform layer.
#[derive(Debug)]
#[non_exhaustive]
pub enum PlatformError {
    /// Window creation failed.
    WindowCreation(String),
    /// Requested platform capability is unsupported on this system.
    UnsupportedCapability(String),
    /// COM library initialization failed.
    ComError(String),
    /// Thread affinity violated (e.g. platform operation called from non-UI thread).
    ThreadAffinityViolation,
    /// Unknown or destroyed window referenced.
    InvalidWindow(WindowId),
    /// Modal relationship violation or cycle.
    ModalViolation(String),
    /// Event loop execution error.
    EventLoop(String),
    /// Underlying core runtime error.
    Core(CoreError),
}

impl fmt::Display for PlatformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WindowCreation(err) => write!(f, "window creation failed: {err}"),
            Self::UnsupportedCapability(cap) => write!(f, "unsupported capability: {cap}"),
            Self::ComError(err) => write!(f, "COM initialization failed: {err}"),
            Self::ThreadAffinityViolation => {
                f.write_str("platform call made from incorrect thread")
            }
            Self::InvalidWindow(id) => write!(f, "invalid window: {id:?}"),
            Self::ModalViolation(err) => write!(f, "modal violation: {err}"),
            Self::EventLoop(err) => write!(f, "event loop error: {err}"),
            Self::Core(err) => write!(f, "core error: {err}"),
        }
    }
}

impl std::error::Error for PlatformError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Core(err) => Some(err),
            _ => None,
        }
    }
}

impl From<CoreError> for PlatformError {
    fn from(err: CoreError) -> Self {
        Self::Core(err)
    }
}
