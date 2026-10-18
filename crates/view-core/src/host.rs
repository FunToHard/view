//! Explicit services supplied by an embedding host. No native loop is created.
use crate::{LogicalRect, Revision, SemanticSnapshot, Snapshot};

/// Declared service support; unavailable is an explicit supported fallback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceSupport {
    /// The embedding host implements this service.
    Host,
    /// Not available in this integration.
    Unavailable,
}
/// Native services are chosen independently of the shared GPU device.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostCapabilities {
    /// Clipboard read/write support.
    pub clipboard: ServiceSupport,
    /// Composition and candidate-position support.
    pub ime: ServiceSupport,
    /// Host semantic/accessibility publication support.
    pub accessibility: ServiceSupport,
}
/// A host service error, surfaced to the application instead of silently ignored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostServiceError {
    /// Capability is unavailable.
    Unsupported,
    /// Host implementation failed.
    Failed(String),
}
/// Host supplies these on its native owner thread. Platform adapters may delegate
/// to existing view-platform services; headless hosts report unavailable services.
pub trait HostServices {
    /// Stable service capability declaration.
    fn capabilities(&self) -> HostCapabilities;
    /// Publish a coherent semantic snapshot, including removals.
    fn publish_semantics(&mut self, snapshot: &SemanticSnapshot) -> Result<(), HostServiceError>;
    /// Enable composition and position its candidate window in logical coordinates.
    fn ime(&mut self, enabled: bool, caret: LogicalRect) -> Result<(), HostServiceError>;
    /// Read host clipboard text.
    fn clipboard_read(&mut self) -> Result<String, HostServiceError>;
    /// Write host clipboard text.
    fn clipboard_write(&mut self, text: &str) -> Result<(), HostServiceError>;
}
/// Publishes only committed revisions and retries failed publication without
/// re-running application effects. The host keeps event/submission authority.
pub struct HostBridge<S> {
    services: S,
    published: Option<Revision>,
}
impl<S: HostServices> HostBridge<S> {
    /// Attach explicitly selected host services.
    pub fn new(services: S) -> Self {
        Self {
            services,
            published: None,
        }
    }
    /// Access native services on the caller's owner thread.
    pub fn services(&mut self) -> &mut S {
        &mut self.services
    }
    /// Publish if supported and changed. False means unchanged or unavailable.
    pub fn publish(&mut self, snapshot: &Snapshot) -> Result<bool, HostServiceError> {
        if self.services.capabilities().accessibility == ServiceSupport::Unavailable {
            return Ok(false);
        }
        if self.published == Some(snapshot.revision) {
            return Ok(false);
        }
        if self.published.is_some_and(|r| r > snapshot.revision) {
            return Err(HostServiceError::Failed("stale host snapshot".into()));
        }
        self.services
            .publish_semantics(&snapshot.semantic_snapshot())?;
        self.published = Some(snapshot.revision);
        Ok(true)
    }
}
