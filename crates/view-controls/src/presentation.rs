//! Pure geometry/state shared by CPU semantics and optional GPU decoration.
use crate::*;
use view_core::{
    LogicalPoint, LogicalRect, LogicalSize, NodeId, Role, SemanticAction, SemanticNode,
};

/// One clipped visible row/cell. Text remains positioned at `bounds.origin` when
/// partially scrolled; clipping never reflows the label or shifts its baseline.
#[derive(Clone, Debug)]
pub struct ControlRow {
    pub label: String,
    pub bounds: LogicalRect,
    pub clip: LogicalRect,
    pub selected: bool,
    pub focused: bool,
    pub disabled: bool,
}
fn translated(mut rect: LogicalRect, origin: LogicalPoint) -> LogicalRect {
    rect.origin = rect.origin + origin;
    rect
}
impl VirtualList {
    pub fn presentation(
        &self,
        bounds: LogicalRect,
        focused: bool,
    ) -> Result<Vec<ControlRow>, ControlError> {
        if !bounds.is_valid() {
            return Err(ControlError::Invalid("list presentation bounds"));
        }
        self.realized()?
            .map(|index| {
                let key = &self.selection.keys()[index];
                Ok(ControlRow {
                    label: key.clone(),
                    bounds: translated(self.row_rect(index), bounds.origin),
                    clip: bounds,
                    selected: self.selection.selected().contains(key),
                    focused: focused && self.selection.focused() == Some(key.as_str()),
                    disabled: false,
                })
            })
            .collect()
    }
    pub fn row_semantics(
        &self,
        index: usize,
        id: NodeId,
        bounds: LogicalRect,
        focused: bool,
    ) -> Result<ControlSemantics, ControlError> {
        let key = self
            .selection
            .keys()
            .get(index)
            .ok_or(ControlError::Stale)?;
        let mut node = SemanticNode::new(id, Role::Custom("list-item".into()));
        node.bounds = bounds;
        node.name = Some(key.clone());
        node.focused = focused && self.selection.focused() == Some(key.as_str());
        node.actions = vec![SemanticAction::Click, SemanticAction::Focus];
        let mut state = ControlSemantics::new(node);
        state.selected = Some(self.selection.selected().contains(key));
        Ok(state)
    }
}
impl Tree {
    pub fn depth(&self, key: &str) -> Result<usize, ControlError> {
        let mut item = self
            .items()
            .iter()
            .find(|item| item.key == key)
            .ok_or(ControlError::Stale)?;
        let mut depth = 0;
        while let Some(parent) = &item.parent {
            item = self
                .items()
                .iter()
                .find(|item| &item.key == parent)
                .ok_or(ControlError::Stale)?;
            depth += 1;
        }
        Ok(depth)
    }
    pub fn has_children(&self, key: &str) -> bool {
        self.items()
            .iter()
            .any(|item| item.parent.as_deref() == Some(key))
    }
    pub fn presentation(
        &self,
        bounds: LogicalRect,
        focused: bool,
    ) -> Result<Vec<ControlRow>, ControlError> {
        let mut rows = self.list.presentation(bounds, focused)?;
        for row in &mut rows {
            let key = row.label.clone();
            let item = self
                .items()
                .iter()
                .find(|item| item.key == key)
                .ok_or(ControlError::Stale)?;
            let marker = if self.has_children(&key) {
                if self.expanded(&key) { "− " } else { "+ " }
            } else {
                "  "
            };
            row.label = format!("{marker}{}", item.label);
            let indent = (self.depth(&key)? as f32 * 16.0).min(row.bounds.size.width);
            row.bounds.origin.x += indent;
            row.bounds.size.width -= indent;
        }
        Ok(rows)
    }
    pub fn row_semantics(
        &self,
        index: usize,
        id: NodeId,
        bounds: LogicalRect,
        focused: bool,
    ) -> Result<ControlSemantics, ControlError> {
        let mut state = self.list.row_semantics(index, id, bounds, focused)?;
        let key = &self.list.selection.keys()[index];
        state.node.role = Role::Custom("tree-item".into());
        state.node.name = Some(
            self.items()
                .iter()
                .find(|item| &item.key == key)
                .ok_or(ControlError::Stale)?
                .label
                .clone(),
        );
        if self.has_children(key) {
            let expanded = self.expanded(key);
            state.expanded = Some(expanded);
            state.node.actions.push(if expanded {
                SemanticAction::Collapse
            } else {
                SemanticAction::Expand
            });
        }
        Ok(state)
    }
}
impl Select {
    pub fn presentation(
        &self,
        bounds: LogicalRect,
        row_height: f32,
        stack: &PopupStack,
    ) -> Result<Vec<ControlRow>, ControlError> {
        if !bounds.is_valid() || !row_height.is_finite() || row_height <= 0.0 {
            return Err(ControlError::Invalid("select presentation"));
        }
        if !self.is_open(stack) {
            return Ok(vec![]);
        }
        // Select/1 has a bounded viewport; reveal the active option and realize
        // at most its visible rows. Hosts keep option NodeIds keyed independently.
        let capacity = (bounds.size.height / row_height).floor() as usize;
        if capacity == 0 {
            return Ok(vec![]);
        }
        let active = self
            .options()
            .iter()
            .position(|option| Some(option.key.as_str()) == self.active())
            .unwrap_or(0);
        let first = active.saturating_sub(capacity - 1);
        Ok(self
            .options()
            .iter()
            .skip(first)
            .take(capacity)
            .enumerate()
            .map(|(offset, option)| ControlRow {
                label: option.label.clone(),
                bounds: LogicalRect::new(
                    bounds.origin + LogicalPoint::new(0.0, offset as f32 * row_height),
                    LogicalSize::new(bounds.size.width, row_height),
                ),
                clip: bounds,
                selected: self.selected() == Some(option.key.as_str()),
                focused: self.active() == Some(option.key.as_str()),
                disabled: self.disabled() || option.disabled,
            })
            .collect())
    }
}
impl Table {
    pub fn presentation(
        &self,
        bounds: LogicalRect,
        focused: bool,
    ) -> Result<Vec<ControlRow>, ControlError> {
        if !bounds.is_valid() {
            return Err(ControlError::Invalid("table presentation"));
        }
        let body = LogicalRect::new(
            bounds.origin + LogicalPoint::new(0.0, self.row_height()),
            LogicalSize::new(
                bounds.size.width,
                (bounds.size.height - self.row_height()).max(0.0),
            ),
        );
        self.realized_cells()?
            .into_iter()
            .map(|(row, column)| {
                let definition = &self.columns()[column];
                let data = row.map(|row| &self.rows()[row]);
                Ok(ControlRow {
                    label: data
                        .map_or_else(|| definition.label.clone(), |row| row.cells[column].clone()),
                    bounds: translated(self.cell_rect(row, column)?, bounds.origin),
                    clip: if row.is_some() {
                        body
                    } else {
                        LogicalRect::new(
                            bounds.origin,
                            LogicalSize::new(bounds.size.width, self.row_height()),
                        )
                    },
                    selected: data
                        .is_some_and(|row| self.selection().selected().contains(&row.key)),
                    focused: focused
                        && column == self.active_column()
                        && data.is_some_and(|row| {
                            self.selection().focused() == Some(row.key.as_str())
                        }),
                    disabled: false,
                })
            })
            .collect()
    }
}
impl<A: Clone> Menu<A> {
    pub fn presentation(
        &self,
        bounds: LogicalRect,
        row_height: f32,
    ) -> Result<Vec<ControlRow>, ControlError> {
        if !bounds.is_valid() || !row_height.is_finite() || row_height <= 0.0 {
            return Err(ControlError::Invalid("menu presentation"));
        }
        let capacity = (bounds.size.height / row_height).floor() as usize;
        let first = self
            .focused
            .unwrap_or(0)
            .saturating_sub(capacity.saturating_sub(1));
        Ok(self
            .items
            .iter()
            .skip(first)
            .take(capacity)
            .enumerate()
            .map(|(offset, item)| ControlRow {
                label: item.label.clone(),
                bounds: LogicalRect::new(
                    bounds.origin + LogicalPoint::new(0.0, offset as f32 * row_height),
                    LogicalSize::new(bounds.size.width, row_height),
                ),
                clip: bounds,
                selected: false,
                focused: self.focused == Some(first + offset),
                disabled: !item.enabled,
            })
            .collect())
    }
    pub fn item_semantics(
        &self,
        index: usize,
        id: NodeId,
        bounds: LogicalRect,
    ) -> Result<ControlSemantics, ControlError> {
        let item = self.items.get(index).ok_or(ControlError::Stale)?;
        let mut node = SemanticNode::new(id, Role::Custom("menu-item".into()));
        node.bounds = bounds;
        node.name = Some(item.label.clone());
        node.disabled = !item.enabled;
        node.focused = self.focused == Some(index) && item.enabled;
        if item.enabled {
            node.actions = vec![SemanticAction::Click, SemanticAction::Focus];
        }
        Ok(ControlSemantics::new(node))
    }
}

