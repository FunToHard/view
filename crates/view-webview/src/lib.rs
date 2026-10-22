//! Optional rectangular WebView2 panels. The host pumps lifecycle work on its UI thread.
//! CPU providers prove contracts, not browser/native support.
#![cfg_attr(not(all(windows, feature = "native")), forbid(unsafe_code))]
mod bridge;
mod manager;
mod profile;
pub use bridge::*;
pub use manager::*;
pub use profile::*;
#[cfg(all(windows, feature = "native"))]
pub mod native;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Unsupported(String),
    Invalid(String),
    Stale,
    Busy,
    Budget,
    Denied,
    Native(String),
    Io(String),
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
pub type Result<T> = std::result::Result<T, Error>;
