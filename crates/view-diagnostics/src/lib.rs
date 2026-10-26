//! Optional, host-owned diagnostics. Nothing installs a hook, writes a file or
//! uploads data until the application explicitly chooses to do so.
//!
//! Collect/redact/serialize while healthy. Panic reporting uses an immutable
//! prepared snapshot, never walks a UI tree, and never attempts UI recovery.
#![forbid(unsafe_code)]

mod panic;
mod storage;
mod symbols;
mod wer;
pub use panic::{PanicHook, PanicReporter, Reporter, chain_panic_hook};
pub use storage::ReportStore;
pub use symbols::{PdbIdentity, read_pdb_identity};
pub use wer::{DumpType, WerLocalDumps};

use serde::{Deserialize, Serialize};
use std::{collections::VecDeque, io, io::Write};
use view_core::Trace;

/// Hard ceilings also apply to host-selected smaller budgets.
const MAX_EVENTS: usize = 256;
const MAX_TEXT_BYTES: usize = 512;
const MAX_CAPABILITIES: usize = 64;
const MAX_REPORT_BYTES: usize = 1_048_576;

/// Diagnostic resource budgets. Zero event capacity disables breadcrumbs.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub events: usize,
    pub text_bytes: usize,
    pub report_bytes: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            events: 64,
            text_bytes: 256,
            report_bytes: 65_536,
        }
    }
}

/// Application-owned build/symbol identifiers; never inferred from source paths.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Identity {
    pub application: String,
    pub build: String,
    pub symbols: String,
    pub framework: String,
    pub platform: String,
    pub backend: String,
}

/// Independent revision streams; a commit is not proof of presentation.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Revisions {
    pub document: u64,
    pub committed: u64,
    pub presented: u64,
    pub device_generation: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Breadcrumb {
    /// Host-provided monotonic observation time; never sampled by this library.
    pub tick: u64,
    pub operation: String,
}

/// Every textual field, including build identifiers, passes the host's redactor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Application,
    Build,
    Symbols,
    Framework,
    Platform,
    Backend,
    Capability,
    Operation,
}

/// Versioned, bounded report. This contains no panic payload, document text,
/// source location, filesystem path, or automatically collected system inventory.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Envelope {
    pub schema: u32,
    pub identity: Identity,
    pub capabilities: Vec<String>,
    pub revisions: Revisions,
    pub breadcrumbs: Vec<Breadcrumb>,
    pub dropped: u64,
}

/// Owner-thread healthy-operation collector; no global registration or locks.
pub struct Diagnostics {
    limits: Limits,
    identity: Identity,
    capabilities: Vec<String>,
    revisions: Revisions,
    breadcrumbs: VecDeque<Breadcrumb>,
    dropped: u64,
}

fn bounded(value: &str, max: usize) -> String {
    let mut end = value.len().min(max);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].into()
}

impl Diagnostics {
    pub fn new(identity: Identity, limits: Limits) -> Self {
        let limits = Limits {
            events: limits.events.min(MAX_EVENTS),
            text_bytes: limits.text_bytes.min(MAX_TEXT_BYTES),
            report_bytes: limits.report_bytes.min(MAX_REPORT_BYTES),
        };
        let identity = Identity {
            application: bounded(&identity.application, limits.text_bytes),
            build: bounded(&identity.build, limits.text_bytes),
            symbols: bounded(&identity.symbols, limits.text_bytes),
            framework: bounded(&identity.framework, limits.text_bytes),
            platform: bounded(&identity.platform, limits.text_bytes),
            backend: bounded(&identity.backend, limits.text_bytes),
        };
        Self {
            limits,
            identity,
            capabilities: vec![],
            revisions: Revisions::default(),
            breadcrumbs: VecDeque::new(),
            dropped: 0,
        }
    }

    fn redacted_identity(
        &self,
        redact: &mut impl FnMut(Field, &str) -> Option<String>,
    ) -> Identity {
        let mut field = |key, value: &str| {
            bounded(
                &redact(key, value).unwrap_or_default(),
                self.limits.text_bytes,
            )
        };
        Identity {
            application: field(Field::Application, &self.identity.application),
            build: field(Field::Build, &self.identity.build),
            symbols: field(Field::Symbols, &self.identity.symbols),
            framework: field(Field::Framework, &self.identity.framework),
            platform: field(Field::Platform, &self.identity.platform),
            backend: field(Field::Backend, &self.identity.backend),
        }
    }

    pub fn capabilities<'a>(&mut self, values: impl IntoIterator<Item = &'a str>) {
        self.capabilities = values
            .into_iter()
            .take(MAX_CAPABILITIES)
            .map(|s| bounded(s, self.limits.text_bytes))
            .collect();
    }

    pub fn revisions(&mut self, revisions: Revisions) {
        self.revisions = revisions;
    }

    pub fn record(&mut self, tick: u64, operation: &str) {
        if self.limits.events == 0 {
            self.dropped = self.dropped.saturating_add(1);
            return;
        }
        if self.breadcrumbs.len() == self.limits.events {
            self.breadcrumbs.pop_front();
            self.dropped = self.dropped.saturating_add(1);
        }
        self.breadcrumbs.push_back(Breadcrumb {
            tick,
            operation: bounded(operation, self.limits.text_bytes),
        });
    }

    /// Drain `Runtime::drain_trace` here while healthy. Node identities and user
    /// values are deliberately omitted; host labels may be recorded separately.
    pub fn trace(&mut self, tick: u64, trace: Trace) {
        let operation = match trace {
            Trace::Mounted(_) => "runtime.mount",
            Trace::Unmounted(_) => "runtime.unmount",
            Trace::Action(_, _) => "runtime.action",
            Trace::Rejected(_, _) => "runtime.reject",
            Trace::Invalidated(_, _) => "runtime.invalidate",
            Trace::Committed(revision) => {
                self.revisions.committed = revision.get();
                "runtime.commit"
            }
        };
        self.record(tick, operation);
    }

    /// Host redaction runs before serialization and may remove fields/events.
    /// It runs only while healthy; panicking here is an application error.
    pub fn prepare(
        &self,
        mut redact: impl FnMut(Field, &str) -> Option<String>,
    ) -> io::Result<PreparedReport> {
        let envelope = Envelope {
            schema: 1,
            identity: self.redacted_identity(&mut redact),
            capabilities: self
                .capabilities
                .iter()
                .filter_map(|s| redact(Field::Capability, s))
                .map(|s| bounded(&s, self.limits.text_bytes))
                .collect(),
            revisions: self.revisions,
            breadcrumbs: self
                .breadcrumbs
                .iter()
                .filter_map(|b| {
                    redact(Field::Operation, &b.operation).map(|s| Breadcrumb {
                        tick: b.tick,
                        operation: bounded(&s, self.limits.text_bytes),
                    })
                })
                .collect(),
            dropped: self.dropped,
        };
        let mut writer = BoundedWriter {
            bytes: Vec::new(),
            max: self.limits.report_bytes,
        };
        serde_json::to_writer(&mut writer, &envelope).map_err(io::Error::other)?;
        Ok(PreparedReport(writer.bytes))
    }
}

/// Immutable redacted bytes. Prepare a fresh report when healthy context changes.
#[derive(Clone)]
pub struct PreparedReport(Vec<u8>);
impl PreparedReport {
    pub fn bytes(&self) -> &[u8] {
        &self.0
    }
}

struct BoundedWriter {
    bytes: Vec<u8>,
    max: usize,
}
impl Write for BoundedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.max.saturating_sub(self.bytes.len()) {
            return Err(io::Error::other("diagnostic report byte budget exceeded"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
