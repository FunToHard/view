use crate::{Error, Result};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::{
    collections::BTreeSet,
    io::{Read, Write},
    path::PathBuf,
};
pub const VERSION: u32 = 1;
pub const MAX_FRAME: usize = 256 * 1024;
pub fn identifier(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 96
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Slot {
    SidePanel,
    Menu,
    Toolbar,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    Ui,
    DocumentRead,
    DocumentEdit,
    Automation,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComponentRequirement {
    pub name: String,
    pub version: u32,
    pub required: bool,
    pub fallback: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub id: String,
    pub version: String,
    pub protocol: u32,
    pub executable: PathBuf,
    pub arguments: Vec<String>,
    pub scopes: BTreeSet<Scope>,
    pub slots: BTreeSet<Slot>,
    pub components: Vec<ComponentRequirement>,
    pub commands: BTreeSet<String>,
}
impl Manifest {
    pub fn validate(&self) -> Result<()> {
        if !identifier(&self.id)
            || !identifier(&self.version)
            || !self.executable.is_absolute()
            || self.arguments.len() > 16
            || self.arguments.iter().any(|s| s.len() > 4096)
            || self.components.len() > 32
            || self.commands.len() > 64
            || self.commands.iter().any(|s| !identifier(s))
        {
            return Err(Error::Invalid("manifest bounds/identity/path".into()));
        }
        if self.protocol != VERSION {
            return Err(Error::Incompatible("protocol".into()));
        }
        let mut seen = BTreeSet::new();
        for c in &self.components {
            if !identifier(&c.name)
                || !seen.insert(&c.name)
                || c.fallback.as_ref().is_some_and(|s| !identifier(s))
            {
                return Err(Error::Invalid("component requirement".into()));
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct UiNode {
    pub key: String,
    pub slot: Slot,
    pub component: String,
    pub props: Value,
}
impl UiNode {
    pub fn label(key: &str, text: &str) -> Self {
        Self {
            key: key.into(),
            slot: Slot::SidePanel,
            component: "label".into(),
            props: serde_json::json!({"label":text}),
        }
    }
    pub fn button(key: &str, label: &str, command: &str) -> Self {
        Self {
            key: key.into(),
            slot: Slot::SidePanel,
            component: "button".into(),
            props: serde_json::json!({"label":label,"command":command}),
        }
    }
    pub fn text(key: &str, label: &str, value: &str) -> Self {
        Self {
            key: key.into(),
            slot: Slot::SidePanel,
            component: "text".into(),
            props: serde_json::json!({"label":label,"value":value}),
        }
    }
    pub fn in_slot(mut self, slot: Slot) -> Self {
        self.slot = slot;
        self
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Patch {
    Put { node: UiNode },
    Remove { key: String },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentSnapshot {
    pub revision: u64,
    pub value: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum GuestMessage {
    Hello {
        protocol: u32,
        id: String,
        version: String,
    },
    Snapshot {
        base: u64,
        nodes: Vec<UiNode>,
    },
    Patch {
        base: u64,
        changes: Vec<Patch>,
    },
    ReadDocument {
        request: u64,
    },
    EditDocument {
        request: u64,
        action: String,
        revision: u64,
        arguments: Value,
    },
    Complete {
        event: u64,
    },
    Validation {
        event: u64,
        key: String,
        value_revision: u64,
        error: Option<String>,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuestEnvelope {
    pub generation: u64,
    pub message: GuestMessage,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum HostMessage {
    Welcome {
        generation: u64,
        protocol: u32,
        components: Vec<String>,
        scopes: BTreeSet<Scope>,
    },
    UiAccepted {
        revision: u64,
    },
    Resync {
        revision: u64,
    },
    Reply {
        request: u64,
        result: std::result::Result<DocumentSnapshot, Error>,
    },
    Event {
        generation: u64,
        event: u64,
        key: String,
        command: Option<String>,
        value: Option<String>,
        value_revision: u64,
    },
    Cancel {
        event: u64,
    },
}
/// Four-byte little-endian byte length, followed by one UTF-8 JSON value.
/// Oversized lengths fail before allocating their payload.
pub fn read_frame<T: DeserializeOwned>(reader: &mut impl Read) -> Result<T> {
    let mut header = [0; 4];
    reader.read_exact(&mut header)?;
    let n = u32::from_le_bytes(header) as usize;
    if n == 0 || n > MAX_FRAME {
        return Err(Error::Limit);
    }
    let mut body = vec![0; n];
    reader.read_exact(&mut body)?;
    serde_json::from_slice(&body).map_err(|e| Error::Invalid(e.to_string()))
}
pub fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let bytes = serde_json::to_vec(value).map_err(|e| Error::Invalid(e.to_string()))?;
    if bytes.is_empty() || bytes.len() > MAX_FRAME {
        return Err(Error::Limit);
    }
    Ok(bytes)
}
pub fn write_frame<T: Serialize>(writer: &mut impl Write, value: &T) -> Result<()> {
    let bytes = encode(value)?;
    writer.write_all(&(bytes.len() as u32).to_le_bytes())?;
    writer.write_all(&bytes)?;
    writer.flush()?;
    Ok(())
}
