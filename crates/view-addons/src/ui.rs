use crate::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};
use view_controls::{
    Button, TextField,
    view_text::{Clipboard, EditorConfig, ExternalPolicy},
};
use view_core::{
    Description, Dirty, LogicalRect, NodeId, Role, Runtime, SemanticAction, Structure,
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Label,
    Button,
    Text,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Props {
    pub label: String,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub value_revision: Option<u64>,
}
impl Props {
    fn validate(&self, kind: Kind) -> Result<()> {
        if self.label.len() > 4096
            || self.value.as_ref().is_some_and(|v| v.len() > 4096)
            || self.command.as_ref().is_some_and(|s| !identifier(s))
        {
            return Err(Error::Limit);
        }
        if (kind == Kind::Button && self.command.is_none())
            || (kind == Kind::Text && self.value.is_none())
        {
            return Err(Error::Invalid("component properties".into()));
        }
        Ok(())
    }
}
pub struct ComponentSchema {
    pub version: u32,
    pub kind: Kind,
    pub decode: fn(&Value) -> Result<Props>,
}
#[derive(Default)]
pub struct Registry {
    schemas: BTreeMap<String, ComponentSchema>,
}
impl Registry {
    pub fn standard() -> Self {
        let mut s = Self::default();
        for (name, kind) in [
            ("label", Kind::Label),
            ("button", Kind::Button),
            ("text", Kind::Text),
        ] {
            s.register(
                name,
                ComponentSchema {
                    version: 1,
                    kind,
                    decode: |v| {
                        serde_json::from_value(v.clone()).map_err(|e| Error::Invalid(e.to_string()))
                    },
                },
            )
            .expect("standard schema");
        }
        s
    }
    pub fn register(&mut self, name: &str, schema: ComponentSchema) -> Result<()> {
        if !identifier(name) || self.schemas.contains_key(name) || self.schemas.len() >= 64 {
            return Err(Error::Invalid("component registry".into()));
        }
        self.schemas.insert(name.into(), schema);
        Ok(())
    }
    pub(crate) fn negotiate(&self, manifest: &Manifest) -> Result<BTreeMap<String, String>> {
        let mut result = BTreeMap::new();
        for c in &manifest.components {
            let selected = if self
                .schemas
                .get(&c.name)
                .is_some_and(|s| s.version == c.version)
            {
                Some(c.name.clone())
            } else if !c.required {
                c.fallback
                    .clone()
                    .filter(|n| self.schemas.get(n).is_some_and(|s| s.version == 1))
            } else {
                None
            };
            match selected {
                Some(name) => {
                    result.insert(c.name.clone(), name);
                }
                None if c.required => return Err(Error::Incompatible(c.name.clone())),
                None => (),
            }
        }
        Ok(result)
    }
    pub(crate) fn decode(&self, name: &str, props: &Value) -> Result<(Kind, Props)> {
        let schema = self
            .schemas
            .get(name)
            .ok_or_else(|| Error::Incompatible(name.into()))?;
        let props = (schema.decode)(props)?;
        props.validate(schema.kind)?;
        Ok((schema.kind, props))
    }
}
pub enum Control {
    Label(String),
    Button(Button),
    Text(Box<TextField>),
}
impl Control {
    fn new(kind: Kind, p: &Props) -> Result<Self> {
        Ok(match kind {
            Kind::Label => Self::Label(p.label.clone()),
            Kind::Button => Self::Button(Button::new(&p.label)),
            Kind::Text => Self::Text(Box::new(
                TextField::new(
                    &p.label,
                    p.value.as_deref().unwrap_or_default(),
                    EditorConfig {
                        max_graphemes: Some(4096),
                        ..Default::default()
                    },
                )
                .map_err(|e| Error::Host(e.to_string()))?,
            )),
        })
    }
    fn value(&self) -> Option<(String, u64)> {
        match self {
            Self::Text(t) => Some((
                t.snapshot().text.text().to_string(),
                t.snapshot().text.revision().get(),
            )),
            _ => None,
        }
    }
    fn unmount(&mut self) {
        match self {
            Self::Text(t) => t.unmount(0),
            Self::Button(b) => b.behavior.set_focused(false),
            _ => (),
        }
    }
}
#[derive(Clone)]
pub(crate) struct Entry {
    pub node: UiNode,
    pub kind: Kind,
    pub props: Props,
    pub control: Rc<RefCell<Control>>,
}
/// Host-owned runtime nodes. The host chooses slot parents; guests cannot name arbitrary parents.
#[derive(Default)]
pub struct MountedUi {
    roots: BTreeMap<Slot, NodeId>,
    nodes: BTreeMap<String, NodeId>,
    entries: BTreeMap<String, Entry>,
}
#[derive(Clone, Debug)]
pub struct LocalEvent {
    pub key: String,
    pub command: Option<String>,
    pub value: Option<String>,
    pub value_revision: u64,
}
pub enum Input {
    Activate,
    Pointer(view_core::PointerEvent),
    Key(view_core::KeyboardEvent),
    Text(String, u64),
    Preedit(String, Option<(usize, usize)>),
    Commit(String, u64),
}
impl MountedUi {
    /// Build host paint records from retained controls; the host owns GPU submission and IME.
    #[cfg(feature = "render")]
    pub fn paint<M, A>(
        &mut self,
        runtime: &Runtime<M, A>,
        fonts: &mut view_controls::view_text::CosmicBackend,
        style: &view_controls::Style,
        scale: view_core::ScaleFactor,
        now: u64,
    ) -> Result<Vec<view_wgpu::Paint>> {
        let mut items = vec![];
        for (key, id) in &self.nodes {
            let node = runtime
                .snapshot()
                .nodes
                .iter()
                .find(|n| n.id == *id)
                .ok_or(Error::Stale)?;
            if node.semantic.as_ref().is_some_and(|s| s.hidden) {
                continue;
            }
            let bounds = runtime.global_rect(*id)?;
            let (transform, clip) = runtime.diagnostic_geometry(*id)?;
            if transform != view_core::Transform2D::translation(bounds.origin.x, bounds.origin.y) {
                return Err(Error::Incompatible(
                    "built-in add-on painter supports translated rectangular slots".into(),
                ));
            }
            let first = items.len();
            let mut control = self.entries[key].control.borrow_mut();
            match &mut *control {
                Control::Label(s) => items.push(
                    view_controls::Label::new(s.clone())
                        .paint(fonts, style, bounds, scale)
                        .map_err(|e| Error::Host(e.to_string()))?,
                ),
                Control::Button(b) => {
                    b.behavior
                        .set_focused(runtime.focused(node.window)? == Some(*id));
                    let style = style.interaction(b.behavior.state());
                    items.extend(view_controls::frame(bounds, &style));
                    let padding = style.padding;
                    let label_bounds = LogicalRect::from_xywh(
                        bounds.origin.x + padding,
                        bounds.origin.y + padding,
                        (bounds.size.width - 2. * padding).max(0.),
                        (bounds.size.height - 2. * padding).max(0.),
                    );
                    items.push(
                        view_controls::Label::new(b.label.clone())
                            .paint(fonts, &style, label_bounds, scale)
                            .map_err(|e| Error::Host(e.to_string()))?,
                    );
                }
                Control::Text(t) => {
                    let focused = runtime.focused(node.window)? == Some(*id);
                    if t.focused() != focused {
                        t.set_focused(focused, now);
                    }
                    items.extend(view_controls::frame(bounds, style));
                    let presentation = t
                        .prepare(fonts, style, bounds, scale, now)
                        .map_err(|e| Error::Host(e.to_string()))?;
                    items.extend(
                        presentation
                            .paint(style)
                            .map_err(|e| Error::Host(e.to_string()))?,
                    );
                }
            }
            for item in &mut items[first..] {
                if let view_wgpu::Paint::Draw(draw) = item
                    && let Some(clip) = clip
                {
                    draw.clip = Some(draw.clip.map_or(clip, |local| {
                        local.intersection(clip).unwrap_or(LogicalRect::ZERO)
                    }));
                }
            }
        }
        Ok(items)
    }
    pub fn node(&self, key: &str) -> Option<NodeId> {
        self.nodes.get(key).copied()
    }
    /// Shared control engines are also available to the host's shaping/paint/input adapter.
    pub fn control(&self, key: &str) -> Option<Rc<RefCell<Control>>> {
        self.entries.get(key).map(|e| e.control.clone())
    }
    pub(crate) fn prepare(
        &self,
        nodes: &[UiNode],
        manifest: &Manifest,
        registry: &Registry,
        selected: &BTreeMap<String, String>,
        max_nodes: usize,
        max_bytes: usize,
    ) -> Result<BTreeMap<String, Entry>> {
        if nodes.len() > max_nodes || encode(&nodes)?.len() > max_bytes {
            return Err(Error::Limit);
        }
        let mut entries = BTreeMap::new();
        for node in nodes {
            if !identifier(&node.key)
                || !manifest.slots.contains(&node.slot)
                || entries.contains_key(&node.key)
            {
                return Err(Error::Denied);
            }
            let name = selected
                .get(&node.component)
                .ok_or_else(|| Error::Incompatible(node.component.clone()))?;
            let (kind, props) = registry.decode(name, &node.props)?;
            if props
                .command
                .as_ref()
                .is_some_and(|s| !manifest.commands.contains(s))
            {
                return Err(Error::Denied);
            }
            let control = if let Some(old) = self.entries.get(&node.key).filter(|old| {
                old.kind == kind
                    && old.node.component == node.component
                    && old.node.slot == node.slot
            }) {
                if let Control::Text(t) = &*old.control.borrow() {
                    let snap = t.snapshot();
                    // An unchanged guest echo never overwrites locally edited text or IME.
                    if props.value != old.props.value
                        && (props.value_revision != Some(snap.text.revision().get())
                            || snap.composition.is_some())
                    {
                        return Err(Error::Stale);
                    }
                }
                old.control.clone()
            } else {
                Rc::new(RefCell::new(Control::new(kind, &props)?))
            };
            entries.insert(
                node.key.clone(),
                Entry {
                    node: node.clone(),
                    kind,
                    props,
                    control,
                },
            );
        }
        Ok(entries)
    }
    pub(crate) fn apply<M, A>(
        &mut self,
        runtime: &mut Runtime<M, A>,
        namespace: &str,
        parents: &BTreeMap<Slot, NodeId>,
        entries: BTreeMap<String, Entry>,
    ) -> Result<()> {
        // All guest schemas, bounds and revision preconditions were checked before mutation.
        for (slot, parent) in parents {
            if let Some(root) = self.roots.get(slot) {
                runtime.structure(*root)?;
            } else if runtime.structure(*parent)? != Structure::Retained {
                return Err(Error::Host("slot parent must retain its children".into()));
            }
        }
        for (key, e) in &entries {
            if let Some(old) = self.entries.get(key) {
                let mut c = e.control.borrow_mut();
                match &mut *c {
                    Control::Label(s) => *s = e.props.label.clone(),
                    Control::Button(b) => b.label = e.props.label.clone(),
                    Control::Text(t) => {
                        t.label = e.props.label.clone();
                        if old.props.value != e.props.value {
                            let revision = t.snapshot().text.revision();
                            t.external(
                                revision,
                                e.props.value.as_deref().unwrap_or_default(),
                                ExternalPolicy::PreserveSelection,
                            )
                            .map_err(|e| Error::Host(e.to_string()))?;
                        }
                    }
                }
            }
        }
        for (key, old) in &self.entries {
            if entries
                .get(key)
                .is_none_or(|e| !Rc::ptr_eq(&e.control, &old.control))
            {
                old.control.borrow_mut().unmount();
            }
        }
        self.entries = entries;
        self.refresh(runtime, namespace, parents)
    }
    pub(crate) fn refresh<M, A>(
        &mut self,
        runtime: &mut Runtime<M, A>,
        namespace: &str,
        parents: &BTreeMap<Slot, NodeId>,
    ) -> Result<()> {
        self.nodes.clear();
        for (slot, parent) in parents {
            let root = if let Some(root) = self.roots.get(slot) {
                *root
            } else {
                let root = runtime.mount(
                    *parent,
                    Description::new(format!("{namespace}/{slot:?}"), (), |_, _, _| Dirty::NONE)
                        .structure(Structure::Declarative)
                        .role(Role::Container),
                )?;
                self.roots.insert(slot.clone(), root);
                root
            };
            let mut keys = vec![];
            let descriptions = self
                .entries
                .iter()
                .filter(|(_, e)| &e.node.slot == slot)
                .enumerate()
                .map(|(index, (key, e))| {
                    keys.push(key.clone());
                    let (role, actions) = match e.kind {
                        Kind::Label => (Role::StaticText, vec![]),
                        Kind::Button => (
                            Role::Button,
                            vec![SemanticAction::Click, SemanticAction::Focus],
                        ),
                        Kind::Text => (
                            Role::TextInput,
                            vec![SemanticAction::SetValue, SemanticAction::Focus],
                        ),
                    };
                    let value = e.control.borrow().value().map(|v| v.0).unwrap_or_default();
                    Description::new(
                        format!("{namespace}/{key}/{}", e.node.component),
                        e.control.clone(),
                        |_, _, _| Dirty::NONE,
                    )
                    .role(role)
                    .name(&e.props.label)
                    .value(value)
                    .semantic_actions(actions)
                    .focusable(e.kind != Kind::Label)
                    .layout(LogicalRect::from_xywh(
                        0.,
                        index as f32 * 48.,
                        280.,
                        44.,
                    ))
                })
                .collect();
            let ids = runtime.reconcile(root, descriptions)?;
            self.nodes.extend(keys.into_iter().zip(ids));
        }
        Ok(())
    }
    pub(crate) fn input<M, A>(
        &mut self,
        runtime: &mut Runtime<M, A>,
        key: &str,
        input: Input,
        clipboard: &mut impl Clipboard,
    ) -> Result<Option<LocalEvent>> {
        let id = self.node(key).ok_or(Error::Stale)?;
        let snapshot = runtime.snapshot();
        let node = snapshot
            .nodes
            .iter()
            .find(|n| n.id == id)
            .ok_or(Error::Stale)?;
        if !node.enabled
            || runtime
                .modal_scope(node.window)?
                .is_some_and(|modal| modal != id && !runtime.is_descendant(modal, id))
        {
            return Err(Error::Denied);
        }
        let mut ancestor = Some(id);
        while let Some(id) = ancestor {
            let n = snapshot
                .nodes
                .iter()
                .find(|n| n.id == id)
                .ok_or(Error::Stale)?;
            if n.visibility != view_core::Visibility::Visible {
                return Err(Error::Denied);
            }
            ancestor = n.parent;
        }
        let e = self.entries.get(key).ok_or(Error::Stale)?;
        let mut c = e.control.borrow_mut();
        let focused = runtime.focused(node.window)? == Some(id);
        let mut activated = false;
        match (&mut *c, input) {
            (Control::Button(b), Input::Activate) => {
                b.activate().map_err(|e| Error::Host(e.to_string()))?;
                activated = true;
            }
            (Control::Button(b), Input::Key(event)) => {
                b.behavior.set_focused(focused);
                activated = b.key(&event);
            }
            (Control::Button(b), Input::Pointer(event)) => {
                activated = b.pointer(event, runtime.global_rect(id)?.contains(event.position));
            }
            (Control::Text(t), input) => {
                // Input routing and window/modal focus are the host runtime's authority.
                let focused = runtime
                    .snapshot()
                    .nodes
                    .iter()
                    .find(|n| n.id == id)
                    .is_some_and(|n| runtime.focused(n.window).ok().flatten() == Some(id));
                if !focused {
                    return Err(Error::Denied);
                }
                if !t.focused() {
                    t.set_focused(true, 0);
                }
                match input {
                    Input::Key(event) => {
                        t.key(&event, clipboard)
                            .map_err(|e| Error::Host(e.to_string()))?;
                    }
                    Input::Text(s, now) => t
                        .insert_text(&s, now)
                        .map_err(|e| Error::Host(e.to_string()))?,
                    Input::Preedit(s, cursor) => t
                        .preedit(&s, cursor)
                        .map_err(|e| Error::Host(e.to_string()))?,
                    Input::Commit(s, now) => t
                        .commit_ime(&s, now)
                        .map_err(|e| Error::Host(e.to_string()))?,
                    Input::Pointer(event) => {
                        t.pointer(event, runtime.global_rect(id)?)
                            .map_err(|e| Error::Host(e.to_string()))?;
                    }
                    _ => return Err(Error::Denied),
                }
            }
            _ => return Err(Error::Denied),
        }
        let (value, value_revision) = c.value().map(|(s, r)| (Some(s), r)).unwrap_or((None, 0));
        runtime.update::<Rc<RefCell<Control>>, _>(
            id,
            Dirty::PAINT.union(Dirty::SEMANTICS),
            |_| (),
        )?;
        if matches!(*c, Control::Button(_)) && !activated {
            return Ok(None);
        }
        Ok(Some(LocalEvent {
            key: key.into(),
            command: e.props.command.clone(),
            value,
            value_revision,
        }))
    }
    pub(crate) fn validation(
        &mut self,
        key: &str,
        revision: u64,
        error: Option<String>,
    ) -> Result<()> {
        if error.as_ref().is_some_and(|e| e.len() > 4096) {
            return Err(Error::Limit);
        }
        let entry = self.entries.get(key).ok_or(Error::Stale)?;
        let mut c = entry.control.borrow_mut();
        match &mut *c {
            Control::Text(t) if t.snapshot().text.revision().get() == revision => {
                t.invalid = error.is_some();
                Ok(())
            }
            _ => Err(Error::Stale),
        }
    }
    pub(crate) fn sync_focus<M, A>(&mut self, runtime: &Runtime<M, A>, now: u64) -> Result<()> {
        for (key, id) in &self.nodes {
            let Some(node) = runtime.snapshot().nodes.iter().find(|n| n.id == *id) else {
                continue;
            };
            let focused = runtime.focused(node.window)? == Some(*id);
            let mut c = self.entries[key].control.borrow_mut();
            match &mut *c {
                Control::Text(t) if t.focused() != focused => t.set_focused(focused, now),
                Control::Button(b) => b.behavior.set_focused(focused),
                _ => (),
            }
        }
        Ok(())
    }
    pub(crate) fn clear<M, A>(&mut self, runtime: &mut Runtime<M, A>) -> Result<()> {
        for e in self.entries.values() {
            e.control.borrow_mut().unmount();
        }
        for (_, root) in std::mem::take(&mut self.roots) {
            if runtime.structure(root).is_ok() {
                runtime.unmount(root)?;
            }
        }
        self.entries.clear();
        self.nodes.clear();
        Ok(())
    }
}
