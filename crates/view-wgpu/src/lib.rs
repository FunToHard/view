//! SDR 2D rendering and an optional basic 3D scene provider.
//! 2D coordinates are logical pixels until preparation; scenes use app world units.
//! GPU objects are scoped to a context; prepared frames own their resources.
#![forbid(unsafe_code)]

mod context;
/// Bounded CPU-side SDR image document operations and deterministic exports.
pub mod document;
mod paint;
mod provider;
mod renderer;
/// Optional minimal opaque 3D scene provider, using the shared device and session.
#[cfg(feature = "scene")]
pub mod scene;
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
