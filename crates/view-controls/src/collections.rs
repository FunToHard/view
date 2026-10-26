use crate::*;
use std::collections::HashSet;
use view_core::{
    Key, KeyPhase, KeyboardEvent, LogicalPoint, LogicalRect, LogicalSize, NamedKey, ScrollState,
};
#[derive(Clone, Debug, Default)]
pub struct SelectionModel {
    keys: Vec<String>,
    selected: HashSet<String>,
    focus: Option<String>,
    anchor: Option<String>,
}
impl SelectionModel {
    pub fn replace(&mut self, keys: Vec<String>) -> Result<(), ControlError> {
        let unique: HashSet<_> = keys.iter().collect();
        if unique.len() != keys.len() {
            return Err(ControlError::Invalid("duplicate selection key"));
        }
        let old = self
            .focus
            .as_ref()
            .and_then(|k| self.keys.iter().position(|x| x == k))
            .unwrap_or(0);
        self.selected.retain(|k| unique.contains(k));
        if self.focus.as_ref().is_some_and(|k| !unique.contains(k)) {
            self.focus = keys.get(old.min(keys.len().saturating_sub(1))).cloned();
        }
        if self.anchor.as_ref().is_some_and(|k| !unique.contains(k)) {
            self.anchor = self.focus.clone();
        }
        self.keys = keys;
        Ok(())
    }
    pub fn keys(&self) -> &[String] {
        &self.keys
    }
    pub fn selected(&self) -> &HashSet<String> {
        &self.selected
    }
    pub fn focused(&self) -> Option<&str> {
        self.focus.as_deref()
    }
    pub fn select(&mut self, key: &str, extend: bool, toggle: bool) -> Result<(), ControlError> {
        let index = self
            .keys
            .iter()
            .position(|k| k == key)
            .ok_or(ControlError::Stale)?;
        if extend {
            if self.anchor.is_none() {
                self.anchor = Some(self.focus.clone().unwrap_or_else(|| key.into()));
            }
            let anchor = self
                .anchor
                .as_ref()
                .and_then(|k| self.keys.iter().position(|x| x == k))
                .unwrap_or(index);
            if !toggle {
                self.selected.clear();
            }
            self.selected.extend(
                self.keys[anchor.min(index)..=anchor.max(index)]
                    .iter()
                    .cloned(),
            );
        } else if toggle {
            if !self.selected.remove(key) {
                self.selected.insert(key.into());
            }
            self.anchor = Some(key.into());
        } else {
            self.selected.clear();
            self.selected.insert(key.into());
            self.anchor = Some(key.into());
        }
        self.focus = Some(key.into());
        Ok(())
    }
    pub fn key(&mut self, e: &KeyboardEvent, page: usize) -> Result<bool, ControlError> {
        if e.phase == KeyPhase::Up || self.keys.is_empty() || e.modifiers.alt || e.modifiers.meta {
            return Ok(false);
        }
        let current = self
            .focus
            .as_ref()
            .and_then(|k| self.keys.iter().position(|x| x == k))
            .unwrap_or(0);
        let last = self.keys.len() - 1;
        let index = match e.key {
            Key::Named(NamedKey::ArrowDown) => {
                if self.focus.is_none() {
                    0
                } else {
                    (current + 1).min(last)
                }
            }
            Key::Named(NamedKey::ArrowUp) => current.saturating_sub(1),
            Key::Named(NamedKey::Home) => 0,
            Key::Named(NamedKey::End) => last,
            Key::Named(NamedKey::PageDown) => current.saturating_add(page.max(1)).min(last),
            Key::Named(NamedKey::PageUp) => current.saturating_sub(page.max(1)),
            Key::Named(NamedKey::Space) => {
                let key = self.keys[current].clone();
                self.select(&key, false, e.modifiers.ctrl)?;
                return Ok(true);
            }
            _ => return Ok(false),
        };
        let key = self.keys[index].clone();
        if e.modifiers.ctrl && !e.modifiers.shift {
            self.focus = Some(key);
        } else {
            self.select(&key, e.modifiers.shift, e.modifiers.ctrl)?;
        }
        Ok(true)
    }
}
pub struct VirtualList {
    pub selection: SelectionModel,
    pub scroll: ScrollState,
    row_height: f32,
    max_realized: usize,
}
impl VirtualList {
    pub fn new(
        keys: Vec<String>,
        row_height: f32,
        viewport: LogicalSize,
        max_realized: usize,
    ) -> Result<Self, ControlError> {
        if !row_height.is_finite() || row_height <= 0.0 || !viewport.is_valid() || max_realized == 0
        {
            return Err(ControlError::Invalid("virtual list geometry"));
        }
        let mut selection = SelectionModel::default();
        selection.replace(keys)?;
        let height = selection.keys.len() as f32 * row_height;
        if !height.is_finite() {
            return Err(ControlError::Invalid("virtual content extent"));
        }
        Ok(Self {
            selection,
            scroll: ScrollState::new(LogicalSize::new(viewport.width, height), viewport),
            row_height,
            max_realized,
        })
    }
    pub fn replace(&mut self, keys: Vec<String>) -> Result<(), ControlError> {
        if !(keys.len() as f32 * self.row_height).is_finite() {
            return Err(ControlError::Invalid("virtual content extent"));
        }
        self.selection.replace(keys)?;
        self.scroll.content_size.height = self.selection.keys.len() as f32 * self.row_height;
        self.scroll.scroll_to(self.scroll.offset);
        self.reveal_focus();
        Ok(())
    }
    pub fn realized(&self) -> Result<std::ops::Range<usize>, ControlError> {
        let first = (self.scroll.offset.y / self.row_height).floor() as usize;
        let end = ((self.scroll.offset.y + self.scroll.viewport_size.height) / self.row_height)
            .ceil() as usize;
        let range = first.min(self.selection.keys.len())..end.min(self.selection.keys.len());
        if range.len() > self.max_realized {
            Err(ControlError::Invalid("realization budget"))
        } else {
            Ok(range)
        }
    }
    pub fn reveal_focus(&mut self) {
        if let Some(i) = self
            .selection
            .focused()
            .and_then(|k| self.selection.keys.iter().position(|x| x == k))
        {
            self.scroll.reveal_rect(LogicalRect::new(
                LogicalPoint::new(0.0, i as f32 * self.row_height),
                LogicalSize::new(self.scroll.viewport_size.width, self.row_height),
            ));
        }
    }
    pub fn key(&mut self, e: &KeyboardEvent) -> Result<bool, ControlError> {
        let changed = self.selection.key(
            e,
            (self.scroll.viewport_size.height / self.row_height) as usize,
        )?;
        self.reveal_focus();
        Ok(changed)
    }
    pub fn pointer_row(&mut self, y: f32, extend: bool, toggle: bool) -> Result<(), ControlError> {
        if !y.is_finite() || y < 0.0 || y >= self.scroll.viewport_size.height {
            return Err(ControlError::Invalid("list pointer"));
        }
        let index = ((y + self.scroll.offset.y) / self.row_height) as usize;
        let key = self
            .selection
            .keys
            .get(index)
            .cloned()
            .ok_or(ControlError::Stale)?;
        self.selection.select(&key, extend, toggle)
    }
    pub fn row_rect(&self, index: usize) -> LogicalRect {
        LogicalRect::new(
            LogicalPoint::new(0.0, index as f32 * self.row_height - self.scroll.offset.y),
            LogicalSize::new(self.scroll.viewport_size.width, self.row_height),
        )
    }
}
#[derive(Clone, Debug)]
pub struct TreeItem {
    pub key: String,
    pub parent: Option<String>,
    pub label: String,
}
pub struct Tree {
    items: Vec<TreeItem>,
    expanded: HashSet<String>,
    pub list: VirtualList,
}
impl Tree {
    pub fn new(items: Vec<TreeItem>, viewport: LogicalSize) -> Result<Self, ControlError> {
        let mut keys = HashSet::new();
        for item in &items {
            if !keys.insert(item.key.clone())
                || item
                    .parent
                    .as_ref()
                    .is_some_and(|p| !keys.contains(p) || p == &item.key)
            {
                return Err(ControlError::Invalid(
                    "tree requires unique keys with parents before children",
                ));
            }
        }
        let list = VirtualList::new(vec![], 24.0, viewport, 512)?;
        let mut tree = Self {
            items,
            expanded: HashSet::new(),
            list,
        };
        tree.refresh()?;
        Ok(tree)
    }
    fn refresh(&mut self) -> Result<(), ControlError> {
        let mut visible = HashSet::new();
        let mut keys = vec![];
        for item in &self.items {
            if item
                .parent
                .as_ref()
                .is_none_or(|p| visible.contains(p) && self.expanded.contains(p))
            {
                visible.insert(item.key.clone());
                keys.push(item.key.clone());
            }
        }
        self.list.replace(keys)
    }
    pub fn set_expanded(&mut self, key: &str, expanded: bool) -> Result<(), ControlError> {
        if !self.items.iter().any(|i| i.key == key) {
            return Err(ControlError::Stale);
        }
        if expanded {
            self.expanded.insert(key.into());
        } else {
            if let Some(focused) = self.list.selection.focused() {
                let mut cursor = Some(focused);
                while let Some(current) = cursor {
                    if current == key {
                        self.list.selection.select(key, false, false)?;
                        break;
                    }
                    cursor = self
                        .items
                        .iter()
                        .find(|i| i.key == current)
                        .and_then(|i| i.parent.as_deref());
                }
            }
            self.expanded.remove(key);
        }
        self.refresh()
    }
    pub fn expanded(&self, key: &str) -> bool {
        self.expanded.contains(key)
    }
    pub fn items(&self) -> &[TreeItem] {
        &self.items
    }
    pub fn key(&mut self, e: &KeyboardEvent) -> Result<bool, ControlError> {
        if e.phase == KeyPhase::Up {
            return Ok(false);
        }
        if let Some(key) = self.list.selection.focused().map(str::to_owned) {
            match e.key {
                Key::Named(NamedKey::ArrowRight) => {
                    if self.expanded(&key) {
                        if let Some(child) = self
                            .items
                            .iter()
                            .find(|item| item.parent.as_ref() == Some(&key))
                            .map(|item| item.key.clone())
                        {
                            self.list.selection.select(&child, false, false)?;
                            self.list.reveal_focus();
                        }
                    } else if self
                        .items
                        .iter()
                        .any(|item| item.parent.as_ref() == Some(&key))
                    {
                        self.set_expanded(&key, true)?;
                    }
                    return Ok(true);
                }
                Key::Named(NamedKey::ArrowLeft) => {
                    if self.expanded(&key) {
                        self.set_expanded(&key, false)?;
                    } else if let Some(parent) = self
                        .items
                        .iter()
                        .find(|i| i.key == key)
                        .and_then(|i| i.parent.clone())
                    {
                        self.list.selection.select(&parent, false, false)?;
                        self.list.reveal_focus();
                    }
                    return Ok(true);
                }
                _ => {}
            }
        }
        self.list.key(e)
    }
}
pub struct Scrollbar {
    drag: Option<(f32, f32)>,
}
impl Default for Scrollbar {
    fn default() -> Self {
        Self::new()
    }
}
impl Scrollbar {
    pub fn new() -> Self {
        Self { drag: None }
    }
    pub fn thumb(scroll: &ScrollState, track: LogicalRect) -> LogicalRect {
        let fraction = if scroll.content_size.height <= 0.0 {
            1.0
        } else {
            (scroll.viewport_size.height / scroll.content_size.height).clamp(0.0, 1.0)
        };
        let height = (track.size.height * fraction)
            .max(12.0)
            .min(track.size.height);
        let max = scroll.max_offset().y;
        let position = if max > 0.0 {
            scroll.offset.y / max
        } else {
            0.0
        };
        LogicalRect::new(
            track.origin + LogicalPoint::new(0.0, position * (track.size.height - height)),
            LogicalSize::new(track.size.width, height),
        )
    }
    pub fn pointer(
        &mut self,
        e: view_core::PointerEvent,
        scroll: &mut ScrollState,
        track: LogicalRect,
    ) -> Result<bool, ControlError> {
        if !track.is_valid() || track.size.height <= 0.0 || !e.position.is_valid() {
            return Err(ControlError::Invalid("scrollbar geometry"));
        }
        let old = scroll.offset;
        let thumb = Self::thumb(scroll, track);
        match e.phase {
            view_core::PointerPhase::Down
                if e.button == Some(view_core::PointerButton::Primary)
                    && track.contains(e.position) =>
            {
                if thumb.contains(e.position) {
                    self.drag = Some((e.position.y, scroll.offset.y));
                } else {
                    scroll.scroll_by(LogicalPoint::new(
                        0.0,
                        if e.position.y < thumb.top() {
                            -scroll.viewport_size.height
                        } else {
                            scroll.viewport_size.height
                        },
                    ));
                }
            }
            view_core::PointerPhase::Move => {
                if let Some((start, offset)) = self.drag {
                    let travel = track.size.height - thumb.size.height;
                    if travel > 0.0 {
                        scroll.scroll_to(LogicalPoint::new(
                            scroll.offset.x,
                            offset + (e.position.y - start) / travel * scroll.max_offset().y,
                        ));
                    }
                }
            }
            view_core::PointerPhase::Cancel => self.drag = None,
            view_core::PointerPhase::Up if e.button == Some(view_core::PointerButton::Primary) => {
                self.drag = None
            }
            _ => {}
        }
        Ok(old != scroll.offset)
    }
}
pub type ScrollView = ScrollState;
