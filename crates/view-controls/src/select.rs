//! Non-editable select/1. PopupStack is authoritative for visibility and focus return.
use crate::{ControlError, ControlSemantics, Popup, PopupStack, RadioOption};
use view_core::{
    Key, KeyPhase, KeyboardEvent, LogicalRect, NamedKey, NodeId, Role, SemanticAction, SemanticNode,
};

pub type SelectOption = RadioOption;

/// A response never executes application effects. `handled == false` on Tab
/// allows ordinary host traversal after the popup has been dismissed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SelectResponse {
    pub handled: bool,
    pub changed: bool,
    pub return_focus: Option<NodeId>,
}

pub struct Select {
    pub label: String,
    options: Vec<SelectOption>,
    selected: Option<String>,
    active: Option<String>,
    popup: Option<NodeId>,
    disabled: bool,
    focused: bool,
    prefix: String,
    prefix_time: u64,
}
impl Select {
    pub fn new(
        label: impl Into<String>,
        options: Vec<SelectOption>,
        selected: Option<String>,
    ) -> Result<Self, ControlError> {
        let mut result = Self {
            label: label.into(),
            options: vec![],
            selected: None,
            active: None,
            popup: None,
            disabled: false,
            focused: false,
            prefix: String::new(),
            prefix_time: 0,
        };
        result.replace(options)?;
        if let Some(key) = selected {
            result.select(&key)?;
        }
        Ok(result)
    }
    pub fn options(&self) -> &[SelectOption] {
        &self.options
    }
    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }
    pub fn active(&self) -> Option<&str> {
        self.active.as_deref()
    }
    pub fn disabled(&self) -> bool {
        self.disabled
    }
    pub fn visible_options(
        &self,
        viewport_height: f32,
        row_height: f32,
    ) -> Result<std::ops::Range<usize>, ControlError> {
        if !viewport_height.is_finite()
            || viewport_height < 0.0
            || !row_height.is_finite()
            || row_height <= 0.0
        {
            return Err(ControlError::Invalid("select viewport"));
        }
        let capacity = (viewport_height / row_height).floor() as usize;
        if capacity == 0 {
            return Ok(0..0);
        }
        let active = self
            .options
            .iter()
            .position(|option| Some(option.key.as_str()) == self.active())
            .unwrap_or(0);
        let first = active.saturating_sub(capacity - 1);
        Ok(first..first.saturating_add(capacity).min(self.options.len()))
    }
    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused && !self.disabled;
    }
    pub fn is_open(&self, stack: &PopupStack) -> bool {
        self.popup.is_some_and(|id| stack.contains(id))
    }
    /// Keyed updates preserve enabled selections; removal/disable clears selection.
    /// Duplicate keys reject atomically. Empty and all-disabled data are valid.
    pub fn replace(&mut self, options: Vec<SelectOption>) -> Result<(), ControlError> {
        let mut keys = std::collections::HashSet::new();
        if options
            .iter()
            .any(|option| !keys.insert(option.key.as_str()))
        {
            return Err(ControlError::Invalid("duplicate select key"));
        }
        let enabled = |key: &String| {
            options
                .iter()
                .any(|option| &option.key == key && !option.disabled)
        };
        self.selected = self.selected.take().filter(enabled);
        self.active = self
            .active
            .take()
            .filter(enabled)
            .or_else(|| self.selected.clone())
            .or_else(|| {
                options
                    .iter()
                    .find(|option| !option.disabled)
                    .map(|option| option.key.clone())
            });
        self.options = options;
        self.prefix.clear();
        Ok(())
    }
    pub fn select(&mut self, key: &str) -> Result<bool, ControlError> {
        if self.disabled {
            return Err(ControlError::Disabled);
        }
        let option = self
            .options
            .iter()
            .find(|option| option.key == key)
            .ok_or(ControlError::Stale)?;
        if option.disabled {
            return Err(ControlError::Disabled);
        }
        let changed = self.selected.as_deref() != Some(key);
        self.selected = Some(key.into());
        self.active = Some(key.into());
        Ok(changed)
    }
    pub fn open(&mut self, stack: &mut PopupStack, popup: Popup) -> Result<(), ControlError> {
        if self.disabled || !self.options.iter().any(|option| !option.disabled) {
            return Err(ControlError::Disabled);
        }
        if popup.modal {
            return Err(ControlError::Invalid("select popup must be nonmodal"));
        }
        if self.is_open(stack) {
            return Ok(());
        }
        stack.open(popup)?;
        self.popup = Some(popup.content);
        self.active = self.selected.clone().or_else(|| {
            self.options
                .iter()
                .find(|option| !option.disabled)
                .map(|option| option.key.clone())
        });
        self.prefix.clear();
        Ok(())
    }
    pub fn close(
        &mut self,
        stack: &mut PopupStack,
        live: impl Fn(NodeId) -> bool,
    ) -> Option<NodeId> {
        self.prefix.clear();
        self.active = self.selected.clone();
        self.popup.take().and_then(|id| stack.dismiss(id, live))
    }
    pub fn set_disabled(
        &mut self,
        disabled: bool,
        stack: &mut PopupStack,
        live: impl Fn(NodeId) -> bool,
    ) -> Option<NodeId> {
        self.disabled = disabled;
        if disabled {
            self.focused = false;
            self.close(stack, live)
        } else {
            None
        }
    }
    /// Pointer and semantic option activation share this commit path.
    pub fn activate(
        &mut self,
        key: &str,
        stack: &mut PopupStack,
        live: impl Fn(NodeId) -> bool,
    ) -> Result<SelectResponse, ControlError> {
        if !self.is_open(stack) {
            return Err(ControlError::Stale);
        }
        let changed = self.select(key)?;
        let return_focus = self.close(stack, live);
        Ok(SelectResponse {
            handled: true,
            changed,
            return_focus,
        })
    }
    fn navigate(&mut self, key: NamedKey) {
        let current = self
            .active
            .as_ref()
            .and_then(|key| self.options.iter().position(|option| &option.key == key));
        let next = match key {
            NamedKey::Home => self.options.iter().position(|option| !option.disabled),
            NamedKey::End => self.options.iter().rposition(|option| !option.disabled),
            NamedKey::ArrowUp => (0..current.unwrap_or(self.options.len()))
                .rev()
                .find(|index| !self.options[*index].disabled)
                .or(current),
            _ => (current.map_or(0, |index| index + 1)..self.options.len())
                .find(|index| !self.options[*index].disabled)
                .or(current),
        };
        self.active = next.map(|index| self.options[index].key.clone());
    }
    pub fn key(
        &mut self,
        event: &KeyboardEvent,
        stack: &mut PopupStack,
        popup: Popup,
        live: impl Fn(NodeId) -> bool,
    ) -> Result<SelectResponse, ControlError> {
        let ignored = SelectResponse::default();
        if self.disabled
            || !self.focused
            || event.phase == KeyPhase::Up
            || event.modifiers.ctrl
            || event.modifiers.meta
        {
            return Ok(ignored);
        }
        let open = self.is_open(stack);
        if event.modifiers.alt {
            if event.key == Key::Named(NamedKey::ArrowDown) && event.phase == KeyPhase::Down {
                self.open(stack, popup)?;
            } else if event.key == Key::Named(NamedKey::ArrowUp) && open {
                return Ok(SelectResponse {
                    handled: true,
                    return_focus: self.close(stack, live),
                    changed: false,
                });
            } else {
                return Ok(ignored);
            }
        } else {
            match &event.key {
                Key::Named(NamedKey::Escape | NamedKey::Tab) if open => {
                    return Ok(SelectResponse {
                        handled: event.key != Key::Named(NamedKey::Tab),
                        changed: false,
                        return_focus: self.close(stack, live),
                    });
                }
                Key::Named(NamedKey::Enter | NamedKey::Space) if event.phase == KeyPhase::Down => {
                    if open {
                        if let Some(key) = self.active.clone() {
                            return self.activate(&key, stack, live);
                        }
                    } else {
                        self.open(stack, popup)?;
                    }
                }
                Key::Named(
                    key
                    @ (NamedKey::ArrowUp | NamedKey::ArrowDown | NamedKey::Home | NamedKey::End),
                ) => {
                    if !open {
                        self.open(stack, popup)?;
                    }
                    self.navigate(*key);
                }
                Key::Character(text) if !text.is_empty() && !text.chars().any(char::is_control) => {
                    if !open {
                        self.open(stack, popup)?;
                    }
                    if event.timestamp.saturating_sub(self.prefix_time) > 700_000 {
                        self.prefix.clear();
                    }
                    self.prefix_time = event.timestamp;
                    let text = text.to_lowercase();
                    if self.prefix == text {
                        self.prefix.clear();
                    }
                    // Bound typeahead independently of application data size.
                    if self.prefix.len() + text.len() > 256 {
                        self.prefix.clear();
                    }
                    self.prefix.push_str(&text);
                    let start = self
                        .active
                        .as_ref()
                        .and_then(|key| self.options.iter().position(|option| &option.key == key))
                        .map_or(0, |index| index + 1);
                    for offset in 0..self.options.len() {
                        let option = &self.options[(start + offset) % self.options.len()];
                        if !option.disabled && option.label.to_lowercase().starts_with(&self.prefix)
                        {
                            self.active = Some(option.key.clone());
                            break;
                        }
                    }
                }
                _ => return Ok(ignored),
            }
        }
        Ok(SelectResponse {
            handled: true,
            ..Default::default()
        })
    }
    pub fn semantics(
        &self,
        id: NodeId,
        bounds: LogicalRect,
        stack: &PopupStack,
    ) -> ControlSemantics {
        let mut node = SemanticNode::new(id, Role::Custom("select".into()));
        node.name = Some(self.label.clone());
        node.bounds = bounds;
        node.disabled = self.disabled;
        node.focused = self.focused;
        node.value = self
            .selected
            .as_ref()
            .and_then(|key| self.options.iter().find(|option| &option.key == key))
            .map(|option| option.label.clone());
        let open = self.is_open(stack);
        if !self.disabled {
            node.actions = vec![
                SemanticAction::Focus,
                if open {
                    SemanticAction::Collapse
                } else {
                    SemanticAction::Expand
                },
            ];
        }
        let mut state = ControlSemantics::new(node);
        state.expanded = Some(open);
        state
    }
    pub fn option_semantics(
        &self,
        index: usize,
        id: NodeId,
        bounds: LogicalRect,
        stack: &PopupStack,
    ) -> Result<ControlSemantics, ControlError> {
        let option = self.options.get(index).ok_or(ControlError::Stale)?;
        let mut node = SemanticNode::new(id, Role::Custom("list-item".into()));
        node.bounds = bounds;
        node.name = Some(option.label.clone());
        node.disabled = self.disabled || option.disabled;
        node.hidden = !self.is_open(stack);
        node.focused = !node.hidden && self.focused && self.active.as_ref() == Some(&option.key);
        if !node.disabled && !node.hidden {
            node.actions = vec![SemanticAction::Click, SemanticAction::Focus];
        }
        let mut state = ControlSemantics::new(node);
        state.selected = Some(self.selected.as_ref() == Some(&option.key));
        Ok(state)
    }
}
