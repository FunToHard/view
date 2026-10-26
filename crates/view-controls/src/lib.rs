//! Controls reuse core identities and shared editing; native and GPU adapters
//! remain explicit. Profile reports distinguish implementation from certification.
#![forbid(unsafe_code)]
mod behavior;
mod collections;
mod docking;
mod password;
mod popup;
mod presentation;
mod select;
mod table;
mod tabs;
mod text;
mod theme;
mod widgets;
pub use behavior::*;
pub use collections::*;
pub use docking::*;
pub use password::*;
pub use popup::*;
pub use presentation::*;
pub use select::*;
pub use table::*;
pub use tabs::*;
pub use text::*;
pub use theme::*;
pub use view_text;
#[cfg(feature = "accessibility")]
pub mod accessibility;
pub use widgets::*;
#[cfg(feature = "render")]
mod paint;
#[cfg(feature = "render")]
pub use paint::*;

#[derive(Clone, Debug, PartialEq)]
pub enum ControlError {
    Invalid(&'static str),
    Disabled,
    Stale,
    Text(view_text::TextError),
}
impl From<view_text::TextError> for ControlError {
    fn from(e: view_text::TextError) -> Self {
        Self::Text(e)
    }
}
impl std::fmt::Display for ControlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for ControlError {}
