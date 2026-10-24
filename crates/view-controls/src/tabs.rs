//! Horizontal, automatically activated tabs. The host owns panels and their state.
use crate::{ControlError, ControlSemantics, RadioGroup, RadioOption};
use view_core::{Key, KeyPhase, KeyboardEvent, LogicalRect, NamedKey, NodeId, Role};

/// Stable keyed tab metadata; disabled tabs remain discoverable but cannot activate.
pub type TabOption = RadioOption;

/// One host focus stop. Left/Right wrap across enabled tabs; Home/End select the
/// endpoints. Tab and vertical arrows are left to the host. No docking or teardown
/// is implicit: applications decide how to retain and hide associated panels.
pub struct TabList {
    choices: RadioGroup,
    focused: bool,
}
impl TabList {
    pub fn new(options: Vec<TabOption>, selected: Option<String>) -> Result<Self, ControlError> {
        Ok(Self {
            choices: RadioGroup::new(options, selected, true)?,
            focused: false,
        })
    }
    pub fn options(&self) -> &[TabOption] {
        self.choices.options()
    }
    pub fn selected(&self) -> &str {
        self.choices.selected().expect("required tab selection")
    }
    pub fn select(&mut self, key: &str) -> Result<(), ControlError> {
        self.choices.select(key)
    }
    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
        self.choices.set_focused(focused);
    }
    pub fn focused(&self) -> bool {
        self.focused
    }
    pub fn key(&mut self, event: &KeyboardEvent) -> Result<bool, ControlError> {
        if !self.focused
            || event.phase == KeyPhase::Up
            || event.modifiers.ctrl
            || event.modifiers.alt
            || event.modifiers.meta
        {
            return Ok(false);
        }
        match event.key {
            Key::Named(NamedKey::ArrowLeft | NamedKey::ArrowRight) => self.choices.key(event),
            Key::Named(NamedKey::Home | NamedKey::End) => {
                let mut enabled = self.options().iter().filter(|option| !option.disabled);
                let option = if event.key == Key::Named(NamedKey::Home) {
                    enabled.next()
                } else {
                    enabled.next_back()
                };
                let key = option.ok_or(ControlError::Disabled)?.key.clone();
                self.select(&key)?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }
    pub fn semantics(
        &self,
        index: usize,
        id: NodeId,
        bounds: LogicalRect,
    ) -> Result<ControlSemantics, ControlError> {
        let mut state = self.choices.semantics(index, id, bounds)?;
        state.node.role = Role::Custom("tab".into());
        state.selected = state.node.checked.take();
        Ok(state)
    }
}
