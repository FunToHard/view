use std::{
    any::{Any, TypeId},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use crate::{ArenaHandle, CoreError, Revision, WindowId};

/// Runtime node identity; copying it does not retain the node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeId(pub(crate) ArenaHandle);
impl NodeId {
    /// Handle for diagnostics, not proof of liveness.
    pub fn handle(self) -> ArenaHandle {
        self.0
    }
}

/// The exclusive authority allowed to change a node's direct children.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Structure {
    /// Explicit mount/remove operations.
    Retained,
    /// Keyed description reconciliation.
    Declarative,
    /// Registered on-demand region builder.
    Immediate,
}

/// Visibility/scheduling state; none of these destroys component state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Visibility {
    /// Participates normally.
    #[default]
    Visible,
    /// Hidden from visible output, but remains mounted.
    Hidden,
    /// Outside the current clip, but remains mounted.
    Clipped,
    /// Immediate builds are suspended until resumed (including descendants).
    Suspended,
}

/// Conservative invalidation causes, not promises of renderer optimizations.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Dirty(pub(crate) u8);
impl Dirty {
    /// No pending work.
    pub const NONE: Self = Self(0);
    /// Description/region construction.
    pub const BUILD: Self = Self(1);
    /// Measurement and arrangement; conservatively invalidates the whole window.
    pub const LAYOUT: Self = Self(2);
    /// Paint records.
    pub const PAINT: Self = Self(4);
    /// Composition and derived hit/semantic placement.
    pub const COMPOSITE: Self = Self(8);
    /// Semantic records.
    pub const SEMANTICS: Self = Self(16);
    /// Independently requested viewport output.
    pub const VIEWPORT: Self = Self(32);
    /// All categories.
    pub const ALL: Self = Self(63);
    /// Whether all supplied categories are present.
    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
    /// Combine causes.
    pub fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
    /// Whether any work is pending.
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
}

pub(crate) type Handler<M, A> = Box<dyn FnMut(&mut dyn Any, &mut M, A) -> Dirty>;
pub(crate) type Builder<M, A> = Box<dyn FnMut(&M, &[Revision]) -> Vec<Description<M, A>>>;

/// A keyed component description. Its Rust state type is its component type.
///
/// Use distinct state newtypes for distinct component kinds. Compatible
/// parent/key/type/structure matches retain state and replace the owned handler.
/// Descriptions express direct children only; retained descendants remain owned
/// by their mount and are never reconciled by an enclosing region.
pub struct Description<M, A> {
    pub(crate) key: String,
    pub(crate) kind: TypeId,
    pub(crate) state: Box<dyn Any>,
    pub(crate) handler: Handler<M, A>,
    pub(crate) structure: Structure,
}
impl<M, A> Description<M, A> {
    /// Describe owned local state and a handler borrowing the current model only
    /// during dispatch. Neither callback nor state may contain borrowed data.
    ///
    /// ```compile_fail
    /// use view_core::{Description, Dirty};
    /// let text = String::from("borrowed");
    /// let description: Description<(), ()> =
    ///     Description::new("node", text.as_str(), |_, _, _| Dirty::NONE);
    /// drop(description);
    /// ```
    pub fn new<S: 'static>(
        key: impl Into<String>,
        state: S,
        mut handler: impl FnMut(&mut S, &mut M, A) -> Dirty + 'static,
    ) -> Self {
        Self {
            key: key.into(),
            kind: TypeId::of::<S>(),
            state: Box::new(state),
            handler: Box::new(move |state, model, action| {
                handler(
                    state.downcast_mut::<S>().expect("matching component type"),
                    model,
                    action,
                )
            }),
            structure: Structure::Retained,
        }
    }
    /// Select direct-child ownership; changing this resets the component.
    pub fn structure(mut self, structure: Structure) -> Self {
        self.structure = structure;
        self
    }
}

