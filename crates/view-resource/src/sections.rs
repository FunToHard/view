//! A typed bounded schema adapter. Parsing/wire-byte limits belong to the transport.
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub struct Section {
    pub key: String,
    pub control: String,
    pub mandatory: bool,
    pub text: String,
    pub action: Option<String>,
    pub asset: Option<String>,
    pub children: Vec<Section>,
}
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub depth: usize,
    pub items: usize,
    pub text_bytes: usize,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SchemaError {
    Version,
    Limit,
    DuplicateKey,
    UnknownControl,
    ForbiddenAction,
    InvalidAsset,
}
#[derive(Clone, Debug)]
pub struct LocalSection<A> {
    pub key: String,
    pub control: String,
    pub text: String,
    pub action: Option<A>,
    pub asset: Option<String>,
    pub children: Vec<LocalSection<A>>,
    pub fallback: bool,
}
/// Local allowlists are the capability boundary. Remote strings cannot add capabilities.
pub struct Registry<A> {
    pub version: u32,
    controls: BTreeSet<String>,
    actions: BTreeMap<String, A>,
    assets: BTreeSet<String>,
    limits: Limits,
}
impl<A: Clone> Registry<A> {
    pub fn new(
        version: u32,
        controls: impl IntoIterator<Item = String>,
        actions: BTreeMap<String, A>,
        assets: BTreeSet<String>,
        limits: Limits,
    ) -> Self {
        Self {
            version,
            controls: controls.into_iter().collect(),
            actions,
            assets,
            limits,
        }
    }
    pub fn compile(
        &self,
        version: u32,
        sections: &[Section],
    ) -> Result<Vec<LocalSection<A>>, SchemaError> {
        if version != self.version {
            return Err(SchemaError::Version);
        }
        let mut items = 0;
        let mut bytes = 0;
        self.walk(sections, 1, &mut items, &mut bytes)
    }
    fn walk(
        &self,
        sections: &[Section],
        depth: usize,
        items: &mut usize,
        bytes: &mut usize,
    ) -> Result<Vec<LocalSection<A>>, SchemaError> {
        if !sections.is_empty() && depth > self.limits.depth {
            return Err(SchemaError::Limit);
        }
        let mut keys = BTreeSet::new();
        let mut output = vec![];
        for s in sections {
            *items = items.checked_add(1).ok_or(SchemaError::Limit)?;
            *bytes = bytes
                .checked_add(s.text.len())
                .and_then(|n| n.checked_add(s.key.len()))
                .and_then(|n| n.checked_add(s.control.len()))
                .and_then(|n| n.checked_add(s.action.as_ref().map_or(0, String::len)))
                .and_then(|n| n.checked_add(s.asset.as_ref().map_or(0, String::len)))
                .ok_or(SchemaError::Limit)?;
            if *items > self.limits.items || *bytes > self.limits.text_bytes || s.key.is_empty() {
                return Err(SchemaError::Limit);
            }
            if !keys.insert(&s.key) {
                return Err(SchemaError::DuplicateKey);
            }
            let known = self.controls.contains(&s.control);
            if !known && s.mandatory {
                return Err(SchemaError::UnknownControl);
            }
            let action = s
                .action
                .as_ref()
                .map(|a| {
                    self.actions
                        .get(a)
                        .cloned()
                        .ok_or(SchemaError::ForbiddenAction)
                })
                .transpose()?;
            if s.asset.as_ref().is_some_and(|a| !self.assets.contains(a)) {
                return Err(SchemaError::InvalidAsset);
            }
            let children = self.walk(&s.children, depth + 1, items, bytes)?;
            output.push(LocalSection {
                key: s.key.clone(),
                control: if known {
                    s.control.clone()
                } else {
                    "fallback".into()
                },
                text: if known {
                    s.text.clone()
                } else {
                    "Unsupported optional section".into()
                },
                action: if known { action } else { None },
                asset: if known { s.asset.clone() } else { None },
                children: if known { children } else { vec![] },
                fallback: !known,
            });
        }
        Ok(output)
    }
}
