//! Protocol v1: explicit identities, preconditions, scopes and bounded schemas.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub const VERSION: u32 = 1;
pub const MAX_MESSAGE: usize = 256 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Code {
    Unsupported,
    Invalid,
    Unauthorized,
    Stale,
    Ambiguous,
    NotFound,
    Busy,
    Timeout,
    Limit,
    Cancelled,
    UnknownOutcome,
    NotActionable,
    Host,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Error {
    pub code: Code,
    pub message: String,
}
impl Error {
    pub fn new(code: Code, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.code, self.message)
    }
}
impl std::error::Error for Error {}
impl From<view_core::CoreError> for Error {
    fn from(e: view_core::CoreError) -> Self {
        Self::new(Code::Host, format!("{e:?}"))
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Selector {
    pub window: Option<String>,
    pub role: Option<String>,
    pub name: Option<String>,
    /// Explicit test ID registered by the host, not an implicit accessible name.
    pub test_id: Option<String>,
    pub ancestor: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Target {
    pub id: String,
    pub revision: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Predicate {
    Commit {
        revision: u64,
    },
    Presentation {
        window: String,
        revision: u64,
        boundary: String,
    },
    Exists {
        selector: Selector,
    },
    Idle,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    Discover,
    Snapshot,
    Inspector,
    Resolve {
        selector: Selector,
    },
    ActionPoint {
        target: Target,
    },
    Command {
        name: String,
        arguments: Value,
        target: Target,
    },
    StartTask {
        name: String,
        arguments: Value,
        target: Target,
    },
    /// Host-provided semantic setup, explicitly separate from physical wheel input.
    Realize {
        selector: Selector,
    },
    Advance {
        millis: u64,
    },
    Wait {
        predicate: Predicate,
        turns: u32,
    },
    Poll {
        operation: u64,
    },
    Cancel {
        operation: u64,
    },
    Capture {
        window: String,
        revision: u64,
    },
    FailureBundle,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub version: u32,
    pub session: String,
    pub id: u64,
    pub operation: Operation,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Response {
    pub id: u64,
    pub revision: u64,
    pub result: Result<Value, Error>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    Inspect,
    Mutate,
    Test,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Field {
    Boolean,
    Integer { min: i64, max: i64 },
    String { max_bytes: usize },
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Schema {
    pub fields: BTreeMap<String, Field>,
}
impl Schema {
    pub fn validate(&self, value: &Value) -> Result<(), Error> {
        let object = value
            .as_object()
            .ok_or_else(|| Error::new(Code::Invalid, "expected object"))?;
        if object.len() != self.fields.len() {
            return Err(Error::new(Code::Invalid, "missing or unknown fields"));
        }
        for (name, field) in &self.fields {
            let v = object
                .get(name)
                .ok_or_else(|| Error::new(Code::Invalid, format!("missing {name}")))?;
            let valid = match field {
                Field::Boolean => v.is_boolean(),
                Field::Integer { min, max } => v.as_i64().is_some_and(|n| n >= *min && n <= *max),
                Field::String { max_bytes } => v.as_str().is_some_and(|s| s.len() <= *max_bytes),
            };
            if !valid {
                return Err(Error::new(Code::Invalid, format!("invalid {name}")));
            }
        }
        Ok(())
    }
    pub fn json_schema(&self) -> Value {
        let properties: serde_json::Map<String, Value> = self.fields.iter().map(|(k,v)| {
            (k.clone(), match v {
                Field::Boolean => json!({"type":"boolean"}),
                Field::Integer{min,max} => json!({"type":"integer","minimum":min,"maximum":max}),
                Field::String{max_bytes} => json!({"type":"string","maxLength":max_bytes,"description":"UTF-8 byte limit also enforced"}),
            })
        }).collect();
        json!({"type":"object","properties":properties,"required":self.fields.keys().collect::<Vec<_>>(),"additionalProperties":false})
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommandDescriptor {
    pub name: String,
    pub description: String,
    pub scope: Scope,
    pub arguments: Schema,
    pub result: Schema,
}

/// Native calls never use these identity strings as pointers.
pub fn node_id(id: view_core::NodeId) -> String {
    handle_id(id.handle())
}
pub fn window_id(id: view_core::WindowId) -> String {
    handle_id(id.handle())
}
fn handle_id(h: view_core::ArenaHandle) -> String {
    format!("{}:{}:{}", h.arena().get(), h.index(), h.generation().get())
}