/// A modal dialog's command policy; the runtime still owns focus containment,
/// child input and lifecycle, through the same PopupStack as menus and selects.
pub struct Dialog<A> {
    pub title: String,
    pub body: String,
    pub actions: Vec<MenuItem<A>>,
    pub default: Option<usize>,
    pub cancel: Option<usize>,
}
impl<A: Clone> Dialog<A> {
    pub fn activate(&self, index: usize) -> Result<A, ControlError> {
        let action = self.actions.get(index).ok_or(ControlError::Stale)?;
        if !action.enabled {
            return Err(ControlError::Disabled);
        }
        Ok(action.action.clone())
    }
    /// Call after the focused child. A multiline editor may consume Enter.
    pub fn key(&self, event: &view_core::KeyboardEvent, child_handled: bool) -> MenuResponse<A> {
        use view_core::{Key, KeyPhase, NamedKey};
        if child_handled
            || event.phase != KeyPhase::Down
            || event.modifiers.ctrl
            || event.modifiers.alt
            || event.modifiers.meta
        {
            return MenuResponse::Ignored;
        }
        match event.key {
            Key::Named(NamedKey::Enter) => self
                .default
                .and_then(|index| self.activate(index).ok())
                .map_or(MenuResponse::Ignored, MenuResponse::Activate),
            Key::Named(NamedKey::Escape) => self
                .cancel
                .and_then(|index| self.activate(index).ok())
                .map_or(MenuResponse::Dismiss, MenuResponse::Activate),
            _ => MenuResponse::Ignored,
        }
    }
    pub fn open(&self, stack: &mut PopupStack, popup: Popup) -> Result<(), ControlError> {
        if !popup.modal {
            return Err(ControlError::Invalid("dialog requires modal popup"));
        }
        if self
            .default
            .is_some_and(|index| index >= self.actions.len())
            || self.cancel.is_some_and(|index| index >= self.actions.len())
        {
            return Err(ControlError::Invalid("dialog command index"));
        }
        stack.open(popup)
    }
    pub fn semantics(&self, id: NodeId, bounds: LogicalRect) -> ControlSemantics {
        let mut node = SemanticNode::new(id, Role::Dialog);
        node.bounds = bounds;
        node.name = Some(self.title.clone());
        node.value = Some(self.body.clone());
        ControlSemantics::new(node)
    }
}
