use crate::{Error, Result, SessionId};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::Value;
use std::collections::BTreeSet;

/// Exact document URLs deliberately grant less authority than an entire origin.
/// No frame handler is registered by the native adapter; only main-document messages qualify.
#[derive(Clone, Debug, Default)]
pub struct PagePolicy {
    pub documents: BTreeSet<String>,
    pub downloads: bool,
    pub devtools: bool,
}
impl PagePolicy {
    pub fn validate(&self) -> Result<()> {
        if self.documents.len() > 64
            || self.documents.iter().any(|u| {
                u.len() > 4096
                    || u.chars().any(|c| c.is_control() || c.is_whitespace())
                    || !(u.starts_with("https://") || u == "about:blank")
            })
        {
            return Err(Error::Invalid(
                "bounded HTTPS documents or explicit local about:blank required".into(),
            ));
        }
        Ok(())
    }
    pub fn permits(&self, url: &str) -> bool {
        self.documents.contains(url)
    }
}
#[derive(Clone, Debug)]
pub struct Bridge {
    session: SessionId,
    epoch: u64,
    source: Option<String>,
    commands: BTreeSet<String>,
    active: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    version: u32,
    session: String,
    epoch: String,
    command: String,
    arguments: Value,
}
impl Bridge {
    pub fn new(session: SessionId) -> Self {
        Self {
            session,
            epoch: 1,
            source: None,
            commands: BTreeSet::new(),
            active: true,
        }
    }
    pub fn epoch(&self) -> u64 {
        self.epoch
    }
    pub fn is_active(&self) -> bool {
        self.active
    }
    pub fn revoke(&mut self) {
        self.epoch = self.epoch.saturating_add(1);
        self.source = None;
        self.commands.clear();
    }
    pub fn close(&mut self) {
        self.revoke();
        self.active = false;
    }
    /// Grant after a navigation completion. Calling this is an explicit host decision.
    pub fn grant(&mut self, policy: &PagePolicy, document: &str, commands: &[&str]) -> Result<u64> {
        if !self.active
            || !policy.permits(document)
            || commands.len() > 64
            || commands.iter().any(|c| c.len() > 128 || !c.contains('.'))
        {
            return Err(Error::Denied);
        }
        self.revoke();
        self.source = Some(document.into());
        self.commands = commands.iter().map(|s| (*s).into()).collect();
        Ok(self.epoch)
    }
    /// The source and main-frame flag must come from the provider, never the JSON payload.
    pub fn decode<T: DeserializeOwned>(
        &self,
        source: &str,
        main_frame: bool,
        command: &str,
        bytes: &[u8],
    ) -> Result<T> {
        if !self.active
            || !main_frame
            || self.source.as_deref() != Some(source)
            || bytes.len() > 65536
        {
            return Err(Error::Denied);
        }
        let message: Envelope =
            serde_json::from_slice(bytes).map_err(|e| Error::Invalid(e.to_string()))?;
        if message.version != 1
            || message.session != self.session.0.to_string()
            || message.epoch != self.epoch.to_string()
        {
            return Err(Error::Stale);
        }
        if message.command != command || !self.commands.contains(command) {
            return Err(Error::Denied);
        }
        serde_json::from_value(message.arguments).map_err(|e| Error::Invalid(e.to_string()))
    }
}
