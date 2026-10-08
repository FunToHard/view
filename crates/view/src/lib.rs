//! view — The devil's framework.
//!
//! A native Rust UI framework in early development. The selected design combines
//! retained UI, on-demand immediate composition, and wgpu-backed 2D/3D rendering.
//!
//! A headless owner-thread runtime supports retained/keyed/immediate ownership,
//! typed actions, cancellation, demand scheduling and committed tree snapshots.
//! Native input, layout, semantics, controls and rendering are not implemented yet.

#![forbid(unsafe_code)]

pub use view_core::{ArenaHandle, ArenaId, CoreError, Generation, Revision, WindowId};
pub use view_core::{
    Cancellation, CompletionToken, Counters, Demand, Description, Dirty, Enqueued, Flush, NodeId,
    NodeSnapshot, Rejected, ResourceKind, Runtime, Snapshot, Structure, Trace, Visibility,
};
