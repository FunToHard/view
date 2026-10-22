//! Optional trusted native-process add-ons. API permissions are not an OS sandbox.
#![forbid(unsafe_code)]
mod host;
mod process;
mod protocol;
mod ui;
pub use host::*;
pub use process::*;
pub use protocol::*;
pub use ui::*;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Error {
    Invalid(String),
    Incompatible(String),
    Denied,
    Stale,
    Limit,
    Timeout,
    Disconnected,
    Io(String),
    Host(String),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}
impl From<view_core::CoreError> for Error {
    fn from(e: view_core::CoreError) -> Self {
        Self::Host(format!("{e:?}"))
    }
}
pub type Result<T> = std::result::Result<T, Error>;
