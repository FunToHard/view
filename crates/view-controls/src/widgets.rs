use crate::*;
use view_core::{
    KeyboardEvent, LogicalRect, NodeId, PointerEvent, Role, SemanticAction, SemanticNode,
};

/// Additional control state supplements the core committed semantic node; all
/// adapters use the same generational NodeId, never a second control identity.
#[derive(Clone, Debug, PartialEq)]
pub struct ControlSemantics {
    pub node: SemanticNode,
    pub read_only: bool,
    pub mixed: bool,
    pub multiline: bool,
    pub invalid: bool,
    pub selection: Option<view_text::Selection>,
    pub range: Option<(f64, f64, f64, f64)>,
    pub expanded: Option<bool>,
    pub selected: Option<bool>,
    /// Full data dimensions and zero-based cell positions survive virtualization.
    pub table_dimensions: Option<(usize, usize)>,
    pub cell_position: Option<(usize, usize)>,
}
impl ControlSemantics {
    pub fn new(node: SemanticNode) -> Self {
        Self {
            node,
            read_only: false,
            mixed: false,
            multiline: false,
            invalid: false,
            selection: None,
            range: None,
            expanded: None,
            selected: None,
            table_dimensions: None,
            cell_position: None,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Label {
    pub text: String,
}
impl Label {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
    pub fn semantics(&self, id: NodeId, bounds: LogicalRect) -> ControlSemantics {
        let mut node = SemanticNode::new(id, Role::StaticText);
        node.name = Some(self.text.clone());
        node.bounds = bounds;
        ControlSemantics::new(node)
    }
}
/// Application-owned asset identity. The rendering adapter resolves it to a
/// live same-device image; loading/decoding never happens during build or paint.
#[derive(Clone, Debug)]
pub struct Image {
    pub asset: u64,
    pub description: String,
}
impl Image {
    pub fn semantics(&self, id: NodeId, bounds: LogicalRect) -> ControlSemantics {
        let mut node = SemanticNode::new(id, Role::Custom("image".into()));
        node.name = Some(self.description.clone());
        node.bounds = bounds;
        ControlSemantics::new(node)
    }
}
#[derive(Clone, Debug)]
pub struct Button {
    pub label: String,
    pub behavior: Pressable,
}
impl Button {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            behavior: Pressable::default(),
        }
    }
    pub fn pointer(&mut self, event: PointerEvent, inside: bool) -> bool {
        self.behavior.pointer(event, inside)
    }
    pub fn key(&mut self, event: &KeyboardEvent) -> bool {
        self.behavior.key(event)
    }
    pub fn activate(&self) -> Result<(), ControlError> {
        if self.behavior.state().disabled {
            Err(ControlError::Disabled)
        } else {
            Ok(())
        }
    }
    pub fn semantics(&self, id: NodeId, bounds: LogicalRect) -> ControlSemantics {
        let mut node = SemanticNode::new(id, Role::Button);
        let state = self.behavior.state();
        node.name = Some(self.label.clone());
        node.bounds = bounds;
        node.disabled = state.disabled;
        node.focused = state.focused;
        if !state.disabled {
            node.actions = vec![SemanticAction::Click, SemanticAction::Focus];
        }
        ControlSemantics::new(node)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckState {
    Off,
    On,
    Mixed,
}
#[derive(Clone, Debug)]
pub struct Checkbox {
    pub button: Button,
    state: CheckState,
    pub allow_mixed: bool,
}
impl Checkbox {
    pub fn new(
        label: impl Into<String>,
        state: CheckState,
        allow_mixed: bool,
    ) -> Result<Self, ControlError> {
        if state == CheckState::Mixed && !allow_mixed {
            return Err(ControlError::Invalid("mixed checkbox policy"));
        }
        Ok(Self {
            button: Button::new(label),
            state,
            allow_mixed,
        })
    }
    pub fn state(&self) -> CheckState {
        self.state
    }
    pub fn activate(&mut self) -> Result<CheckState, ControlError> {
        self.button.activate()?;
        self.state = match self.state {
            CheckState::Off => CheckState::On,
            CheckState::On if self.allow_mixed => CheckState::Mixed,
            _ => CheckState::Off,
        };
        Ok(self.state)
    }
    pub fn pointer(&mut self, e: PointerEvent, inside: bool) -> Result<bool, ControlError> {
        if self.button.pointer(e, inside) {
            self.activate()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
    pub fn key(&mut self, e: &KeyboardEvent) -> Result<bool, ControlError> {
        if self.button.key(e) {
            self.activate()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
    pub fn semantics(&self, id: NodeId, bounds: LogicalRect) -> ControlSemantics {
        let mut s = self.button.semantics(id, bounds);
        s.node.role = Role::Checkbox;
        s.node.checked = Some(self.state == CheckState::On);
        s.mixed = self.state == CheckState::Mixed;
        s
    }
}
pub type Toggle = Checkbox;
#[derive(Clone, Debug)]
pub struct RadioOption {
    pub key: String,
    pub label: String,
    pub disabled: bool,
}
pub struct RadioGroup {
    options: Vec<RadioOption>,
    selected: Option<String>,
    focused: Option<String>,
    required: bool,
    active: bool,
}
impl RadioGroup {
    pub fn new(
        options: Vec<RadioOption>,
        selected: Option<String>,
        required: bool,
    ) -> Result<Self, ControlError> {
        let mut keys = std::collections::HashSet::new();
        if options.iter().any(|o| !keys.insert(o.key.clone())) {
            return Err(ControlError::Invalid("duplicate radio key"));
        }
        let mut s = Self {
            options,
            selected: None,
            focused: None,
            required,
            active: false,
        };
        if let Some(key) = selected {
            s.select(&key)?;
        } else if required {
            let key = s
                .options
                .iter()
                .find(|o| !o.disabled)
                .ok_or(ControlError::Invalid(
                    "required radio has no enabled choice",
                ))?
                .key
                .clone();
            s.select(&key)?;
        }
        Ok(s)
    }
    pub fn options(&self) -> &[RadioOption] {
        &self.options
    }
    pub fn set_focused(&mut self, active: bool) {
        self.active = active;
    }
    /// Arrow keys select and focus the next enabled member; Tab belongs to the
    /// host focus manager and enters/leaves the group as one stop.
    pub fn key(&mut self, event: &KeyboardEvent) -> Result<bool, ControlError> {
        use view_core::{Key, KeyPhase, NamedKey};
        if event.phase == KeyPhase::Up
            || event.modifiers.ctrl
            || event.modifiers.alt
            || event.modifiers.meta
        {
            return Ok(false);
        }
        match event.key {
            Key::Named(NamedKey::ArrowDown | NamedKey::ArrowRight) => self.move_choice(true)?,
            Key::Named(NamedKey::ArrowUp | NamedKey::ArrowLeft) => self.move_choice(false)?,
            _ => return Ok(false),
        }
        Ok(true)
    }
    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }
    pub fn select(&mut self, key: &str) -> Result<(), ControlError> {
        let o = self
            .options
            .iter()
            .find(|o| o.key == key)
            .ok_or(ControlError::Stale)?;
        if o.disabled {
            return Err(ControlError::Disabled);
        }
        self.selected = Some(key.into());
        self.focused = Some(key.into());
        Ok(())
    }
    pub fn clear(&mut self) -> Result<(), ControlError> {
        if self.required {
            return Err(ControlError::Invalid("required radio selection"));
        }
        self.selected = None;
        Ok(())
    }
    pub fn move_choice(&mut self, forward: bool) -> Result<(), ControlError> {
        let enabled: Vec<_> = self
            .options
            .iter()
            .filter(|o| !o.disabled)
            .map(|o| o.key.clone())
            .collect();
        if enabled.is_empty() {
            return Err(ControlError::Disabled);
        }
        let current = self
            .focused
            .as_ref()
            .and_then(|key| enabled.iter().position(|k| k == key));
        let index = current.map_or(0, |i| {
            if forward {
                (i + 1) % enabled.len()
            } else {
                (i + enabled.len() - 1) % enabled.len()
            }
        });
        self.select(&enabled[index])
    }
    pub fn semantics(
        &self,
        index: usize,
        id: NodeId,
        bounds: LogicalRect,
    ) -> Result<ControlSemantics, ControlError> {
        let o = self.options.get(index).ok_or(ControlError::Stale)?;
        let mut node = SemanticNode::new(id, Role::Radio);
        node.bounds = bounds;
        node.name = Some(o.label.clone());
        node.checked = Some(self.selected.as_ref() == Some(&o.key));
        node.focused = self.active && self.focused.as_ref() == Some(&o.key);
        node.disabled = o.disabled;
        if !o.disabled {
            node.actions = vec![SemanticAction::Click, SemanticAction::Focus];
        }
        Ok(ControlSemantics::new(node))
    }
}
pub struct Slider {
    pub range: RangeBehavior,
    pub focused: bool,
    dragging: bool,
}
impl Slider {
    pub fn new(spec: RangeSpec, value: f64) -> Result<Self, ControlError> {
        Ok(Self {
            range: RangeBehavior::new(spec, value)?,
            focused: false,
            dragging: false,
        })
    }
    pub fn pointer(&mut self, e: PointerEvent, bounds: LogicalRect) -> Result<bool, ControlError> {
        if !bounds.is_valid() || bounds.size.width <= 0.0 {
            return Err(ControlError::Invalid("slider bounds"));
        }
        if self.range.disabled {
            self.dragging = false;
            return Ok(false);
        }
        match e.phase {
            view_core::PointerPhase::Down
                if e.button == Some(view_core::PointerButton::Primary)
                    && bounds.contains(e.position) =>
            {
                self.dragging = true;
                self.focused = true;
            }
            view_core::PointerPhase::Cancel => {
                self.dragging = false;
                return Ok(false);
            }
            view_core::PointerPhase::Up if e.button == Some(view_core::PointerButton::Primary) => {
                if !self.dragging {
                    return Ok(false);
                }
                self.dragging = false;
                return self
                    .range
                    .set_fraction(((e.position.x - bounds.origin.x) / bounds.size.width) as f64);
            }
            _ => {}
        }
        if self.dragging {
            self.range
                .set_fraction(((e.position.x - bounds.origin.x) / bounds.size.width) as f64)
        } else {
            Ok(false)
        }
    }
    pub fn key(&mut self, event: &KeyboardEvent) -> Result<bool, ControlError> {
        if self.focused {
            self.range.key(event)
        } else {
            Ok(false)
        }
    }
    pub fn cancel(&mut self) {
        self.dragging = false;
    }
    pub fn semantics(&self, id: NodeId, bounds: LogicalRect) -> ControlSemantics {
        let mut node = SemanticNode::new(id, Role::Slider);
        node.bounds = bounds;
        node.disabled = self.range.disabled;
        node.focused = self.focused;
        node.value = Some(self.range.value().to_string());
        if !node.disabled {
            node.actions = vec![SemanticAction::Focus, SemanticAction::SetValue];
        }
        let mut s = ControlSemantics::new(node);
        let r = self.range.spec();
        s.range = Some((r.min, r.max, self.range.value(), r.step));
        s
    }
}
