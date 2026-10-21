//! Explicit UI-thread resource reducers. Executors transport results; builds only read.
#![forbid(unsafe_code)]

pub mod assets;
pub mod sections;
pub mod stream;

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;
use view_core::{Arena, ArenaHandle, Cancellation, CoreError, Dirty, NodeId, Revision, Runtime};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Idle,
    Loading,
    Ready,
    Failed,
    Offline,
}

/// Explicit policy for read-only/idempotent retrieval. Mutations need app conflict policy.
#[derive(Clone, Copy, Debug)]
pub struct Policy {
    pub retain_on_refresh: bool,
    pub max_retries: u32,
    pub retry_delay: Duration,
    pub reconnect: bool,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            retain_on_refresh: true,
            max_retries: 2,
            retry_delay: Duration::from_secs(1),
            reconnect: false,
        }
    }
}

/// Worker-safe identity, including resource namespace and monotonically advancing request.
#[derive(Clone, Debug)]
pub struct Request {
    resource: ArenaHandle,
    generation: Revision,
    pub owner: NodeId,
    pub key: String,
    cancelled: Arc<AtomicBool>,
    lifetime: Cancellation,
}
impl Request {
    pub fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire) || self.lifetime.is_revoked()
    }
}

/// A resource has one owner lifetime. Use an application root for caches that outlive views.
pub struct Resource<T> {
    identity: ArenaHandle,
    owner: NodeId,
    lifetime: Cancellation,
    generation: Revision,
    revision: Revision,
    request: Option<Request>,
    key: String,
    value: Option<T>,
    error: Option<String>,
    progress: Option<f32>,
    status: Status,
    policy: Policy,
    attempts: u32,
    retry_at: Option<Duration>,
    subscriptions: Vec<(NodeId, Revision)>,
    max_subscriptions: usize,
}
impl<T> Resource<T> {
    pub fn new(
        owner: NodeId,
        lifetime: Cancellation,
        policy: Policy,
        max_subscriptions: usize,
    ) -> Result<Self, CoreError> {
        let mut arena = Arena::new()?;
        let identity = arena.insert(())?;
        Ok(Self {
            identity,
            owner,
            lifetime,
            generation: Revision::default(),
            revision: Revision::default(),
            request: None,
            key: String::new(),
            value: None,
            error: None,
            progress: None,
            status: Status::Idle,
            policy,
            attempts: 0,
            retry_at: None,
            subscriptions: vec![],
            max_subscriptions,
        })
    }
    pub fn status(&self) -> Status {
        self.status
    }
    pub fn value(&self) -> Option<&T> {
        self.value.as_ref()
    }
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
    pub fn revision(&self) -> Revision {
        self.revision
    }
    pub fn progress(&self) -> Option<f32> {
        self.progress
    }
    pub fn retry_at(&self) -> Option<Duration> {
        self.retry_at
    }
    fn changed(&mut self) -> Result<(), CoreError> {
        self.revision = self.revision.checked_next()?;
        Ok(())
    }
    fn revoke(&mut self) {
        if let Some(request) = self.request.take() {
            request.cancelled.store(true, Ordering::Release);
        }
    }
    /// Returns an effect descriptor; no executor/network work is launched here.
    pub fn begin(&mut self, key: impl Into<String>) -> Result<Request, CoreError> {
        if self.lifetime.is_revoked() {
            return Err(CoreError::InactiveRequest);
        }
        let key = key.into();
        if key.len() > 4096 {
            return Err(CoreError::CapacityExhausted);
        }
        let generation = self.generation.checked_next()?;
        self.changed()?;
        self.revoke();
        if key != self.key {
            self.attempts = 0;
            self.value = None;
        }
        if !self.policy.retain_on_refresh {
            self.value = None;
        }
        self.key = key.clone();
        self.generation = generation;
        self.error = None;
        self.progress = None;
        self.retry_at = None;
        self.status = Status::Loading;
        let request = Request {
            resource: self.identity,
            generation,
            owner: self.owner,
            key,
            cancelled: Arc::new(AtomicBool::new(false)),
            lifetime: self.lifetime.clone(),
        };
        self.request = Some(request.clone());
        Ok(request)
    }
    fn accepts(&self, request: &Request) -> bool {
        !request.cancelled()
            && self.request.as_ref().is_some_and(|r| {
                r.resource == request.resource
                    && r.generation == request.generation
                    && r.owner == request.owner
                    && r.key == request.key
            })
    }
    /// UI-thread preflight for adapters that validate/decode before applying a completion.
    pub fn is_current(&self, request: &Request) -> bool {
        self.accepts(request)
    }
    pub fn complete(&mut self, request: &Request, value: T) -> Result<bool, CoreError> {
        if !self.accepts(request) {
            return Ok(false);
        }
        self.changed()?;
        self.revoke();
        self.value = Some(value);
        self.status = Status::Ready;
        self.progress = None;
        self.error = None;
        self.attempts = 0;
        Ok(true)
    }
    pub fn fail(
        &mut self,
        request: &Request,
        error: String,
        now: Duration,
    ) -> Result<bool, CoreError> {
        if !self.accepts(request) {
            return Ok(false);
        }
        if error.len() > 4096 {
            return Err(CoreError::CapacityExhausted);
        }
        self.changed()?;
        self.revoke();
        self.status = Status::Failed;
        self.error = Some(error);
        self.progress = None;
        self.retry_at = if self.attempts < self.policy.max_retries {
            now.checked_add(self.policy.retry_delay)
        } else {
            None
        };
        Ok(true)
    }
    pub fn set_progress(&mut self, request: &Request, fraction: f32) -> Result<bool, CoreError> {
        if !self.accepts(request) {
            return Ok(false);
        }
        if !fraction.is_finite() || !(0.0..=1.0).contains(&fraction) {
            return Err(CoreError::InvalidGeometry);
        }
        if self.progress.is_some_and(|p| fraction <= p) {
            return Ok(false);
        }
        self.changed()?;
        self.progress = Some(fraction);
        Ok(true)
    }
    /// Host-clock retry is explicit and only applies to retrieval effects.
    pub fn retry(&mut self, now: Duration) -> Result<Option<Request>, CoreError> {
        if self.retry_at.is_none_or(|deadline| now < deadline) {
            return Ok(None);
        }
        let r = self.begin(self.key.clone())?;
        self.attempts += 1;
        Ok(Some(r))
    }
    pub fn cancel(&mut self) -> Result<(), CoreError> {
        self.changed()?;
        self.revoke();
        self.retry_at = None;
        self.progress = None;
        self.status = if self.value.is_some() {
            Status::Ready
        } else {
            Status::Idle
        };
        Ok(())
    }
    pub fn offline(&mut self) -> Result<(), CoreError> {
        self.cancel()?;
        self.status = Status::Offline;
        Ok(())
    }
    pub fn reconnect(&mut self) -> Result<Option<Request>, CoreError> {
        if self.status == Status::Offline && self.policy.reconnect {
            self.begin(self.key.clone()).map(Some)
        } else {
            Ok(None)
        }
    }
    pub fn subscribe(&mut self, owner: NodeId) -> Result<(), CoreError> {
        if self.subscriptions.iter().any(|(id, _)| *id == owner) {
            return Ok(());
        }
        if self.subscriptions.len() >= self.max_subscriptions {
            return Err(CoreError::CapacityExhausted);
        }
        self.subscriptions.push((owner, Revision::default()));
        Ok(())
    }
    pub fn unsubscribe(&mut self, owner: NodeId) {
        self.subscriptions.retain(|(id, _)| *id != owner);
    }
    /// UI-thread adapter invalidates each live subscriber once per resource revision.
    pub fn notify<M, A>(&mut self, runtime: &mut Runtime<M, A>) -> Result<usize, CoreError> {
        let mut count = 0;
        self.subscriptions.retain_mut(|(id, seen)| {
            if runtime.dirty(*id).is_err() {
                return false;
            }
            if *seen != self.revision && runtime.invalidate(*id, Dirty::BUILD).is_ok() {
                *seen = self.revision;
                count += 1;
            }
            true
        });
        Ok(count)
    }
}
impl<T> Drop for Resource<T> {
    fn drop(&mut self) {
        self.revoke();
    }
}
