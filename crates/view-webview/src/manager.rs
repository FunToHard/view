use crate::{Bridge, Error, PagePolicy, Profile, ProfileLease, Profiles, Result};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use view_core::{Cancellation, LogicalRect};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SessionId(pub u64);
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PanelId(pub u64);
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Lifetime {
    #[default]
    DisposeAfterUse,
    KeepHot,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Creating,
    Visible,
    Hot,
    Closing,
    Released,
    Failed,
}
#[derive(Clone, Debug)]
pub struct Placement {
    pub bounds: LogicalRect,
    pub scale: f64,
    pub transformed: bool,
    pub masked: bool,
}
impl Placement {
    pub fn pixels(&self) -> Result<[i32; 4]> {
        if self.transformed || self.masked {
            return Err(Error::Unsupported(
                "native rectangular child; no GPU transform/mask/overlay".into(),
            ));
        }
        let r = self.bounds;
        let values = [
            r.origin.x as f64,
            r.origin.y as f64,
            (r.origin.x + r.size.width) as f64,
            (r.origin.y + r.size.height) as f64,
        ];
        if !self.scale.is_finite()
            || self.scale <= 0.
            || r.size.width < 0.
            || r.size.height < 0.
            || values
                .iter()
                .any(|v| !v.is_finite() || (v * self.scale).abs() > i32::MAX as f64)
        {
            return Err(Error::Invalid("placement/DPI".into()));
        }
        Ok(values.map(|v| (v * self.scale).round() as i32))
    }
}
#[derive(Clone, Debug)]
pub struct Budgets {
    pub panels: usize,
    pub closing: usize,
    pub hot_count: usize,
    /// Host accounting estimate, not an assertion of browser working-set usage.
    pub hot_bytes: u64,
    pub hot_ttl: Duration,
    pub cleanup_timeout: Duration,
}
impl Default for Budgets {
    fn default() -> Self {
        Self {
            panels: 32,
            closing: 16,
            hot_count: 4,
            hot_bytes: 256 * 1024 * 1024,
            hot_ttl: Duration::from_secs(300),
            cleanup_timeout: Duration::from_secs(30),
        }
    }
}
#[derive(Clone, Debug, Default)]
pub struct Cleanup {
    pub panel_released: bool,
    pub session_exited: bool,
    pub profile_removed: bool,
    pub shared_retained: bool,
    pub persistent_retained: bool,
    pub pending: bool,
    pub failure: Option<String>,
}
#[derive(Clone, Debug)]
pub enum Event {
    Ready(PanelId, u32),
    NavigationStarting(PanelId, String, bool),
    Navigation(PanelId, String, bool),
    Message(PanelId, String, Vec<u8>),
    Failed(PanelId, String),
    Exited(SessionId, u32),
}
#[derive(Clone)]
pub struct Start {
    pub panel: PanelId,
    pub session: SessionId,
    pub profile: ProfileLease,
    pub parent: u64,
    pub placement: Placement,
    pub policy: PagePolicy,
    pub bridge: Rc<RefCell<Bridge>>,
}
/// Implementations must keep callbacks on the owner thread and report whole-group exit,
/// not just a single renderer exit. A synchronous `start` error must mean startup was
/// rolled back and no process or callback can still use the supplied profile. Native
/// child focus/IME/accessibility belongs to the provider.
pub trait Provider {
    fn start(&mut self, start: Start) -> Result<()>;
    fn events(&mut self) -> Vec<Event>;
    fn place(&mut self, panel: PanelId, parent: u64, placement: &Placement) -> Result<()>;
    fn visible(&mut self, panel: PanelId, visible: bool) -> Result<()>;
    fn focus(&mut self, panel: PanelId) -> Result<()>;
    fn navigate(&mut self, panel: PanelId, url: &str) -> Result<()>;
    fn close(&mut self, panel: PanelId) -> Result<()>;
    fn release_session(&mut self, session: SessionId);
}
struct Panel {
    start: Start,
    lifetime: Lifetime,
    owner: Cancellation,
    state: State,
    hot_since: Duration,
    estimate: u64,
    pid: Option<u32>,
    cleanup: Cleanup,
    closing_since: Duration,
    delete: Option<std::thread::JoinHandle<Result<()>>>,
}
fn complete_exit(p: &mut Panel) {
    if p.start.profile.ephemeral {
        let lease = p.start.profile.clone();
        if p.delete.is_none() {
            p.delete = Some(std::thread::spawn(move || {
                crate::profile::remove_owned(lease)
            }));
        }
    } else {
        p.cleanup.persistent_retained = true;
        p.cleanup.pending = false;
        p.state = State::Released;
    }
}
/// The UI/application root owns this manager; unmount is observed through Cancellation.
/// Rc-backed provider state intentionally makes the manager !Send and !Sync.
pub struct Manager<P: Provider> {
    provider: P,
    profiles: Profiles,
    budgets: Budgets,
    panels: BTreeMap<PanelId, Panel>,
    now: Duration,
    shutting_down: bool,
    notices: Vec<Event>,
}
static NEXT: AtomicU64 = AtomicU64::new(0);
fn identity() -> u64 {
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros() as u64;
    let seed = time.wrapping_mul(65536).max(1);
    let _ = NEXT.compare_exchange(0, seed, Ordering::AcqRel, Ordering::Acquire);
    NEXT.fetch_add(1, Ordering::Relaxed)
}
impl<P: Provider> Manager<P> {
    pub fn new(provider: P, profiles: Profiles, budgets: Budgets) -> Self {
        Self {
            provider,
            profiles,
            budgets,
            panels: BTreeMap::new(),
            now: Duration::ZERO,
            shutting_down: false,
            notices: vec![],
        }
    }
    pub fn provider_mut(&mut self) -> &mut P {
        &mut self.provider
    }
    pub fn open(
        &mut self,
        parent: u64,
        placement: Placement,
        policy: PagePolicy,
        profile: Profile,
        lifetime: Lifetime,
        owner: Cancellation,
    ) -> Result<PanelId> {
        if self.shutting_down || owner.is_revoked() {
            return Err(Error::Stale);
        }
        if self.panels.len() >= self.budgets.panels {
            return Err(Error::Budget);
        }
        placement.pixels()?;
        policy.validate()?;
        let id = PanelId(identity());
        let session = SessionId(id.0);
        let profile = self.profiles.allocate(session, &profile)?;
        if !profile.shared
            && self
                .panels
                .values()
                .any(|p| p.start.profile.path == profile.path && p.state != State::Released)
        {
            return Err(Error::Busy);
        }
        let start = Start {
            panel: id,
            session,
            profile,
            parent,
            placement,
            policy,
            bridge: Rc::new(RefCell::new(Bridge::new(session))),
        };
        if let Err(start_error) = self.provider.start(start.clone()) {
            start.bridge.borrow_mut().close();
            if start.profile.ephemeral
                && let Err(cleanup_error) = crate::profile::remove_owned(start.profile.clone())
            {
                return Err(Error::Io(format!(
                    "provider start failed: {start_error}; failed to remove ephemeral profile {}: {cleanup_error}",
                    start.profile.path.display()
                )));
            }
            return Err(start_error);
        }
        self.panels.insert(
            id,
            Panel {
                start,
                lifetime,
                owner,
                state: State::Creating,
                hot_since: self.now,
                estimate: 0,
                pid: None,
                cleanup: Cleanup::default(),
                closing_since: self.now,
                delete: None,
            },
        );
        Ok(id)
    }
    pub fn state(&self, id: PanelId) -> Result<State> {
        Ok(self.panels.get(&id).ok_or(Error::Stale)?.state)
    }
    pub fn receipt(&self, id: PanelId) -> Result<&Cleanup> {
        Ok(&self.panels.get(&id).ok_or(Error::Stale)?.cleanup)
    }
    pub fn bridge(&self, id: PanelId) -> Result<Rc<RefCell<Bridge>>> {
        Ok(self
            .panels
            .get(&id)
            .ok_or(Error::Stale)?
            .start
            .bridge
            .clone())
    }
    pub fn profile(&self, id: PanelId) -> Result<&ProfileLease> {
        Ok(&self.panels.get(&id).ok_or(Error::Stale)?.start.profile)
    }
    pub fn notices(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.notices)
    }
    pub fn navigate(&mut self, id: PanelId, url: &str) -> Result<()> {
        let p = self.panels.get(&id).ok_or(Error::Stale)?;
        if !matches!(p.state, State::Visible | State::Hot) || !p.start.policy.permits(url) {
            return Err(Error::Denied);
        }
        p.start.bridge.borrow_mut().revoke();
        self.provider.navigate(id, url)
    }
    pub fn focus(&mut self, id: PanelId) -> Result<()> {
        if self.state(id)? != State::Visible {
            return Err(Error::Denied);
        }
        self.provider.focus(id)
    }
    pub fn place(&mut self, id: PanelId, parent: u64, placement: Placement) -> Result<()> {
        placement.pixels()?;
        if !matches!(self.state(id)?, State::Visible | State::Hot) {
            return Err(Error::Stale);
        }
        self.provider.place(id, parent, &placement)?;
        let p = self.panels.get_mut(&id).ok_or(Error::Stale)?;
        p.start.parent = parent;
        p.start.placement = placement;
        Ok(())
    }
    pub fn finish(&mut self, id: PanelId, retained_bytes: u64) -> Result<()> {
        let p = self.panels.get(&id).ok_or(Error::Stale)?;
        if p.lifetime == Lifetime::DisposeAfterUse || self.shutting_down {
            return self.dispose(id);
        }
        if p.state != State::Visible {
            return Err(Error::Busy);
        }
        let hot: Vec<_> = self
            .panels
            .values()
            .filter(|p| p.state == State::Hot)
            .collect();
        if hot.len() >= self.budgets.hot_count
            || hot
                .iter()
                .try_fold(retained_bytes, |s, p| s.checked_add(p.estimate))
                .is_none_or(|s| s > self.budgets.hot_bytes)
        {
            return Err(Error::Budget);
        }
        self.provider.visible(id, false)?;
        let p = self.panels.get_mut(&id).expect("present");
        p.state = State::Hot;
        p.hot_since = self.now;
        p.estimate = retained_bytes;
        Ok(())
    }
    /// Reattachment uses the same controller. It never silently recreates a lost hot instance.
    pub fn reattach(&mut self, id: PanelId, owner: Cancellation) -> Result<()> {
        let p = self.panels.get(&id).ok_or(Error::Stale)?;
        if owner.is_revoked() || p.state != State::Hot {
            return Err(Error::Stale);
        }
        self.provider.visible(id, true)?;
        let p = self.panels.get_mut(&id).expect("present");
        p.state = State::Visible;
        p.owner = owner;
        Ok(())
    }
    pub fn eviction_candidates(&self) -> Vec<PanelId> {
        self.panels
            .iter()
            .filter(|(_, p)| {
                p.state == State::Hot
                    && self.now.saturating_sub(p.hot_since) >= self.budgets.hot_ttl
            })
            .map(|(id, _)| *id)
            .collect()
    }
    pub fn dispose(&mut self, id: PanelId) -> Result<()> {
        if matches!(self.state(id)?, State::Closing | State::Released) {
            return Ok(());
        }
        if self
            .panels
            .values()
            .filter(|p| p.state == State::Closing)
            .count()
            >= self.budgets.closing
        {
            return Err(Error::Budget);
        }
        let p = self.panels.get_mut(&id).ok_or(Error::Stale)?;
        p.start.bridge.borrow_mut().close();
        p.state = State::Closing;
        if let Err(error) = self.provider.close(id) {
            p.state = State::Failed;
            p.cleanup.failure = Some(error.to_string());
            p.cleanup.pending = true;
            return Err(error);
        }
        p.state = State::Closing;
        p.closing_since = self.now;
        p.cleanup.panel_released = true;
        p.cleanup.pending = true;
        if p.start.profile.shared {
            p.cleanup.shared_retained = true;
            p.cleanup.pending = false;
            p.state = State::Released;
            self.provider.release_session(p.start.session);
        } else if p.cleanup.session_exited {
            complete_exit(p);
            self.provider.release_session(p.start.session);
        }
        Ok(())
    }
    pub fn retry_cleanup(&mut self, id: PanelId) -> Result<()> {
        let p = self.panels.get_mut(&id).ok_or(Error::Stale)?;
        if !p.cleanup.session_exited || !p.start.profile.ephemeral || p.delete.is_some() {
            return Err(Error::Busy);
        }
        let lease = p.start.profile.clone();
        p.delete = Some(std::thread::spawn(move || {
            crate::profile::remove_owned(lease)
        }));
        p.cleanup.failure = None;
        p.cleanup.pending = true;
        p.closing_since = self.now;
        Ok(())
    }
    /// Pump after native event callbacks return. Never waits for browser exit or filesystem IO.
    pub fn tick(&mut self, now: Duration) -> Result<()> {
        if now < self.now {
            return Err(Error::Invalid("clock regressed".into()));
        }
        self.now = now;
        for event in self.provider.events() {
            match &event {
                Event::Ready(id, pid) => {
                    if let Some(p) = self.panels.get_mut(id) {
                        p.pid = Some(*pid);
                        if p.state == State::Creating {
                            p.state = State::Visible;
                        }
                    }
                }
                Event::Failed(id, reason) => {
                    if let Some(p) = self.panels.get_mut(id) {
                        p.start.bridge.borrow_mut().close();
                        if p.state != State::Released {
                            p.cleanup.failure = Some(reason.clone());
                            if p.state != State::Closing {
                                p.state = State::Failed;
                            }
                        }
                    }
                }
                Event::Exited(session, pid) => {
                    for p in self
                        .panels
                        .values_mut()
                        .filter(|p| p.start.session == *session && p.pid == Some(*pid))
                    {
                        if p.cleanup.session_exited || p.state == State::Released {
                            continue;
                        }
                        if p.state == State::Closing {
                            p.cleanup.session_exited = true;
                            complete_exit(p);
                            self.provider.release_session(*session);
                        } else {
                            p.cleanup.session_exited = true;
                            p.state = State::Failed;
                            p.start.bridge.borrow_mut().close();
                        }
                    }
                }
                _ => {}
            }
            if self.notices.len() == 256 {
                self.notices.remove(0);
            }
            self.notices.push(event);
        }
        let unmounted: Vec<_> = self
            .panels
            .iter()
            .filter(|(_, p)| {
                p.owner.is_revoked() && matches!(p.state, State::Visible | State::Creating)
            })
            .map(|(id, _)| *id)
            .collect();
        for id in unmounted {
            if self.state(id)? == State::Creating
                || self.finish(id, self.budgets.hot_bytes).is_err()
            {
                self.dispose(id)?;
            }
        }
        for p in self.panels.values_mut() {
            if p.delete.as_ref().is_some_and(|w| w.is_finished()) {
                match p
                    .delete
                    .take()
                    .expect("finished")
                    .join()
                    .unwrap_or_else(|_| Err(Error::Io("cleanup worker panic".into())))
                {
                    Ok(()) => {
                        p.cleanup.profile_removed = true;
                        p.cleanup.pending = false;
                        p.state = State::Released;
                    }
                    Err(e) => {
                        p.cleanup.failure = Some(e.to_string());
                        p.cleanup.pending = true;
                    }
                }
            }
            if p.state == State::Closing
                && now.saturating_sub(p.closing_since) >= self.budgets.cleanup_timeout
            {
                p.cleanup
                    .failure
                    .get_or_insert("cleanup timeout; still pending, no process termination".into());
            }
        }
        Ok(())
    }
    pub fn shutdown(&mut self) -> Result<()> {
        self.shutting_down = true;
        let ids: Vec<_> = self.panels.keys().copied().collect();
        for id in ids {
            self.dispose(id)?;
        }
        Ok(())
    }
    /// Explicitly retire a completed receipt to free the bounded manager slot.
    pub fn forget(&mut self, id: PanelId) -> Result<Cleanup> {
        if self.state(id)? != State::Released {
            return Err(Error::Busy);
        }
        Ok(self.panels.remove(&id).expect("present").cleanup)
    }
}
impl<P: Provider> Drop for Manager<P> {
    fn drop(&mut self) {
        // Best effort only: no promise of process exit or profile deletion from Drop.
        for (id, p) in &self.panels {
            p.start.bridge.borrow_mut().close();
            let _ = self.provider.close(*id);
        }
    }
}
