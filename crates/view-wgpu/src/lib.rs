//! SDR 2D rendering. All coordinates are logical pixels until preparation.
//! GPU objects are scoped to a context; prepared frames own their resources.
#![forbid(unsafe_code)]

mod context;
mod paint;
mod provider;
mod renderer;
mod surface;
pub use context::*;
pub use paint::*;
pub use provider::*;
pub use renderer::*;
pub use surface::*;
pub use {glyphon, lyon, wgpu};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenderError {
    Unsupported(String),
    Invalid(String),
    StaleResource,
    BudgetExceeded,
    Device(String),
    SurfaceLost,
}
impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for RenderError {}
