//! view — The devil's framework.
//!
//! A native Rust UI framework in early development. The selected design combines
//! retained UI, on-demand immediate composition, and wgpu-backed 2D/3D rendering.
//!
//! A headless owner-thread runtime supports retained/keyed/immediate ownership,
//! typed actions, cancellation, demand scheduling and committed tree snapshots.
//! Geometry, constraints layout, input routing, focus, scrolling, semantics,
//! and native platform integration are implemented. Controls and rendering are in progress.

#![forbid(unsafe_code)]

/// Optional backend-independent text foundations (not a complete text control).
#[cfg(feature = "text")]
pub use view_text as text;

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
