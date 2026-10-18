//! Independent frame demand and drag transactions over runtime-owned UI identity.

use crate::{
    CoreError, LogicalPoint, NodeId, PointerButton, Revision, RoutedPointerAction, Snapshot,
    Visibility,
};
use std::time::Duration;

/// Viewport cadence, driven by the host's monotonic clock (never a private loop).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FramePolicy {
    /// Render only after explicit invalidation or becoming visible.
    OnDemand,
    /// Render at most once per interval, coalescing missed ticks.
    Continuous(Duration),
}
struct Entry {
    owner: NodeId,
    policy: FramePolicy,
    requested: bool,
    visible: bool,
    due: Duration,
}
/// Bounded scheduler whose entries are pruned when their runtime owner disappears.
pub struct ViewportScheduler {
    entries: Vec<Entry>,
    capacity: usize,
    last: Duration,
}
impl ViewportScheduler {
    /// Create without starting a timer or thread.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
            last: Duration::ZERO,
        }
    }
    /// Register a mounted owner. Liveness/ancestor visibility is resolved at poll.
    pub fn register(&mut self, owner: NodeId, policy: FramePolicy) -> Result<(), CoreError> {
        if matches!(policy, FramePolicy::Continuous(d) if d.is_zero()) {
            return Err(CoreError::InvalidConstraints);
        }
        if self.entries.iter().any(|e| e.owner == owner) {
            return Err(CoreError::DuplicateKey);
        }
        if self.entries.len() == self.capacity {
            return Err(CoreError::CapacityExhausted);
        }
        self.entries.push(Entry {
            owner,
            policy,
            requested: true,
            visible: false,
            due: self.last,
        });
        Ok(())
    }
    /// Coalesce repeated invalidations; hidden work remains pending until visible.
    pub fn request(&mut self, owner: NodeId) -> Result<(), CoreError> {
        self.entries
            .iter_mut()
            .find(|e| e.owner == owner)
            .ok_or(CoreError::InactiveRequest)?
            .requested = true;
        Ok(())
    }
    /// Number of live registrations (after the most recent poll).
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    /// Whether all owners have been removed.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    /// Return due owners without touching UI build/layout. Supply a committed
    /// snapshot, host time and window-level visibility (e.g. not minimized).
    /// The host may retry failed frames using `request`.
    pub fn poll(
        &mut self,
        snapshot: &Snapshot,
        now: Duration,
        window_visible: impl Fn(crate::WindowId) -> bool,
    ) -> Result<Vec<NodeId>, CoreError> {
        if now < self.last {
            return Err(CoreError::InvalidConstraints);
        }
        // Check deadline arithmetic before consuming any request.
        for e in &self.entries {
            if let FramePolicy::Continuous(d) = e.policy {
                now.checked_add(d).ok_or(CoreError::RevisionExhausted)?;
            }
        }
        self.last = now;
        self.entries
            .retain(|e| snapshot.nodes.iter().any(|n| n.id == e.owner));
        let mut ready = Vec::new();
        for e in &mut self.entries {
            let node = snapshot
                .nodes
                .iter()
                .find(|n| n.id == e.owner)
                .expect("retained owner");
            let mut visible = window_visible(node.window)
                && node.bounds.size.width > 0.0
                && node.bounds.size.height > 0.0;
            let mut current = Some(node);
            while let Some(n) = current {
                visible &= n.visibility == Visibility::Visible;
                current = n
                    .parent
                    .and_then(|id| snapshot.nodes.iter().find(|p| p.id == id));
            }
            if visible && !e.visible {
                e.requested = true;
            }
            e.visible = visible;
            let continuous_due = matches!(e.policy, FramePolicy::Continuous(_) if now >= e.due);
            if visible && (e.requested || continuous_due) {
                ready.push(e.owner);
                e.requested = false;
                if let FramePolicy::Continuous(d) = e.policy {
                    e.due = now + d;
                }
            }
        }
        Ok(ready)
    }
    /// Next host wake-up after polling, or None when all visible work is idle.
    pub fn deadline(&self) -> Option<Duration> {
        self.entries
            .iter()
            .filter(|e| e.visible)
            .filter_map(|e| {
                if e.requested {
                    Some(self.last)
                } else if matches!(e.policy, FramePolicy::Continuous(_)) {
                    Some(e.due)
                } else {
                    None
                }
            })
            .min()
    }
}

/// One completed gesture, to enqueue as a single application undo command.
#[derive(Clone, Debug, PartialEq)]
pub struct DragCommit<T> {
    /// App-owned object ID.
    pub object: u64,
    /// Document revision against which the gesture began.
    pub revision: Revision,
    /// Original value.
    pub before: T,
    /// Last preview value.
    pub after: T,
}
/// Local preview state; use runtime pointer capture, never a second input router.
pub struct DragSession<T> {
    owner: NodeId,
    object: u64,
    revision: Revision,
    before: T,
    preview: T,
    finished: bool,
}
impl<T: Clone + PartialEq> DragSession<T> {
    /// Begin after a routed primary Down, then acquire runtime capture for owner.
    pub fn new(owner: NodeId, object: u64, revision: Revision, before: T) -> Self {
        Self {
            owner,
            object,
            revision,
            preview: before.clone(),
            before,
            finished: false,
        }
    }
    /// Current preview, without mutating the document.
    pub fn preview(&self) -> &T {
        &self.preview
    }
    /// Whether this gesture committed or was cancelled.
    pub fn finished(&self) -> bool {
        self.finished
    }
    /// Cancel on Escape, capture loss, owner removal, or changed document revision.
    pub fn cancel(&mut self) {
        self.preview = self.before.clone();
        self.finished = true;
    }
    /// Consume routed events exactly once. Returns at most one commit. Mapping
    /// uses local logical coordinates; transforms remain app/document-specific.
    pub fn route(
        &mut self,
        action: &RoutedPointerAction,
        revision: Revision,
        mut map: impl FnMut(LogicalPoint) -> T,
    ) -> Option<DragCommit<T>> {
        if self.finished {
            return None;
        }
        if revision != self.revision {
            self.cancel();
            return None;
        }
        match action {
            RoutedPointerAction::Cancel(id) if *id == self.owner => self.cancel(),
            RoutedPointerAction::Move { target, local } if *target == self.owner => {
                self.preview = map(*local)
            }
            RoutedPointerAction::Up {
                target,
                local,
                button: PointerButton::Primary,
            } if *target == self.owner => {
                self.preview = map(*local);
                self.finished = true;
                if self.preview != self.before {
                    return Some(DragCommit {
                        object: self.object,
                        revision,
                        before: self.before.clone(),
                        after: self.preview.clone(),
                    });
                }
            }
            _ => {}
        }
        None
    }
    /// Reconcile with committed owner lifetime/visibility, including ancestors.
    pub fn reconcile(&mut self, snapshot: &Snapshot) {
        let mut id = Some(self.owner);
        while let Some(current) = id {
            let Some(node) = snapshot.nodes.iter().find(|n| n.id == current) else {
                self.cancel();
                return;
            };
            if node.visibility != Visibility::Visible {
                self.cancel();
                return;
            }
            id = node.parent;
        }
    }
}
