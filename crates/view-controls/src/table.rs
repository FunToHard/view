//! Read-only bounded-table/1: keyed row selection, cell navigation and two-axis
//! virtualization. Sorting is an application-owned keyed row replacement.
use crate::{ControlError, ControlSemantics, VirtualList};
use view_core::{
    Key, KeyPhase, KeyboardEvent, LogicalPoint, LogicalRect, LogicalSize, NamedKey, NodeId, Role,
    ScrollState, SemanticAction, SemanticNode,
};

#[derive(Clone, Debug)]
pub struct TableColumn {
    pub key: String,
    pub label: String,
    pub width: f32,
}
#[derive(Clone, Debug)]
pub struct TableRow {
    pub key: String,
    pub cells: Vec<String>,
}
#[derive(Clone, Copy, Debug)]
pub struct TableLimits {
    pub max_rows: usize,
    pub max_columns: usize,
    pub max_realized_cells: usize,
}
impl Default for TableLimits {
    fn default() -> Self {
        Self {
            max_rows: 100_000,
            max_columns: 64,
            max_realized_cells: 512,
        }
    }
}
pub struct Table {
    columns: Vec<TableColumn>,
    rows: Vec<TableRow>,
    list: VirtualList,
    limits: TableLimits,
    row_height: f32,
    active_column: usize,
}
impl Table {
    pub const PROFILE: &str = "bounded-table/1";
    pub fn new(
        columns: Vec<TableColumn>,
        rows: Vec<TableRow>,
        row_height: f32,
        viewport: LogicalSize,
        limits: TableLimits,
    ) -> Result<Self, ControlError> {
        let mut keys = std::collections::HashSet::new();
        if columns.is_empty()
            || columns.len() > limits.max_columns
            || limits.max_rows == 0
            || limits.max_realized_cells == 0
            || columns.iter().any(|column| {
                !keys.insert(column.key.as_str())
                    || !column.width.is_finite()
                    || column.width <= 0.0
            })
            || !row_height.is_finite()
            || row_height <= 0.0
            || !viewport.is_valid()
            || viewport.height < row_height
        {
            return Err(ControlError::Invalid("table columns, geometry or limits"));
        }
        let width: f32 = columns.iter().map(|column| column.width).sum();
        if !width.is_finite() {
            return Err(ControlError::Invalid("table width"));
        }
        let list = VirtualList::new(
            vec![],
            row_height,
            LogicalSize::new(viewport.width, viewport.height - row_height),
            limits.max_realized_cells,
        )?;
        let mut table = Self {
            columns,
            rows: vec![],
            list,
            limits,
            row_height,
            active_column: 0,
        };
        table.list.scroll.content_size.width = width;
        table.replace(rows)?;
        Ok(table)
    }
    pub fn columns(&self) -> &[TableColumn] {
        &self.columns
    }
    pub fn rows(&self) -> &[TableRow] {
        &self.rows
    }
    pub fn selection(&self) -> &crate::SelectionModel {
        &self.list.selection
    }
    pub fn active_column(&self) -> usize {
        self.active_column
    }
    pub fn scroll(&self) -> &ScrollState {
        &self.list.scroll
    }
    pub fn row_height(&self) -> f32 {
        self.row_height
    }
    /// Rectangular data and unique stable keys are validated before state changes.
    pub fn replace(&mut self, rows: Vec<TableRow>) -> Result<(), ControlError> {
        let mut keys = std::collections::HashSet::new();
        if rows.len() > self.limits.max_rows
            || rows
                .iter()
                .any(|row| row.cells.len() != self.columns.len() || !keys.insert(row.key.as_str()))
        {
            return Err(ControlError::Invalid("table row shape, identity or budget"));
        }
        self.list
            .replace(rows.iter().map(|row| row.key.clone()).collect())?;
        self.rows = rows;
        Ok(())
    }
    pub fn select(&mut self, key: &str, extend: bool, toggle: bool) -> Result<(), ControlError> {
        self.list.selection.select(key, extend, toggle)?;
        self.reveal_focus();
        Ok(())
    }
    pub fn focus_cell(&mut self, key: &str, column: usize) -> Result<(), ControlError> {
        if column >= self.columns.len() {
            return Err(ControlError::Stale);
        }
        self.list.selection.select(key, false, false)?;
        self.active_column = column;
        self.reveal_focus();
        Ok(())
    }
    pub fn scroll_by(&mut self, delta: LogicalPoint) -> bool {
        if !delta.is_valid() {
            return false;
        }
        let old = self.list.scroll.offset;
        self.list.scroll.scroll_by(delta);
        old != self.list.scroll.offset
    }
    pub fn resize(&mut self, viewport: LogicalSize) -> Result<(), ControlError> {
        if !viewport.is_valid() || viewport.height < self.row_height {
            return Err(ControlError::Invalid("table viewport"));
        }
        self.list.scroll.viewport_size =
            LogicalSize::new(viewport.width, viewport.height - self.row_height);
        self.list.scroll.scroll_to(self.list.scroll.offset);
        self.reveal_focus();
        Ok(())
    }
    pub fn realized_rows(&self) -> Result<std::ops::Range<usize>, ControlError> {
        self.list.realized()
    }
    pub fn realized_columns(&self) -> std::ops::Range<usize> {
        let left = self.list.scroll.offset.x;
        let right = left + self.list.scroll.viewport_size.width;
        let mut edge = 0.0;
        let mut first = self.columns.len();
        let mut end = first;
        for (index, column) in self.columns.iter().enumerate() {
            if edge < right && edge + column.width > left {
                first = first.min(index);
                end = index + 1;
            }
            edge += column.width;
        }
        first..end
    }
    /// Includes visible headers in the declared cell budget.
    pub fn realized_cells(&self) -> Result<Vec<(Option<usize>, usize)>, ControlError> {
        let rows = self.realized_rows()?;
        let columns = self.realized_columns();
        let count = (rows.len() + 1)
            .checked_mul(columns.len())
            .ok_or(ControlError::Invalid("table realization budget"))?;
        if count > self.limits.max_realized_cells {
            return Err(ControlError::Invalid("table realization budget"));
        }
        let mut cells = Vec::with_capacity(count);
        cells.extend(columns.clone().map(|column| (None, column)));
        for row in rows {
            cells.extend(columns.clone().map(|column| (Some(row), column)));
        }
        Ok(cells)
    }
    pub fn cell_rect(
        &self,
        row: Option<usize>,
        column: usize,
    ) -> Result<LogicalRect, ControlError> {
        let definition = self.columns.get(column).ok_or(ControlError::Stale)?;
        if row.is_some_and(|row| row >= self.rows.len()) {
            return Err(ControlError::Stale);
        }
        let x: f32 = self.columns[..column]
            .iter()
            .map(|column| column.width)
            .sum();
        let y = row.map_or(0.0, |row| {
            self.row_height + row as f32 * self.row_height - self.list.scroll.offset.y
        });
        Ok(LogicalRect::new(
            LogicalPoint::new(x - self.list.scroll.offset.x, y),
            LogicalSize::new(definition.width, self.row_height),
        ))
    }
    fn reveal_focus(&mut self) {
        self.list.reveal_focus();
        if let Ok(mut cell) = self.cell_rect(None, self.active_column) {
            cell.origin.x += self.list.scroll.offset.x;
            cell.origin.y = self.list.scroll.offset.y;
            cell.size.height = 0.0;
            self.list.scroll.reveal_rect(cell);
        }
    }
    pub fn key(&mut self, event: &KeyboardEvent) -> Result<bool, ControlError> {
        if event.phase == KeyPhase::Up || event.modifiers.alt || event.modifiers.meta {
            return Ok(false);
        }
        match event.key {
            Key::Named(NamedKey::ArrowLeft) if !event.modifiers.ctrl => {
                self.active_column = self.active_column.saturating_sub(1)
            }
            Key::Named(NamedKey::ArrowRight) if !event.modifiers.ctrl => {
                self.active_column = (self.active_column + 1).min(self.columns.len() - 1)
            }
            _ => {
                let handled = self.list.key(event)?;
                self.reveal_focus();
                return Ok(handled);
            }
        }
        self.reveal_focus();
        Ok(true)
    }
    /// Coordinates are local to the whole table, including its fixed header.
    pub fn pointer(
        &mut self,
        point: LogicalPoint,
        extend: bool,
        toggle: bool,
    ) -> Result<(), ControlError> {
        if !point.is_valid()
            || point.x < 0.0
            || point.x >= self.list.scroll.viewport_size.width
            || point.y < self.row_height
        {
            return Err(ControlError::Invalid("table pointer"));
        }
        let x = point.x + self.list.scroll.offset.x;
        let mut edge = 0.0;
        let column = self
            .columns
            .iter()
            .position(|column| {
                edge += column.width;
                x < edge
            })
            .ok_or(ControlError::Stale)?;
        self.list
            .pointer_row(point.y - self.row_height, extend, toggle)?;
        self.active_column = column;
        self.reveal_focus();
        Ok(())
    }
    pub fn semantics(&self, id: NodeId, bounds: LogicalRect, label: &str) -> ControlSemantics {
        let mut node = SemanticNode::new(id, Role::Custom("table".into()));
        node.bounds = bounds;
        node.name = Some(label.into());
        node.actions = vec![SemanticAction::Focus];
        let mut state = ControlSemantics::new(node);
        state.read_only = true;
        state.table_dimensions = Some((self.rows.len() + 1, self.columns.len()));
        state
    }
    pub fn cell_semantics(
        &self,
        row: Option<usize>,
        column: usize,
        id: NodeId,
        bounds: LogicalRect,
        focused: bool,
    ) -> Result<ControlSemantics, ControlError> {
        let definition = self.columns.get(column).ok_or(ControlError::Stale)?;
        let mut node = SemanticNode::new(
            id,
            Role::Custom(
                if row.is_some() {
                    "cell"
                } else {
                    "column-header"
                }
                .into(),
            ),
        );
        node.bounds = bounds;
        node.name = Some(if let Some(row) = row {
            self.rows.get(row).ok_or(ControlError::Stale)?.cells[column].clone()
        } else {
            definition.label.clone()
        });
        let mut state = ControlSemantics::new(node);
        state.read_only = true;
        state.cell_position = Some((row.map_or(0, |index| index + 1), column));
        if let Some(index) = row {
            let key = &self.rows[index].key;
            state.selected = Some(self.selection().selected().contains(key));
            state.node.focused = focused
                && self.selection().focused() == Some(key.as_str())
                && column == self.active_column;
            state.node.actions = vec![SemanticAction::Click, SemanticAction::Focus];
        }
        Ok(state)
    }
}
