use crate::*;
use view_core::{LogicalPoint, LogicalRect, LogicalSize, NodeId};
#[derive(Clone, Copy, Debug)]
pub struct Popup {
    pub owner: NodeId,
    pub content: NodeId,
    pub return_focus: Option<NodeId>,
    pub bounds: LogicalRect,
    pub modal: bool,
}
#[derive(Default)]
pub struct PopupStack {
    stack: Vec<Popup>,
}
impl PopupStack {
    pub fn placement(
        anchor: LogicalRect,
        size: LogicalSize,
        work_area: LogicalRect,
    ) -> Result<LogicalRect, ControlError> {
        if !anchor.is_valid() || !size.is_valid() || !work_area.is_valid() {
            return Err(ControlError::Invalid("popup placement"));
        }
        let size = LogicalSize::new(
            size.width.min(work_area.size.width),
            size.height.min(work_area.size.height),
        );
        let x = anchor
            .left()
            .clamp(work_area.left(), work_area.right() - size.width);
        let y = if anchor.bottom() + size.height <= work_area.bottom() {
            anchor.bottom()
        } else {
            anchor.top() - size.height
        }
        .clamp(work_area.top(), work_area.bottom() - size.height);
        Ok(LogicalRect::new(LogicalPoint::new(x, y), size))
    }
    pub fn open(&mut self, popup: Popup) -> Result<(), ControlError> {
        if !popup.bounds.is_valid() || self.stack.iter().any(|p| p.content == popup.content) {
            return Err(ControlError::Invalid("popup"));
        }
        self.stack.push(popup);
        Ok(())
    }
    pub fn top(&self) -> Option<&Popup> {
        self.stack.last()
    }
    pub fn escape(&mut self, live: impl Fn(NodeId) -> bool) -> Option<NodeId> {
        self.stack.pop().and_then(|p| {
            p.return_focus
                .filter(|id| live(*id))
                .or_else(|| live(p.owner).then_some(p.owner))
        })
    }
    pub fn outside(
        &mut self,
        point: LogicalPoint,
        live: impl Fn(NodeId) -> bool,
    ) -> Option<NodeId> {
        if self
            .top()
            .is_some_and(|p| !p.modal && !p.bounds.contains(point))
        {
            self.escape(live)
        } else {
            None
        }
    }
    pub fn remove_owner(&mut self, owner: NodeId) {
        if let Some(i) = self
            .stack
            .iter()
            .position(|p| p.owner == owner || p.content == owner)
        {
            self.stack.truncate(i);
        }
    }
    pub fn focus_allowed(
        &self,
        target: NodeId,
        is_descendant: impl Fn(NodeId, NodeId) -> bool,
    ) -> bool {
        self.stack
            .iter()
            .rev()
            .find(|p| p.modal)
            .is_none_or(|p| p.content == target || is_descendant(p.content, target))
    }
}
pub struct SplitPane {
    pub first: f32,
    pub minimum_first: f32,
    pub minimum_second: f32,
    drag_origin: Option<(f32, f32)>,
}
impl SplitPane {
    pub fn new(first: f32, minimum_first: f32, minimum_second: f32) -> Result<Self, ControlError> {
        if [first, minimum_first, minimum_second]
            .iter()
            .any(|v| !v.is_finite() || *v < 0.0)
        {
            return Err(ControlError::Invalid("split size"));
        }
        Ok(Self {
            first,
            minimum_first,
            minimum_second,
            drag_origin: None,
        })
    }
    pub fn arrange(&mut self, total: f32) -> Result<(f32, f32), ControlError> {
        if !total.is_finite() || total < self.minimum_first + self.minimum_second {
            return Err(ControlError::Invalid("split constraints"));
        }
        self.first = self
            .first
            .clamp(self.minimum_first, total - self.minimum_second);
        Ok((self.first, total - self.first))
    }
    pub fn begin(&mut self, coordinate: f32) -> Result<(), ControlError> {
        if !coordinate.is_finite() {
            return Err(ControlError::Invalid("split pointer"));
        }
        self.drag_origin = Some((coordinate, self.first));
        Ok(())
    }
    pub fn drag(&mut self, coordinate: f32, total: f32) -> Result<(), ControlError> {
        if !coordinate.is_finite() {
            return Err(ControlError::Invalid("split pointer"));
        }
        if let Some((origin, first)) = self.drag_origin {
            let old = self.first;
            self.first = first + coordinate - origin;
            if let Err(error) = self.arrange(total) {
                self.first = old;
                return Err(error);
            }
        }
        Ok(())
    }
    pub fn cancel(&mut self) {
        if let Some((_, first)) = self.drag_origin.take() {
            self.first = first;
        }
    }
    pub fn finish(&mut self) {
        self.drag_origin = None;
    }
    pub fn step(&mut self, delta: f32, total: f32) -> Result<(), ControlError> {
        if !delta.is_finite() {
            return Err(ControlError::Invalid("split step"));
        }
        let old = self.first;
        self.first += delta;
        if let Err(e) = self.arrange(total) {
            self.first = old;
            return Err(e);
        }
        Ok(())
    }
}
#[derive(Clone, Debug)]
pub struct MenuItem<A> {
    pub label: String,
    pub action: A,
    pub enabled: bool,
}
pub struct Menu<A> {
    pub items: Vec<MenuItem<A>>,
    pub focused: Option<usize>,
}
#[derive(Clone, Debug, PartialEq)]
pub enum MenuResponse<A> {
    Ignored,
    Handled,
    Dismiss,
    Activate(A),
}
impl<A: Clone> Menu<A> {
    pub fn key(&mut self, event: &view_core::KeyboardEvent) -> MenuResponse<A> {
        use view_core::{Key, KeyPhase, NamedKey};
        if event.phase == KeyPhase::Up {
            return MenuResponse::Ignored;
        }
        match event.key {
            Key::Named(NamedKey::ArrowDown) => self.navigate(true),
            Key::Named(NamedKey::ArrowUp) => self.navigate(false),
            Key::Named(NamedKey::Home) => {
                self.focused = None;
                self.navigate(true);
            }
            Key::Named(NamedKey::End) => {
                self.focused = self.items.iter().rposition(|item| item.enabled)
            }
            Key::Named(NamedKey::Escape) => return MenuResponse::Dismiss,
            Key::Named(NamedKey::Enter | NamedKey::Space) if event.phase == KeyPhase::Down => {
                return self
                    .activate()
                    .map_or(MenuResponse::Handled, MenuResponse::Activate);
            }
            _ => return MenuResponse::Ignored,
        }
        MenuResponse::Handled
    }
    pub fn navigate(&mut self, forward: bool) {
        let enabled: Vec<_> = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, i)| i.enabled)
            .map(|(i, _)| i)
            .collect();
        if enabled.is_empty() {
            self.focused = None;
            return;
        }
        let next = self
            .focused
            .and_then(|i| enabled.iter().position(|x| *x == i))
            .map_or(0, |i| {
                if forward {
                    (i + 1) % enabled.len()
                } else {
                    (i + enabled.len() - 1) % enabled.len()
                }
            });
        self.focused = Some(enabled[next]);
    }
    pub fn activate(&self) -> Option<A> {
        self.focused
            .and_then(|i| self.items.get(i))
            .filter(|i| i.enabled)
            .map(|i| i.action.clone())
    }
}
pub struct CommandPalette<A> {
    commands: Vec<MenuItem<A>>,
    query: String,
    pub menu: Menu<A>,
}
impl<A: Clone> CommandPalette<A> {
    pub fn new(commands: Vec<MenuItem<A>>) -> Self {
        Self {
            menu: Menu {
                items: commands.clone(),
                focused: None,
            },
            commands,
            query: String::new(),
        }
    }
    pub fn query(&mut self, query: &str) {
        self.query = query.into();
        let q = query.to_lowercase();
        self.menu.items = self
            .commands
            .iter()
            .filter(|c| c.label.to_lowercase().contains(&q))
            .cloned()
            .collect();
        self.menu.focused = None;
        self.menu.navigate(true);
    }
}
