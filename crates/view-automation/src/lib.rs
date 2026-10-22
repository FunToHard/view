//! Versioned, opt-in automation. All mutations enter the harness owner thread.
//! Transport adapters do not receive access to Rust state or arbitrary evaluation.
#![forbid(unsafe_code)]

pub mod desktop;
pub mod inspector;
pub mod protocol;
pub mod session;
pub use protocol::*;
pub use session::*;
