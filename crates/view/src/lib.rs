//! view — The devil's framework.
//!
//! A native Rust UI framework in early development. The selected design combines
//! retained UI, on-demand immediate composition, and wgpu-backed 2D/3D rendering.
//!
//! A headless owner-thread runtime supports retained/keyed/immediate ownership,
//! typed actions, cancellation, demand scheduling and committed tree snapshots.
//! Geometry, constraints layout, input routing, focus, scrolling, semantics,
//! and native platform integration are implemented. Optional preview controls and 2D rendering are implemented; native certification remains open.

#![forbid(unsafe_code)]

/// Backend-independent canvas transforms and coherent draw/interaction records.
pub use view_core::canvas;
/// Host service handoff and committed semantics bridge.
pub use view_core::host;
/// Independent frame demand and app-owned drag transactions.
pub use view_core::viewport;

/// Optional backend-independent text foundations (not a complete text control).
#[cfg(feature = "text")]
pub use view_text as text;

/// Optional wgpu 30 renderer. Core-only consumers do not acquire GPU dependencies.
#[cfg(feature = "wgpu")]
pub use view_wgpu as render;

/// Optional basic 3D camera, meshes, materials and depth-tested provider.
#[cfg(feature = "scene")]
pub use view_wgpu::scene;

/// Optional standard controls sharing runtime identity and text sessions.
#[cfg(feature = "controls")]
pub use view_controls as controls;

pub use view_core::{
    Alignment, Axis, Constraints, CrossAxisAlignment, FlexItem, FlexLayout, LayoutCache,
    MainAxisAlignment, PaddingLayout, StackLayout,
};
pub use view_core::{ArenaHandle, ArenaId, CoreError, Generation, Revision, WindowId};
pub use view_core::{
    Cancellation, CompletionToken, Counters, Demand, Description, Dirty, Enqueued, Flush, NodeId,
    NodeSnapshot, Rejected, ResourceKind, Runtime, Snapshot, Structure, Trace, Visibility,
};
pub use view_core::{ClipChain, HitNodeEntry, HitTestContract, HitTestResult, TransformChain};
pub use view_core::{
    DocumentPoint, DocumentRect, DocumentSize, EPSILON, LogicalInsets, LogicalPoint, LogicalRect,
    LogicalSize, PhysicalPoint, PhysicalRect, PhysicalSize, ScaleFactor, Transform2D,
};
pub use view_core::{FocusDirection, FocusManager, FocusableRegistration, Shortcut};
pub use view_core::{
    Key, KeyPhase, KeyboardEvent, Modifiers, NamedKey, PointerButton, PointerEvent, PointerPhase,
    PointerRouter, RoutedPointerAction,
};
pub use view_core::{
    Role, SemanticAction, SemanticContract, SemanticNode, SemanticSnapshot, SemanticUpdate,
};
pub use view_core::{ScrollChaining, ScrollState, VirtualScroll};