/// Failed enqueue retains the original required action for retry.
#[derive(Debug)]
pub struct Rejected<A> {
    /// Reason no action was accepted.
    pub error: CoreError,
    /// Original action, not silently discarded.
    pub action: A,
}

/// Successful enqueue, including any explicitly superseded preview.
#[derive(Debug)]
pub struct Enqueued<A> {
    /// Ordered input sequence.
    pub sequence: Revision,
    /// Replaced preview; required actions are never coalesced.
    pub replaced: Option<A>,
}

/// Owner-scoped resource category for inspection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceKind {
    /// Owned subscription.
    Subscription,
    /// Background request/task.
    Task,
    /// External callback registration.
    Callback,
}

/// Sendable cancellation observation for a worker. No runtime access is exposed.
#[derive(Clone, Debug)]
pub struct Cancellation(pub(crate) Arc<AtomicBool>);
impl Cancellation {
    /// True after cancellation, successful consumption or owner teardown.
    pub fn is_revoked(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

/// Opaque owner/request incarnation. Cloneable for workers, validated on dispatch.
#[derive(Clone, Debug)]
pub struct CompletionToken {
    pub(crate) owner: NodeId,
    pub(crate) lease: ArenaHandle,
    pub(crate) cancellation: Cancellation,
}
impl CompletionToken {
    /// Worker-visible revocation flag.
    pub fn cancellation(&self) -> Cancellation {
        self.cancellation.clone()
    }
}

/// Immutable structural observation from a completed headless commit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeSnapshot {
    /// Live node at this revision.
    pub id: NodeId,
    /// Owning window.
    pub window: WindowId,
    /// Structural parent, absent only for a window root.
    pub parent: Option<NodeId>,
    /// Children in declaration/mount order.
    pub children: Vec<NodeId>,
    /// Parent-scoped application key.
    pub key: String,
    /// Visibility at commit.
    pub visibility: Visibility,
    /// Exclusive structural authority.
    pub structure: Structure,
    /// Resource registrations grouped by category.
    pub resources: Vec<ResourceKind>,
}

/// A coherent headless tree snapshot; not geometry, semantics or GPU presentation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    /// Runtime commit revision.
    pub revision: Revision,
    /// Nodes in deterministic arena slot order.
    pub nodes: Vec<NodeSnapshot>,
}

/// Per-window work request, coalesced until consumed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Demand {
    /// UI changes or a runnable immediate region need a commit.
    pub ui: bool,
    /// A viewport frame is requested independently of UI work.
    pub viewport: bool,
}

/// Saturating diagnostic counters; not used for identity or correctness.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counters {
    /// Successfully dispatched actions.
    pub actions: u64,
    /// Region builder calls.
    pub builds: u64,
    /// Published headless commits.
    pub commits: u64,
    /// Flushes that did no work.
    pub idle_polls: u64,
    /// Mounted nodes.
    pub mounts: u64,
    /// Removed nodes.
    pub unmounts: u64,
}

/// Optional bounded observation; no global reporter or callback is installed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trace {
    /// Node mounted.
    Mounted(NodeId),
    /// Node removed.
    Unmounted(NodeId),
    /// Ordered action executed.
    Action(NodeId, Revision),
    /// Stale queued action discarded with explicit evidence.
    Rejected(NodeId, Revision),
    /// Dirty propagation requested.
    Invalidated(NodeId, Dirty),
    /// Snapshot published.
    Committed(Revision),
}

/// Result of one owner-thread drain/build/commit turn.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Flush {
    /// A full region-response buffer prevented further ordered dispatch. Resume
    /// the suspended region or drain its builder before retrying pending input.
    pub backpressure: bool,
    /// Actions applied exactly once during this call.
    pub dispatched: usize,
    /// Actions revoked before dispatch; model was not called.
    pub rejected: Vec<(Revision, CoreError)>,
    /// New commit, or none for an idle turn.
    pub committed: Option<Revision>,
}
