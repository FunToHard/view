use std::{
    any::{Any, TypeId},
    collections::{HashSet, VecDeque},
    marker::PhantomData,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use crate::{Arena, ArenaHandle, CoreError, Revision, WindowId, runtime_types::*};

struct Node<M, A> {
    window: WindowId,
    parent: Option<NodeId>,
    children: Vec<NodeId>,
    key: String,
    kind: TypeId,
    state: Box<dyn Any>,
    handler: Handler<M, A>,
    structure: Structure,
    visibility: Visibility,
    dirty: Dirty,
    region_pending: bool,
    builder: Option<Builder<M, A>>,
    responses: VecDeque<Revision>,
    leases: Vec<ArenaHandle>,
}

struct Window {
    root: Option<NodeId>,
    demand: Demand,
    presented: Revision,
}

struct Lease {
    owner: NodeId,
    key: Option<u64>,
    kind: ResourceKind,
    cancellation: Cancellation,
    cancel: Option<Box<dyn FnOnce()>>,
    queued: bool,
}

struct Queued<A> {
    owner: NodeId,
    action: A,
    sequence: Revision,
    preview: Option<u64>,
    completion: Option<ArenaHandle>,
}

/// Single-owner headless runtime with explicit application model borrows.
///
/// The runtime is neither Send nor Sync. It stores only owned callbacks/state;
/// workers return completion tokens/actions to an application-managed transport.
/// `flush` runs effects once, then region descriptions and a structural commit.
/// Panicking user callbacks are fatal to that turn; there is no panic recovery or
/// rollback guarantee. Build callbacks must be pure with respect to app effects.
///
/// ```compile_fail
/// use view_core::Runtime;
/// let runtime = Runtime::<(), ()>::new(8).unwrap();
/// std::thread::spawn(move || drop(runtime));
/// ```
pub struct Runtime<M, A> {
    nodes: Arena<Node<M, A>>,
    windows: Arena<Window>,
    leases: Arena<Lease>,
    queue: VecDeque<Queued<A>>,
    capacity: usize,
    sequence: Revision,
    snapshot: Snapshot,
    changed: bool,
    counters: Counters,
    trace: VecDeque<Trace>,
    trace_capacity: usize,
    _owner_thread: PhantomData<Rc<()>>,
}

impl<M, A> Runtime<M, A> {
    /// Create an idle runtime with a bounded required-action queue.
    /// Zero capacity is allowed and rejects every enqueue.
    pub fn new(queue_capacity: usize) -> Result<Self, CoreError> {
        Ok(Self {
            nodes: Arena::new()?,
            windows: Arena::new()?,
            leases: Arena::new()?,
            queue: VecDeque::new(),
            capacity: queue_capacity,
            sequence: Revision::INITIAL,
            snapshot: Snapshot::default(),
            changed: false,
            counters: Counters::default(),
            trace: VecDeque::new(),
            trace_capacity: 0,
            _owner_thread: PhantomData,
        })
    }

    fn record(&mut self, event: Trace) {
        if self.trace_capacity == 0 {
            return;
        }
        if self.trace.len() == self.trace_capacity {
            self.trace.pop_front();
        }
        self.trace.push_back(event);
    }

    /// Enable a bounded observer buffer; zero disables and clears it.
    pub fn observe(&mut self, capacity: usize) {
        self.trace_capacity = capacity;
        while self.trace.len() > capacity {
            self.trace.pop_front();
        }
    }

    /// Drain retained breadcrumbs in occurrence order.
    pub fn drain_trace(&mut self) -> impl Iterator<Item = Trace> + '_ {
        self.trace.drain(..)
    }

    /// Current saturating instrumentation counters.
    pub fn counters(&self) -> Counters {
        self.counters
    }

    /// Last fully published tree. Mutations remain invisible here until commit.
    pub fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }

    /// Number of accepted actions awaiting dispatch.
    pub fn pending_actions(&self) -> usize {
        self.queue.len()
    }

    /// Whether publication, input or a runnable region is pending, including
    /// teardown of the last window which has no per-window demand.
    pub fn has_work(&self) -> bool {
        self.changed
            || !self.queue.is_empty()
            || self.windows.iter().any(|(id, _)| {
                self.demand(WindowId::from_handle(id))
                    .is_ok_and(|demand| demand.ui)
            })
    }

    /// Register a logical window and its root; no native window is created.
    pub fn open_window(
        &mut self,
        root: Description<M, A>,
    ) -> Result<(WindowId, NodeId), CoreError> {
        let window = WindowId::from_handle(self.windows.insert(Window {
            root: None,
            demand: Demand {
                ui: true,
                viewport: false,
            },
            presented: Revision::INITIAL,
        })?);
        match self.insert(window, None, root) {
            Ok(id) => {
                self.windows.get_mut(window.handle())?.root = Some(id);
                Ok((window, id))
            }
            Err(error) => {
                self.windows.remove(window.handle())?;
                Err(error)
            }
        }
    }

    fn insert(
        &mut self,
        window: WindowId,
        parent: Option<NodeId>,
        description: Description<M, A>,
    ) -> Result<NodeId, CoreError> {
        let id = NodeId(self.nodes.insert(Node {
            window,
            parent,
            children: Vec::new(),
            key: description.key,
            kind: description.kind,
            state: description.state,
            handler: description.handler,
            structure: description.structure,
            visibility: Visibility::Visible,
            dirty: Dirty::ALL,
            region_pending: true,
            builder: None,
            responses: VecDeque::new(),
            leases: Vec::new(),
        })?);
        self.changed = true;
        self.windows.get_mut(window.handle())?.demand.ui = true;
        self.counters.mounts = self.counters.mounts.saturating_add(1);
        self.record(Trace::Mounted(id));
        Ok(id)
    }

    /// Mount into a retained parent. Invalid/stale parents fail before insertion.
    pub fn mount(
        &mut self,
        parent: NodeId,
        description: Description<M, A>,
    ) -> Result<NodeId, CoreError> {
        let node = self.nodes.get(parent.0)?;
        if node.structure != Structure::Retained {
            return Err(CoreError::StructuralOwnership);
        }
        if node.children.iter().any(|id| {
            self.nodes
                .get(id.0)
                .is_ok_and(|child| child.key == description.key)
        }) {
            return Err(CoreError::DuplicateKey);
        }
        let id = self.insert(node.window, Some(parent), description)?;
        self.nodes.get_mut(parent.0)?.children.push(id);
        self.invalidate(parent, Dirty::LAYOUT)?;
        Ok(id)
    }

    /// Remove a retained mount. Declarative/immediate children are removed only
    /// by their structural owner; window roots are removed by `close_window`.
    pub fn unmount(&mut self, id: NodeId) -> Result<(), CoreError> {
        let parent = self
            .nodes
            .get(id.0)?
            .parent
            .ok_or(CoreError::StructuralOwnership)?;
        if self.nodes.get(parent.0)?.structure != Structure::Retained {
            return Err(CoreError::StructuralOwnership);
        }
        self.nodes
            .get_mut(parent.0)?
            .children
            .retain(|child| *child != id);
        self.remove_subtree(id)?;
        self.invalidate(parent, Dirty::LAYOUT)
    }

    fn remove_subtree(&mut self, id: NodeId) -> Result<(), CoreError> {
        // Iterative postorder avoids stack overflow for deeply nested ownership.
        let mut pending = vec![(id, false)];
        while let Some((current, visited)) = pending.pop() {
            if !visited {
                pending.push((current, true));
                for child in self.nodes.get(current.0)?.children.iter().rev() {
                    pending.push((*child, false));
                }
                continue;
            }
            let node = self.nodes.remove(current.0)?;
            for lease in node.leases {
                self.revoke(lease, true)?;
            }
            self.changed = true;
            self.counters.unmounts = self.counters.unmounts.saturating_add(1);
            self.record(Trace::Unmounted(current));
        }
        Ok(())
    }

    /// Tear down a logical window and every owned node/resource.
    pub fn close_window(&mut self, window: WindowId) -> Result<(), CoreError> {
        let root = self
            .windows
            .get(window.handle())?
            .root
            .expect("registered root");
        self.remove_subtree(root)?;
        self.windows.remove(window.handle())?;
        Ok(())
    }

    /// Inspect mounted state without effects or a mutable runtime capability.
    pub fn state<S: 'static>(&self, id: NodeId) -> Result<&S, CoreError> {
        self.nodes
            .get(id.0)?
            .state
            .downcast_ref()
            .ok_or(CoreError::StateTypeMismatch)
    }

    /// Retained property/state mutation with explicit invalidation.
    pub fn update<S: 'static, R>(
        &mut self,
        id: NodeId,
        dirty: Dirty,
        update: impl FnOnce(&mut S) -> R,
    ) -> Result<R, CoreError> {
        let state = self
            .nodes
            .get_mut(id.0)?
            .state
            .downcast_mut()
            .ok_or(CoreError::StateTypeMismatch)?;
        let result = update(state);
        self.changed = true;
        let window = self.nodes.get(id.0)?.window;
        self.windows.get_mut(window.handle())?.demand.ui = true;
        self.invalidate(id, dirty)?;
        Ok(result)
    }

    /// Hide/clip/suspend without disposing state, handlers or registrations.
    pub fn set_visibility(&mut self, id: NodeId, visibility: Visibility) -> Result<(), CoreError> {
        self.nodes.get_mut(id.0)?.visibility = visibility;
        self.invalidate(id, Dirty::LAYOUT.union(Dirty::SEMANTICS))?;
        self.changed = true;
        Ok(())
    }

    fn suspended(&self, mut id: NodeId) -> bool {
        loop {
            let Ok(node) = self.nodes.get(id.0) else {
                return true;
            };
            if node.visibility == Visibility::Suspended {
                return true;
            }
            match node.parent {
                Some(parent) => id = parent,
                None => return false,
            }
        }
    }

    fn response_region(&self, mut id: NodeId) -> Option<NodeId> {
        loop {
            let node = self.nodes.get(id.0).ok()?;
            if node.builder.is_some() {
                return Some(id);
            }
            id = node.parent?;
        }
    }

    /// Propagate dependencies conservatively. Layout invalidates all nodes in
    /// the same window, covering ancestors and size-dependent siblings.
    pub fn invalidate(&mut self, id: NodeId, dirty: Dirty) -> Result<(), CoreError> {
        let window = self.nodes.get(id.0)?.window;
        let mut expanded = dirty;
        if dirty.contains(Dirty::BUILD) {
            expanded = expanded.union(Dirty::LAYOUT);
        }
        if expanded.contains(Dirty::LAYOUT) {
            expanded = expanded
                .union(Dirty::PAINT)
                .union(Dirty::COMPOSITE)
                .union(Dirty::SEMANTICS);
        }
        if expanded.contains(Dirty::COMPOSITE) {
            expanded = expanded.union(Dirty::SEMANTICS);
        }
        if expanded.contains(Dirty::LAYOUT) {
            let ids: Vec<_> = self
                .nodes
                .iter()
                .filter(|(_, n)| n.window == window)
                .map(|(id, _)| id)
                .collect();
            let layout = Dirty::LAYOUT
                .union(Dirty::PAINT)
                .union(Dirty::COMPOSITE)
                .union(Dirty::SEMANTICS);
            for sibling in ids {
                let node = self.nodes.get_mut(sibling)?;
                node.dirty = node.dirty.union(layout);
            }
        }
        let node = self.nodes.get_mut(id.0)?;
        node.dirty = node.dirty.union(expanded);
        if dirty.contains(Dirty::BUILD) {
            node.region_pending = true;
        }
        if dirty.contains(Dirty::BUILD)
            && let Some(region) = self.response_region(id)
        {
            self.nodes.get_mut(region.0)?.region_pending = true;
        }
        let demand = &mut self.windows.get_mut(window.handle())?.demand;
        if dirty.contains(Dirty::VIEWPORT) {
            demand.viewport = true;
        }
        if expanded.0 & !Dirty::VIEWPORT.0 != 0 {
            demand.ui = true;
            self.changed = true;
        }
        self.record(Trace::Invalidated(id, expanded));
        Ok(())
    }

    /// Current dirty causes for diagnostics before commit consumes them.
    pub fn dirty(&self, id: NodeId) -> Result<Dirty, CoreError> {
        Ok(self.nodes.get(id.0)?.dirty)
    }

    /// Reconcile one declarative parent's children by parent/key/state type.
    /// Duplicate keys are rejected before any mutation. Structure changes reset
    /// state as type changes do. Retained descendants of reused mounts survive.
    pub fn reconcile(
        &mut self,
        parent: NodeId,
        descriptions: Vec<Description<M, A>>,
    ) -> Result<Vec<NodeId>, CoreError> {
        if self.nodes.get(parent.0)?.structure != Structure::Declarative {
            return Err(CoreError::StructuralOwnership);
        }
        self.reconcile_owned(parent, descriptions)
    }

    fn reconcile_owned(
        &mut self,
        parent: NodeId,
        descriptions: Vec<Description<M, A>>,
    ) -> Result<Vec<NodeId>, CoreError> {
        let mut keys = HashSet::new();
        if descriptions
            .iter()
            .any(|description| !keys.insert(&description.key))
        {
            return Err(CoreError::DuplicateKey);
        }
        let owner = self.nodes.get(parent.0)?;
        let window = owner.window;
        let old = owner.children.clone();
        let mut children = Vec::with_capacity(descriptions.len());
        for description in descriptions {
            let existing = old.iter().copied().find(|id| {
                self.nodes.get(id.0).is_ok_and(|node| {
                    node.key == description.key
                        && node.kind == description.kind
                        && node.structure == description.structure
                })
            });
            let child = if let Some(id) = existing {
                self.nodes.get_mut(id.0)?.handler = description.handler;
                id
            } else {
                self.insert(window, Some(parent), description)?
            };
            children.push(child);
        }
        for id in old {
            if !children.contains(&id) {
                self.remove_subtree(id)?;
            }
        }
        self.nodes.get_mut(parent.0)?.children = children.clone();
        self.invalidate(parent, Dirty::LAYOUT)?;
        Ok(children)
    }

    /// Register an owned on-demand builder. It receives immutable current model
    /// access and ordered, once-consumed action response sequences.
    pub fn set_region_builder(
        &mut self,
        id: NodeId,
        builder: impl FnMut(&M, &[Revision]) -> Vec<Description<M, A>> + 'static,
    ) -> Result<(), CoreError> {
        let node = self.nodes.get_mut(id.0)?;
        if node.structure != Structure::Immediate {
            return Err(CoreError::StructuralOwnership);
        }
        node.builder = Some(Box::new(builder));
        self.invalidate(id, Dirty::BUILD)
    }

    /// Enqueue a required action. Full queues return the original action.
    pub fn enqueue(&mut self, owner: NodeId, action: A) -> Result<Enqueued<A>, Rejected<A>> {
        self.enqueue_inner(owner, action, None, None)
    }

    /// Enqueue a replaceable preview. Same-owner/key previews are replaced and
    /// moved to the newest sequence position; required actions stay ordered.
    pub fn enqueue_preview(
        &mut self,
        owner: NodeId,
        key: u64,
        action: A,
    ) -> Result<Enqueued<A>, Rejected<A>> {
        self.enqueue_inner(owner, action, Some(key), None)
    }

    fn enqueue_inner(
        &mut self,
        owner: NodeId,
        action: A,
        preview: Option<u64>,
        completion: Option<ArenaHandle>,
    ) -> Result<Enqueued<A>, Rejected<A>> {
        if let Err(error) = self.nodes.get(owner.0) {
            return Err(Rejected { error, action });
        }
        let replace = preview.and_then(|key| {
            self.queue
                .iter()
                .position(|queued| queued.owner == owner && queued.preview == Some(key))
        });
        if replace.is_none() && self.queue.len() >= self.capacity {
            return Err(Rejected {
                error: CoreError::QueueFull,
                action,
            });
        }
        let sequence = match self.sequence.checked_next() {
            Ok(next) => next,
            Err(error) => return Err(Rejected { error, action }),
        };
        let replaced = replace.map(|index| {
            self.queue
                .remove(index)
                .expect("matched queue index")
                .action
        });
        self.sequence = sequence;
        self.queue.push_back(Queued {
            owner,
            action,
            sequence,
            preview,
            completion,
        });
        Ok(Enqueued { sequence, replaced })
    }

    /// Register a revocation hook tied to owner removal or explicit cancellation.
    /// The hook runs on the owner thread; it should be short and must not panic.
    pub fn own(
        &mut self,
        owner: NodeId,
        kind: ResourceKind,
        cancel: impl FnOnce() + 'static,
    ) -> Result<CompletionToken, CoreError> {
        self.add_lease(owner, None, kind, Box::new(cancel))
    }

    fn add_lease(
        &mut self,
        owner: NodeId,
        key: Option<u64>,
        kind: ResourceKind,
        cancel: Box<dyn FnOnce()>,
    ) -> Result<CompletionToken, CoreError> {
        self.nodes.get(owner.0)?;
        let cancellation = Cancellation(Arc::new(AtomicBool::new(false)));
        let lease = self.leases.insert(Lease {
            owner,
            key,
            kind,
            cancellation: cancellation.clone(),
            cancel: Some(cancel),
            queued: false,
        })?;
        self.nodes.get_mut(owner.0)?.leases.push(lease);
        self.changed = true;
        let window = self.nodes.get(owner.0)?.window;
        self.windows.get_mut(window.handle())?.demand.ui = true;
        Ok(CompletionToken {
            owner,
            lease,
            cancellation,
        })
    }

    /// Start/supersede a named task. Its lease generation is the request version.
    pub fn start_request(
        &mut self,
        owner: NodeId,
        key: u64,
        cancel: impl FnOnce() + 'static,
    ) -> Result<CompletionToken, CoreError> {
        let old: Vec<_> = self
            .nodes
            .get(owner.0)?
            .leases
            .iter()
            .copied()
            .filter(|id| {
                self.leases
                    .get(*id)
                    .is_ok_and(|lease| lease.key == Some(key))
            })
            .collect();
        for id in old {
            self.revoke(id, true)?;
        }
        self.add_lease(owner, Some(key), ResourceKind::Task, Box::new(cancel))
    }

    fn revoke(&mut self, id: ArenaHandle, cancelled: bool) -> Result<(), CoreError> {
        let mut lease = self.leases.remove(id)?;
        lease.cancellation.0.store(true, Ordering::Release);
        if let Ok(node) = self.nodes.get_mut(lease.owner.0) {
            node.leases.retain(|owned| *owned != id);
            if let Ok(window) = self.windows.get_mut(node.window.handle()) {
                window.demand.ui = true;
            }
        }
        self.changed = true;
        if cancelled && let Some(cancel) = lease.cancel.take() {
            cancel();
        }
        Ok(())
    }

    /// Explicitly revoke a task/subscription/callback and its queued completion.
    pub fn cancel(&mut self, token: &CompletionToken) -> Result<(), CoreError> {
        self.validate_token(token)?;
        self.revoke(token.lease, true)
    }

    fn validate_token(&self, token: &CompletionToken) -> Result<&Lease, CoreError> {
        self.nodes.get(token.owner.0)?;
        let lease = self
            .leases
            .get(token.lease)
            .map_err(|_| CoreError::InactiveRequest)?;
        if lease.owner != token.owner || token.cancellation.is_revoked() {
            return Err(CoreError::InactiveRequest);
        }
        Ok(lease)
    }

    /// Accept a worker result once, then revalidate at dispatch. A full queue
    /// leaves the request active so the caller may retry the returned action.
    pub fn complete(
        &mut self,
        token: &CompletionToken,
        action: A,
    ) -> Result<Enqueued<A>, Rejected<A>> {
        let error = match self.validate_token(token) {
            Ok(lease) if !lease.queued => None,
            Ok(_) => Some(CoreError::InactiveRequest),
            Err(error) => Some(error),
        };
        if let Some(error) = error {
            return Err(Rejected { error, action });
        }
        let receipt = self.enqueue_inner(token.owner, action, None, Some(token.lease))?;
        self.leases
            .get_mut(token.lease)
            .expect("validated lease")
            .queued = true;
        Ok(receipt)
    }

    /// Coalesced work demand. Suspended regions retain pending builds without
    /// causing continuous idle wakeups; resuming them restores demand.
    pub fn demand(&self, window: WindowId) -> Result<Demand, CoreError> {
        let mut demand = self.windows.get(window.handle())?.demand;
        demand.ui |= self.queue.iter().any(|queued| {
            self.nodes
                .get(queued.owner.0)
                .is_ok_and(|node| node.window == window)
        });
        demand.ui |= self.nodes.iter().any(|(id, node)| {
            node.window == window
                && node.builder.is_some()
                && node.region_pending
                && !self.suspended(NodeId(id))
        });
        Ok(demand)
    }

    /// Request viewport work without rebuilding the UI.
    pub fn request_viewport(&mut self, window: WindowId) -> Result<(), CoreError> {
        self.windows.get_mut(window.handle())?.demand.viewport = true;
        Ok(())
    }

    /// Consume a coalesced viewport request. This does not claim GPU presentation.
    pub fn take_viewport_request(&mut self, window: WindowId) -> Result<bool, CoreError> {
        Ok(std::mem::take(
            &mut self.windows.get_mut(window.handle())?.demand.viewport,
        ))
    }

    /// Record an adapter's acknowledgement at its documented presentation boundary.
    /// This runtime cannot establish actual native display/presentation itself.
    pub fn acknowledge_presentation(
        &mut self,
        window: WindowId,
        revision: Revision,
    ) -> Result<(), CoreError> {
        let target = self.windows.get_mut(window.handle())?;
        if revision > self.snapshot.revision || revision < target.presented {
            return Err(CoreError::InvalidPresentation);
        }
        target.presented = revision;
        Ok(())
    }

    /// Last adapter-acknowledged revision, initially zero.
    pub fn presented(&self, window: WindowId) -> Result<Revision, CoreError> {
        Ok(self.windows.get(window.handle())?.presented)
    }

    /// Read-only measurement/inspection boundary; repeating it cannot dispatch
    /// runtime actions. User code must not hide external effects in this closure.
    pub fn measure<R>(&self, measure: impl FnOnce(&Snapshot) -> R) -> R {
        measure(&self.snapshot)
    }

    /// Drain accepted actions once, build dirty regions, publish one coherent
    /// structural snapshot. Queued actions revoked before dispatch are reported.
    /// No native input routing, layout or rendering is claimed by this commit.
    pub fn flush(&mut self, model: &mut M) -> Result<Flush, CoreError> {
        // Reserve the commit number before running any irreversible callback.
        let next = self.snapshot.revision.checked_next()?;
        let mut result = Flush::default();
        while let Some(front) = self.queue.front() {
            // A suspended region may retain responses; preserve backpressure.
            let region = self.response_region(front.owner);
            if region.is_some_and(|id| {
                self.nodes
                    .get(id.0)
                    .is_ok_and(|node| node.responses.len() >= self.capacity)
            }) {
                result.backpressure = true;
                break;
            }
            let queued = self.queue.pop_front().expect("front exists");
            let invalid = self.nodes.get(queued.owner.0).err().or_else(|| {
                queued.completion.and_then(|id| {
                    self.leases
                        .get(id)
                        .err()
                        .map(|_| CoreError::InactiveRequest)
                })
            });
            if let Some(error) = invalid {
                result.rejected.push((queued.sequence, error));
                self.record(Trace::Rejected(queued.owner, queued.sequence));
                continue;
            }
            if let Some(id) = queued.completion {
                self.revoke(id, false)?;
            }
            let node = self.nodes.get_mut(queued.owner.0)?;
            let dirty = (node.handler)(node.state.as_mut(), model, queued.action);
            if let Some(region) = region {
                let region = self.nodes.get_mut(region.0)?;
                region.responses.push_back(queued.sequence);
                region.region_pending = true;
            }
            self.changed = true;
            self.invalidate(queued.owner, dirty)?;
            self.counters.actions = self.counters.actions.saturating_add(1);
            result.dispatched += 1;
            self.record(Trace::Action(queued.owner, queued.sequence));
        }
        // Parent-before-child traversal prevents building a child removed by its parent.
        let mut pending: Vec<NodeId> = self
            .windows
            .iter()
            .filter_map(|(_, window)| window.root)
            .collect();
        pending.reverse();
        while let Some(id) = pending.pop() {
            if self.suspended(id) {
                continue;
            }
            let node = self.nodes.get_mut(id.0)?;
            if node.region_pending
                && let Some(mut builder) = node.builder.take()
            {
                let responses: Vec<_> = node.responses.drain(..).collect();
                // Consume before calling user code: failed descriptions never replay effects.
                node.region_pending = false;
                let descriptions = builder(model, &responses);
                self.nodes.get_mut(id.0)?.builder = Some(builder);
                self.counters.builds = self.counters.builds.saturating_add(1);
                if let Err(error) = self.reconcile_owned(id, descriptions) {
                    self.nodes.get_mut(id.0)?.region_pending = true;
                    return Err(error);
                }
            }
            pending.extend(self.nodes.get(id.0)?.children.iter().rev().copied());
        }
        if self.changed {
            let nodes = self
                .nodes
                .iter()
                .map(|(id, node)| NodeSnapshot {
                    id: NodeId(id),
                    window: node.window,
                    parent: node.parent,
                    children: node.children.clone(),
                    key: node.key.clone(),
                    visibility: node.visibility,
                    structure: node.structure,
                    resources: node
                        .leases
                        .iter()
                        .filter_map(|lease| self.leases.get(*lease).ok().map(|lease| lease.kind))
                        .collect(),
                })
                .collect();
            self.snapshot = Snapshot {
                revision: next,
                nodes,
            };
            let ids: Vec<_> = self.nodes.iter().map(|(id, _)| id).collect();
            for id in ids {
                self.nodes.get_mut(id)?.dirty = Dirty::NONE;
            }
            let windows: Vec<_> = self.windows.iter().map(|(id, _)| id).collect();
            for id in windows {
                self.windows.get_mut(id)?.demand.ui = false;
            }
            self.changed = false;
            self.counters.commits = self.counters.commits.saturating_add(1);
            self.record(Trace::Committed(next));
            result.committed = Some(next);
        } else if result.dispatched == 0 && result.rejected.is_empty() {
            self.counters.idle_polls = self.counters.idle_polls.saturating_add(1);
        }
        Ok(result)
    }
}

impl<M, A> Drop for Runtime<M, A> {
    fn drop(&mut self) {
        // Mark every token revoked before running any user cancellation hook.
        let ids: Vec<_> = self
            .leases
            .iter()
            .map(|(id, lease)| {
                lease.cancellation.0.store(true, Ordering::Release);
                id
            })
            .collect();
        for id in ids {
            if let Ok(mut lease) = self.leases.remove(id)
                && let Some(cancel) = lease.cancel.take()
            {
                cancel();
            }
        }
    }
}
