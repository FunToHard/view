use crate::*;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};
use view_core::{Cancellation, NodeId, Runtime};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Installed,
    Negotiating,
    Active,
    Deactivating,
    Failed,
    Disabled,
    Removed,
}
#[derive(Clone, Debug)]
pub struct Limits {
    pub nodes: usize,
    pub ui_bytes: usize,
    pub queue: usize,
    pub pending: usize,
    pub messages_per_second: usize,
    pub startup: Duration,
    pub task: Duration,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            nodes: 128,
            ui_bytes: 128 * 1024,
            queue: 32,
            pending: 32,
            messages_per_second: 256,
            startup: Duration::from_secs(5),
            task: Duration::from_secs(5),
        }
    }
}
/// Host-owned application model and undo policy; methods run only on the host UI thread.
/// Validate the revision and command arguments before committing any mutation.
pub trait Application {
    fn snapshot(&mut self) -> Result<DocumentSnapshot>;
    fn edit(
        &mut self,
        action: &str,
        revision: u64,
        arguments: serde_json::Value,
    ) -> Result<DocumentSnapshot>;
}
#[derive(Default)]
pub struct Grants {
    pub scopes: BTreeSet<Scope>,
    /// Host registered typed argument validators, not guest supplied schemas.
    pub actions: BTreeMap<String, fn(&serde_json::Value) -> Result<()>>,
    pub automation_commands: BTreeSet<String>,
}
#[derive(Clone, Debug)]
pub struct CommandInfo {
    pub name: String,
    pub generation: u64,
}
static GENERATION: AtomicU64 = AtomicU64::new(1);
pub struct Addon {
    manifest: Manifest,
    grants: Grants,
    registry: Registry,
    limits: Limits,
    selected: BTreeMap<String, String>,
    parents: BTreeMap<Slot, NodeId>,
    owner: Cancellation,
    state: State,
    generation: u64,
    revision: u64,
    process: Option<Process>,
    ui: MountedUi,
    nodes: BTreeMap<String, UiNode>,
    now: Duration,
    started: Duration,
    rate_second: u64,
    rate_count: usize,
    last_request: u64,
    next_event: u64,
    pending: BTreeMap<u64, Duration>,
    outbox: VecDeque<LocalEvent>,
    failure: Option<Error>,
}
impl Addon {
    pub fn install(
        manifest: Manifest,
        grants: Grants,
        registry: Registry,
        parents: BTreeMap<Slot, NodeId>,
        owner: Cancellation,
        limits: Limits,
    ) -> Result<Self> {
        manifest.validate()?;
        if !manifest.scopes.is_subset(&grants.scopes)
            || !manifest.slots.iter().all(|s| parents.contains_key(s))
        {
            return Err(Error::Denied);
        }
        if limits.nodes == 0
            || limits.nodes > 1024
            || limits.ui_bytes > MAX_FRAME
            || limits.queue == 0
            || limits.queue > 256
            || limits.pending == 0
            || limits.pending > 256
            || limits.messages_per_second == 0
            || limits.startup.is_zero()
            || limits.task.is_zero()
        {
            return Err(Error::Limit);
        }
        let selected = registry.negotiate(&manifest)?;
        Ok(Self {
            manifest,
            grants,
            registry,
            limits,
            selected,
            parents,
            owner,
            state: State::Installed,
            generation: 0,
            revision: 0,
            process: None,
            ui: MountedUi::default(),
            nodes: BTreeMap::new(),
            now: Duration::ZERO,
            started: Duration::ZERO,
            rate_second: 0,
            rate_count: 0,
            last_request: 0,
            next_event: 0,
            pending: BTreeMap::new(),
            outbox: VecDeque::new(),
            failure: None,
        })
    }
    pub fn state(&self) -> State {
        self.state
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn failure(&self) -> Option<&Error> {
        self.failure.as_ref()
    }
    pub fn ui(&self) -> &MountedUi {
        &self.ui
    }
    pub fn ui_mut(&mut self) -> &mut MountedUi {
        &mut self.ui
    }
    pub fn pending(&self) -> usize {
        self.pending.len() + self.outbox.len()
    }
    pub fn receipt(&self) -> Option<ProcessReceipt> {
        self.process.as_ref().map(Process::receipt)
    }
    pub fn diagnostics(&self) -> String {
        self.process
            .as_ref()
            .map(Process::diagnostics)
            .unwrap_or_default()
    }
    pub fn activate(&mut self) -> Result<()> {
        if !matches!(self.state, State::Installed | State::Failed) || self.owner.is_revoked() {
            return Err(Error::Stale);
        }
        if self.process.as_ref().is_some_and(|p| {
            let r = p.receipt();
            !r.exited || r.workers != 0
        }) {
            return Err(Error::Limit);
        }
        let generation = GENERATION
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |g| g.checked_add(1))
            .map_err(|_| Error::Limit)?;
        let process = Process::spawn(&self.manifest, self.limits.queue)?;
        self.process = Some(process);
        self.generation = generation;
        self.started = self.now;
        self.revision = 0;
        self.last_request = 0;
        self.next_event = 0;
        self.rate_count = 0;
        self.rate_second = self.now.as_secs();
        self.failure = None;
        self.state = State::Negotiating;
        Ok(())
    }
    fn namespace(&self) -> String {
        format!("addon/{}/{}", self.manifest.id, self.generation)
    }
    fn send(&self, message: &HostMessage) -> Result<()> {
        self.process
            .as_ref()
            .ok_or(Error::Disconnected)?
            .send(message)
    }
    pub fn commands(&self) -> Vec<CommandInfo> {
        if self.state != State::Active || !self.manifest.scopes.contains(&Scope::Automation) {
            return vec![];
        }
        self.manifest
            .commands
            .intersection(&self.grants.automation_commands)
            .map(|c| CommandInfo {
                name: format!("{}.{}", self.manifest.id, c),
                generation: self.generation,
            })
            .collect()
    }
    pub fn invoke(&mut self, command: &CommandInfo) -> Result<()> {
        if command.generation != self.generation
            || !self.commands().iter().any(|c| c.name == command.name)
        {
            return Err(Error::Denied);
        }
        let name = command
            .name
            .strip_prefix(&format!("{}.", self.manifest.id))
            .ok_or(Error::Denied)?;
        if self.outbox.len() >= self.limits.pending {
            return Err(Error::Limit);
        }
        let key = format!("command:{name}");
        self.outbox.push_back(LocalEvent {
            key,
            command: Some(name.into()),
            value: None,
            value_revision: 0,
        });
        Ok(())
    }
    pub fn input<M, A>(
        &mut self,
        runtime: &mut Runtime<M, A>,
        key: &str,
        input: Input,
        clipboard: &mut impl view_controls::view_text::Clipboard,
    ) -> Result<()> {
        if self.state != State::Active || self.owner.is_revoked() {
            return Err(Error::Stale);
        }
        let coalesced = !matches!(input, Input::Activate)
            && self
                .outbox
                .iter()
                .any(|e| e.key == key && e.command.is_none());
        if !coalesced && self.outbox.len() >= self.limits.pending {
            return Err(Error::Limit);
        }
        let Some(event) = self.ui.input(runtime, key, input, clipboard)? else {
            return self.ui.refresh(runtime, &self.namespace(), &self.parents);
        };
        if coalesced {
            if let Some(old) = self
                .outbox
                .iter_mut()
                .find(|e| e.key == key && e.command.is_none())
            {
                *old = event;
            }
        } else {
            self.outbox.push_back(event);
        }
        self.ui.refresh(runtime, &self.namespace(), &self.parents)
    }
    pub fn deactivate<M, A>(&mut self, runtime: &mut Runtime<M, A>) -> Result<()> {
        self.stop(runtime, State::Deactivating, None)
    }
    pub fn disable<M, A>(&mut self, runtime: &mut Runtime<M, A>) -> Result<()> {
        self.stop(runtime, State::Disabled, None)
    }
    pub fn remove<M, A>(&mut self, runtime: &mut Runtime<M, A>) -> Result<()> {
        self.stop(runtime, State::Removed, None)
    }
    pub fn enable(&mut self) -> Result<()> {
        if self.state != State::Disabled {
            return Err(Error::Stale);
        }
        self.state = State::Installed;
        Ok(())
    }
    fn stop<M, A>(
        &mut self,
        runtime: &mut Runtime<M, A>,
        state: State,
        failure: Option<Error>,
    ) -> Result<()> {
        self.state = state;
        self.failure = failure;
        self.pending.clear();
        self.outbox.clear();
        self.nodes.clear();
        if let Some(p) = &mut self.process {
            p.cancel();
        }
        self.ui.clear(runtime)
    }
    pub fn poll<M, A>(
        &mut self,
        runtime: &mut Runtime<M, A>,
        app: &mut impl Application,
        now: Duration,
    ) -> Result<()> {
        if now < self.now {
            return Err(Error::Invalid("clock regressed".into()));
        }
        self.now = now;
        if self.owner.is_revoked() && matches!(self.state, State::Active | State::Negotiating) {
            self.deactivate(runtime)?;
        }
        if self.state == State::Deactivating
            && self.receipt().is_some_and(|r| r.exited && r.workers == 0)
        {
            self.state = State::Installed;
        }
        if !matches!(self.state, State::Active | State::Negotiating) {
            return Ok(());
        }
        let result = self.poll_inner(runtime, app);
        if let Err(e) = result {
            self.stop(runtime, State::Failed, Some(e.clone()))?;
            return Err(e);
        }
        Ok(())
    }
    fn poll_inner<M, A>(
        &mut self,
        runtime: &mut Runtime<M, A>,
        app: &mut impl Application,
    ) -> Result<()> {
        self.ui
            .sync_focus(runtime, self.now.as_millis().min(u64::MAX as u128) as u64)?;
        if self.state == State::Negotiating
            && self.now.saturating_sub(self.started) >= self.limits.startup
        {
            return Err(Error::Timeout);
        }
        if self.rate_second != self.now.as_secs() {
            self.rate_second = self.now.as_secs();
            self.rate_count = 0;
        }
        for _ in 0..self.limits.queue {
            let Some(envelope) = self
                .process
                .as_ref()
                .ok_or(Error::Disconnected)?
                .receive()?
            else {
                break;
            };
            self.rate_count += 1;
            if self.rate_count > self.limits.messages_per_second {
                return Err(Error::Limit);
            }
            self.accept(runtime, app, envelope)?;
        }
        if self.state == State::Active {
            let expired: Vec<_> = self
                .pending
                .iter()
                .filter(|(_, deadline)| self.now >= **deadline)
                .map(|(id, _)| *id)
                .collect();
            for event in expired {
                self.pending.remove(&event);
                self.send(&HostMessage::Cancel { event })?;
            }
            while self.pending.len() < self.limits.pending {
                let Some(event) = self.outbox.front() else {
                    break;
                };
                let event = event.clone();
                let id = self.next_event.checked_add(1).ok_or(Error::Limit)?;
                self.send(&HostMessage::Event {
                    generation: self.generation,
                    event: id,
                    key: event.key,
                    command: event.command,
                    value: event.value,
                    value_revision: event.value_revision,
                })?;
                self.next_event = id;
                self.pending
                    .insert(id, self.now.saturating_add(self.limits.task));
                self.outbox.pop_front();
            }
        }
        Ok(())
    }
    // Called only after framing/serde validation. Exposed for deterministic protocol harnesses.
    pub fn accept<M, A>(
        &mut self,
        runtime: &mut Runtime<M, A>,
        app: &mut impl Application,
        envelope: GuestEnvelope,
    ) -> Result<()> {
        if self.state == State::Negotiating {
            match envelope.message {
                GuestMessage::Hello {
                    protocol,
                    id,
                    version,
                } if envelope.generation == 0
                    && protocol == VERSION
                    && id == self.manifest.id
                    && version == self.manifest.version =>
                {
                    self.send(&HostMessage::Welcome {
                        generation: self.generation,
                        protocol: VERSION,
                        components: self.selected.keys().cloned().collect(),
                        scopes: self.manifest.scopes.clone(),
                    })?;
                    self.state = State::Active;
                    return Ok(());
                }
                _ => return Err(Error::Incompatible("guest handshake".into())),
            }
        }
        if self.state != State::Active
            || envelope.generation != self.generation
            || self.owner.is_revoked()
        {
            return Err(Error::Stale);
        }
        match envelope.message {
            GuestMessage::Hello { .. } => return Err(Error::Invalid("duplicate hello".into())),
            GuestMessage::Snapshot { base, nodes } => self.update_ui(runtime, base, nodes)?,
            GuestMessage::Patch { base, changes } => {
                if changes.len() > self.limits.nodes {
                    return Err(Error::Limit);
                }
                if base != self.revision {
                    self.send(&HostMessage::Resync {
                        revision: self.revision,
                    })?;
                    return Ok(());
                }
                let mut nodes = self.nodes.clone();
                for patch in changes {
                    match patch {
                        Patch::Put { node } => {
                            nodes.insert(node.key.clone(), node);
                        }
                        Patch::Remove { key } => {
                            if nodes.remove(&key).is_none() {
                                return Err(Error::Stale);
                            }
                        }
                    }
                }
                self.update_ui(runtime, base, nodes.into_values().collect())?;
            }
            GuestMessage::ReadDocument { request } => {
                let result = self
                    .request(request, Scope::DocumentRead)
                    .and_then(|_| app.snapshot());
                self.reply(request, result)?;
            }
            GuestMessage::EditDocument {
                request,
                action,
                revision,
                arguments,
            } => {
                let result = self.request(request, Scope::DocumentEdit).and_then(|_| {
                    let validate = self.grants.actions.get(&action).ok_or(Error::Denied)?;
                    validate(&arguments)?;
                    app.edit(&action, revision, arguments)
                });
                self.reply(request, result)?;
            }
            GuestMessage::Complete { event } => {
                self.pending.remove(&event);
            }
            GuestMessage::Validation {
                event,
                key,
                value_revision,
                error,
            } => {
                if !self.pending.contains_key(&event) {
                    return Ok(());
                }
                if let Err(e) = self.ui.validation(&key, value_revision, error)
                    && e != Error::Stale
                {
                    return Err(e);
                }
            }
        }
        Ok(())
    }
    fn request(&mut self, id: u64, scope: Scope) -> Result<()> {
        if id == 0 || id <= self.last_request {
            return Err(Error::Stale);
        }
        self.last_request = id;
        if !self.manifest.scopes.contains(&scope) {
            return Err(Error::Denied);
        }
        Ok(())
    }
    fn reply(&self, request: u64, result: Result<DocumentSnapshot>) -> Result<()> {
        self.send(&HostMessage::Reply { request, result })
    }
    fn update_ui<M, A>(
        &mut self,
        runtime: &mut Runtime<M, A>,
        base: u64,
        nodes: Vec<UiNode>,
    ) -> Result<()> {
        if !self.manifest.scopes.contains(&Scope::Ui) {
            return Err(Error::Denied);
        }
        if base != self.revision {
            return self.send(&HostMessage::Resync {
                revision: self.revision,
            });
        }
        let entries = match self.ui.prepare(
            &nodes,
            &self.manifest,
            &self.registry,
            &self.selected,
            self.limits.nodes,
            self.limits.ui_bytes,
        ) {
            Err(Error::Stale) => {
                return self.send(&HostMessage::Resync {
                    revision: self.revision,
                });
            }
            other => other?,
        };
        let revision = self.revision.checked_add(1).ok_or(Error::Limit)?;
        self.ui
            .apply(runtime, &self.namespace(), &self.parents, entries)?;
        self.nodes = nodes.into_iter().map(|n| (n.key.clone(), n)).collect();
        self.revision = revision;
        self.send(&HostMessage::UiAccepted { revision })
    }
}
