//! Committed control semantics to AccessKit. The application executes returned
//! actions on its UI thread, then publishes a new coherent revision.
use crate::{ControlError, ControlSemantics};
use accesskit::{
    Action, ActionData, ActionRequest, Node, NodeId as AccessibleId, Role, TreeId, TreeInfo,
    TreeUpdate,
};
use std::collections::{HashMap, HashSet};
use view_core::{NodeId, Revision, ScaleFactor, SemanticAction};
use view_text::{Selection, TextIndex, grapheme_boundaries};

/// Hierarchy comes from the same committed runtime tree as the control state.
#[derive(Clone)]
pub struct Entry {
    pub parent: Option<NodeId>,
    pub state: ControlSemantics,
}
#[derive(Clone, Debug, PartialEq)]
pub enum ControlAction {
    Click,
    Focus,
    SetValue(String),
    SetNumericValue(f64),
    SetSelection(Selection),
    Expand,
    Collapse,
}
#[derive(Clone, Debug, PartialEq)]
pub struct RoutedAction {
    pub owner: NodeId,
    pub action: ControlAction,
}

#[derive(Clone, Default)]
pub struct SemanticBridge {
    serial: u64,
    ids: HashMap<NodeId, AccessibleId>,
    runs: HashMap<NodeId, AccessibleId>,
    current: HashMap<NodeId, ControlSemantics>,
    revision: Option<Revision>,
}
impl SemanticBridge {
    pub fn revision(&self) -> Option<Revision> {
        self.revision
    }
    pub fn id(&self, owner: NodeId) -> Option<AccessibleId> {
        self.ids.get(&owner).copied()
    }
    fn allocate(&mut self) -> Result<AccessibleId, ControlError> {
        self.serial = self.serial.checked_add(1).ok_or(ControlError::Stale)?;
        Ok(AccessibleId(self.serial))
    }
    /// Full updates are deliberate: removing a child is expressed by replacing
    /// its parent's child list. IDs are never reused, even after runtime reuse.
    pub fn commit(
        &mut self,
        revision: Revision,
        entries: &[Entry],
        scale: ScaleFactor,
    ) -> Result<TreeUpdate, ControlError> {
        let mut next = self.clone();
        let update = next.build(revision, entries, scale)?;
        *self = next;
        Ok(update)
    }
    fn build(
        &mut self,
        revision: Revision,
        entries: &[Entry],
        scale: ScaleFactor,
    ) -> Result<TreeUpdate, ControlError> {
        if self.revision.is_some_and(|r| revision <= r) {
            return Err(ControlError::Stale);
        }
        let mut seen = HashSet::new();
        let mut root = None;
        for entry in entries {
            if !entry.state.node.bounds.is_valid() || !seen.insert(entry.state.node.id) {
                return Err(ControlError::Invalid("semantic identity or bounds"));
            }
            if let Some(parent) = entry.parent {
                if parent == entry.state.node.id || !seen.contains(&parent) {
                    return Err(ControlError::Invalid(
                        "semantic parents must precede children",
                    ));
                }
            } else if root.replace(entry.state.node.id).is_some() {
                return Err(ControlError::Invalid("multiple semantic roots"));
            }
            if !self.ids.contains_key(&entry.state.node.id) {
                let id = self.allocate()?;
                self.ids.insert(entry.state.node.id, id);
            }
        }
        let root = root.ok_or(ControlError::Invalid("missing semantic root"))?;
        self.ids.retain(|owner, _| seen.contains(owner));
        self.runs.retain(|owner, _| seen.contains(owner));
        self.current.clear();
        let mut nodes = Vec::new();
        let mut focus = self.ids[&root];
        let mut has_focus = false;
        for entry in entries {
            let mut inherited = entry.state.clone();
            if let Some(parent) = entry.parent {
                let parent = &self.current[&parent];
                inherited.node.hidden |= parent.node.hidden;
                inherited.node.disabled |= parent.node.disabled;
            }
            let s = &inherited;
            let source = &s.node;
            let id = self.ids[&source.id];
            let role = match &source.role {
                view_core::Role::Window => Role::Window,
                view_core::Role::Dialog => Role::Dialog,
                view_core::Role::Button => Role::Button,
                view_core::Role::Checkbox => Role::CheckBox,
                view_core::Role::Radio => Role::RadioButton,
                view_core::Role::TextInput if s.multiline => Role::MultilineTextInput,
                view_core::Role::TextInput => Role::TextInput,
                view_core::Role::StaticText => Role::Label,
                view_core::Role::ScrollArea => Role::ScrollView,
                view_core::Role::Slider => Role::Slider,
                view_core::Role::Custom(name) if name == "image" => Role::Image,
                view_core::Role::Custom(name) if name == "list" => Role::ListBox,
                view_core::Role::Custom(name) if name == "list-item" => Role::ListBoxOption,
                view_core::Role::Custom(name) if name == "tree-item" => Role::TreeItem,
                _ => Role::GenericContainer,
            };
            let mut node = Node::new(role);
            if let Some(name) = &source.name {
                node.set_label(name.clone());
            }
            if let Some(value) = &source.value {
                node.set_value(value.clone());
            }
            let b = source.bounds;
            let f = scale.get() as f64;
            node.set_bounds(accesskit::Rect::new(
                b.left() as f64 * f,
                b.top() as f64 * f,
                b.right() as f64 * f,
                b.bottom() as f64 * f,
            ));
            if source.disabled {
                node.set_disabled();
            }
            if source.hidden {
                node.set_hidden();
            }
            if s.read_only {
                node.set_read_only();
            }
            if s.invalid {
                node.set_invalid(accesskit::Invalid::True);
            }
            if let Some(checked) = source.checked {
                node.set_toggled(if s.mixed {
                    accesskit::Toggled::Mixed
                } else if checked {
                    accesskit::Toggled::True
                } else {
                    accesskit::Toggled::False
                });
            }
            if let Some(expanded) = s.expanded {
                node.set_expanded(expanded);
            }
            if let Some(selected) = s.selected {
                node.set_selected(selected);
            }
            if let Some((min, max, value, step)) = s.range {
                if [min, max, value, step].iter().any(|v| !v.is_finite())
                    || min > max
                    || !(min..=max).contains(&value)
                    || step <= 0.0
                {
                    return Err(ControlError::Invalid("semantic range"));
                }
                node.set_min_numeric_value(min);
                node.set_max_numeric_value(max);
                node.set_numeric_value(value);
                node.set_numeric_value_step(step);
            }
            for action in &source.actions {
                if !source.disabled && !source.hidden {
                    match action {
                        SemanticAction::Click => node.add_action(Action::Click),
                        SemanticAction::Focus => node.add_action(Action::Focus),
                        SemanticAction::SetValue if !s.read_only => {
                            node.add_action(Action::SetValue)
                        }
                        SemanticAction::Expand => node.add_action(Action::Expand),
                        SemanticAction::Collapse => node.add_action(Action::Collapse),
                        _ => {}
                    }
                }
            }
            let mut children: Vec<_> = entries
                .iter()
                .filter(|e| e.parent == Some(source.id))
                .map(|e| self.ids[&e.state.node.id])
                .collect();
            if let (Some(selection), Some(value)) = (s.selection, source.value.as_ref()) {
                let boundaries = grapheme_boundaries(value);
                let anchor = boundaries
                    .binary_search(&selection.anchor.0)
                    .map_err(|_| ControlError::Invalid("semantic selection"))?;
                let caret = boundaries
                    .binary_search(&selection.focus.0)
                    .map_err(|_| ControlError::Invalid("semantic selection"))?;
                // AccessKit stores character lengths in u8. Preserve grapheme
                // ownership: unusually long clusters expose value but no text range.
                if let Ok(lengths) = boundaries
                    .windows(2)
                    .map(|w| u8::try_from(w[1] - w[0]))
                    .collect::<Result<Vec<_>, _>>()
                {
                    let run_id = if let Some(id) = self.runs.get(&source.id) {
                        *id
                    } else {
                        let id = self.allocate()?;
                        self.runs.insert(source.id, id);
                        id
                    };
                    let mut run = Node::new(Role::TextRun);
                    run.set_value(value.clone());
                    run.set_character_lengths(lengths);
                    children.push(run_id);
                    nodes.push((run_id, run));
                    node.set_text_selection(accesskit::TextSelection {
                        anchor: accesskit::TextPosition {
                            node: run_id,
                            character_index: anchor,
                        },
                        focus: accesskit::TextPosition {
                            node: run_id,
                            character_index: caret,
                        },
                    });
                    if !source.disabled && !source.hidden {
                        node.add_action(Action::SetTextSelection);
                    }
                } else {
                    self.runs.remove(&source.id);
                }
            } else {
                self.runs.remove(&source.id);
            }
            node.set_children(children);
            if source.focused && !source.disabled && !source.hidden {
                if has_focus {
                    return Err(ControlError::Invalid("multiple semantic focus"));
                }
                has_focus = true;
                focus = id;
            }
            nodes.push((id, node));
            self.current.insert(source.id, s.clone());
        }
        self.revision = Some(revision);
        Ok(TreeUpdate {
            nodes,
            tree: Some(TreeInfo::new(self.ids[&root])),
            tree_id: TreeId::ROOT,
            focus,
        })
    }
    /// Reject requests queued against earlier values/ownership, hidden/disabled
    /// controls, unsupported actions and malformed text-range payloads.
    pub fn route(
        &self,
        revision: Revision,
        request: &ActionRequest,
    ) -> Result<RoutedAction, ControlError> {
        if self.revision != Some(revision) || request.target_tree != TreeId::ROOT {
            return Err(ControlError::Stale);
        }
        let owner = *self
            .ids
            .iter()
            .find(|(_, id)| **id == request.target_node)
            .map(|(owner, _)| owner)
            .ok_or(ControlError::Stale)?;
        let s = &self.current[&owner];
        if s.node.disabled || s.node.hidden {
            return Err(ControlError::Disabled);
        }
        let supported = |action| s.node.actions.contains(&action);
        let action = match (&request.action, &request.data) {
            (Action::Click, None) if supported(SemanticAction::Click) => ControlAction::Click,
            (Action::Focus, None) if supported(SemanticAction::Focus) => ControlAction::Focus,
            (Action::Expand, None) if supported(SemanticAction::Expand) => ControlAction::Expand,
            (Action::Collapse, None) if supported(SemanticAction::Collapse) => {
                ControlAction::Collapse
            }
            (Action::SetValue, Some(ActionData::Value(value)))
                if supported(SemanticAction::SetValue) && !s.read_only =>
            {
                ControlAction::SetValue(value.to_string())
            }
            (Action::SetValue, Some(ActionData::NumericValue(value)))
                if supported(SemanticAction::SetValue)
                    && !s.read_only
                    && value.is_finite()
                    && s.range
                        .is_some_and(|(min, max, _, _)| (min..=max).contains(value)) =>
            {
                ControlAction::SetNumericValue(*value)
            }
            (Action::SetTextSelection, Some(ActionData::SetTextSelection(selection)))
                if s.selection.is_some() =>
            {
                let run = self.runs.get(&owner).ok_or(ControlError::Stale)?;
                if selection.anchor.node != *run || selection.focus.node != *run {
                    return Err(ControlError::Stale);
                }
                let boundaries =
                    grapheme_boundaries(s.node.value.as_deref().ok_or(ControlError::Stale)?);
                ControlAction::SetSelection(Selection {
                    anchor: TextIndex(
                        *boundaries
                            .get(selection.anchor.character_index)
                            .ok_or(ControlError::Stale)?,
                    ),
                    focus: TextIndex(
                        *boundaries
                            .get(selection.focus.character_index)
                            .ok_or(ControlError::Stale)?,
                    ),
                })
            }
            _ => return Err(ControlError::Invalid("unsupported accessibility action")),
        };
        Ok(RoutedAction { owner, action })
    }
}
