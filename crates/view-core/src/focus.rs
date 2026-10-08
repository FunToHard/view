//! Focus management: tab traversal, shortcuts/command scopes, modal precedence, and restoration.
//!
//! Focus represents the active keyboard/accessibility target. It is explicitly
//! distinguished from application document or 3D scene selection.

use std::collections::HashMap;

use crate::{
    CoreError, NodeId,
    input::{Key, Modifiers},
};

/// Direction for focus navigation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusDirection {
    /// Next focusable node (Tab).
    Forward,
    /// Previous focusable node (Shift+Tab).
    Backward,
}

/// Keyboard shortcut combining a key with modifier flags.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Shortcut {
    /// Key.
    pub key: Key,
    /// Modifiers.
    pub modifiers: Modifiers,
}

impl Shortcut {
    /// Create a shortcut.
    pub fn new(key: Key, modifiers: Modifiers) -> Self {
        Self { key, modifiers }
    }
}

/// Node registration for focus participation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FocusableRegistration {
    /// Node identity.
    pub id: NodeId,
    /// Tab order index (default 0). Nodes with equal tab_index preserve declaration order.
    pub tab_index: i32,
}

/// Focus manager for a window.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FocusManager {
    focused: Option<NodeId>,
    modal_scope: Option<NodeId>,
    history: Vec<NodeId>,
    shortcuts: HashMap<Shortcut, NodeId>,
}

impl FocusManager {
    /// Create an empty focus manager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Currently focused node, if any.
    #[inline]
    pub fn focused(&self) -> Option<NodeId> {
        self.focused
    }

    /// Currently active modal scope root, if any.
    #[inline]
    pub fn modal_scope(&self) -> Option<NodeId> {
        self.modal_scope
    }

    /// Set or clear the active modal scope.
    ///
    /// When a modal scope is set, focus and input are constrained to its descendants.
    /// When dismissed, focus is restored to the node focused prior to opening the modal.
    pub fn set_modal_scope(&mut self, modal: Option<NodeId>) {
        if modal.is_some() && self.modal_scope.is_none() {
            // Push currently focused node into history.
            if let Some(current) = self.focused {
                self.history.push(current);
            }
        } else if modal.is_none() && self.modal_scope.is_some() {
            // Modal closing: restore focus.
            self.restore_focus();
        }
        self.modal_scope = modal;
    }

    /// Explicitly set focus to a node.
    pub fn set_focus(&mut self, node: Option<NodeId>) {
        if let Some(prev) = self.focused
            && Some(prev) != node
        {
            self.history.push(prev);
        }
        self.focused = node;
    }

    /// Restore focus from history.
    pub fn restore_focus(&mut self) -> Option<NodeId> {
        while let Some(prev) = self.history.pop() {
            if Some(prev) != self.focused {
                self.focused = Some(prev);
                return Some(prev);
            }
        }
        None
    }

    /// Handle unmounting of a node. If the node or an ancestor was focused,
    /// clears or restores focus.
    pub fn handle_unmount(&mut self, unmounted: NodeId) {
        self.history.retain(|&id| id != unmounted);
        if self.focused == Some(unmounted) {
            self.focused = None;
            self.restore_focus();
        }
        if self.modal_scope == Some(unmounted) {
            self.modal_scope = None;
            self.restore_focus();
        }
        self.shortcuts.retain(|_, &mut target| target != unmounted);
    }

    /// Register a window-scoped shortcut.
    pub fn register_shortcut(&mut self, shortcut: Shortcut, target: NodeId) {
        self.shortcuts.insert(shortcut, target);
    }

    /// Resolve a keyboard shortcut to a target node.
    pub fn resolve_shortcut(&self, shortcut: &Shortcut) -> Option<NodeId> {
        self.shortcuts.get(shortcut).copied()
    }

    /// Cycle focus within a list of candidate focusable nodes.
    pub fn navigate(
        &mut self,
        direction: FocusDirection,
        mut candidates: Vec<FocusableRegistration>,
    ) -> Option<NodeId> {
        if candidates.is_empty() {
            return None;
        }

        // Sort by tab_index ascending.
        candidates.sort_by_key(|c| c.tab_index);

        let current_index = self
            .focused
            .and_then(|id| candidates.iter().position(|c| c.id == id));

        let next_index = match (current_index, direction) {
            (None, FocusDirection::Forward) => 0,
            (None, FocusDirection::Backward) => candidates.len() - 1,
            (Some(idx), FocusDirection::Forward) => (idx + 1) % candidates.len(),
            (Some(idx), FocusDirection::Backward) => {
                if idx == 0 {
                    candidates.len() - 1
                } else {
                    idx - 1
                }
            }
        };

        let target = candidates[next_index].id;
        self.set_focus(Some(target));
        Some(target)
    }

    /// Validate whether a candidate target is permitted by active modal precedence.
    pub fn is_allowed_by_modal(
        &self,
        target: NodeId,
        is_descendant: impl FnOnce(NodeId, NodeId) -> bool,
    ) -> Result<(), CoreError> {
        if let Some(modal) = self.modal_scope
            && target != modal
            && !is_descendant(modal, target)
        {
            return Err(CoreError::ModalBlocked);
        }
        Ok(())
    }
}
