//! Input event model, hover/press/cancel state machines, pointer capture, and routing.
//!
//! Event routing targets committed geometry. Geometry-dependent input flushes pending
//! commits before resolving targets.

use crate::{NodeId, geometry::LogicalPoint, hit_test::HitTestResult};

/// Pointer buttons.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PointerButton {
    /// Primary (usually left mouse button or touch contact).
    Primary,
    /// Secondary (usually right mouse button).
    Secondary,
    /// Auxiliary (usually middle mouse button / wheel click).
    Auxiliary,
}

/// Pointer event lifecycle phase.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PointerPhase {
    /// Pointer pressed down.
    Down,
    /// Pointer released up.
    Up,
    /// Pointer moved.
    Move,
    /// Pointer operation cancelled (e.g. system gesture, capture loss).
    Cancel,
    /// Wheel or trackpad scroll delta.
    Scroll {
        /// Horizontal scroll delta.
        delta_x: f32,
        /// Vertical scroll delta.
        delta_y: f32,
    },
}

/// Normalized pointer event.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointerEvent {
    /// Interaction phase.
    pub phase: PointerPhase,
    /// Position in window logical coordinates.
    pub position: LogicalPoint,
    /// Button associated with the event, if any.
    pub button: Option<PointerButton>,
    /// Active keyboard modifiers.
    pub modifiers: Modifiers,
    /// Monotonic timestamp in microseconds.
    pub timestamp: u64,
}

/// Keyboard modifiers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Modifiers {
    /// Shift key held.
    pub shift: bool,
    /// Control key held.
    pub ctrl: bool,
    /// Alt key held.
    pub alt: bool,
    /// Windows/Command key held.
    pub meta: bool,
}

/// Common named keys for shortcuts and navigation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NamedKey {
    /// Tab key.
    Tab,
    /// Enter / Return key.
    Enter,
    /// Escape key.
    Escape,
    /// Space key.
    Space,
    /// Backspace key.
    Backspace,
    /// Delete key.
    Delete,
    /// Up arrow.
    ArrowUp,
    /// Down arrow.
    ArrowDown,
    /// Left arrow.
    ArrowLeft,
    /// Right arrow.
    ArrowRight,
    /// Page Up.
    PageUp,
    /// Page Down.
    PageDown,
    /// Home.
    Home,
    /// End.
    End,
}

/// Logical key representation.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    /// Standard named key.
    Named(NamedKey),
    /// Character produced by keypress (e.g. "a", "Z", "1").
    Character(String),
    /// Key that is not identified or mapped.
    Unidentified,
}

/// Keyboard event phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyPhase {
    /// Key pressed down.
    Down,
    /// Key released up.
    Up,
    /// Key repeat from held press.
    Repeat,
}

/// Normalized keyboard event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyboardEvent {
    /// Key phase.
    pub phase: KeyPhase,
    /// Key identity.
    pub key: Key,
    /// Active modifiers.
    pub modifiers: Modifiers,
    /// Timestamp in microseconds.
    pub timestamp: u64,
}

/// Normalized routed pointer action delivered to a node.
#[derive(Clone, Debug, PartialEq)]
pub enum RoutedPointerAction {
    /// Cursor entered node boundaries.
    Enter(NodeId),
    /// Cursor left node boundaries.
    Leave(NodeId),
    /// Cursor moved over node.
    Move {
        /// Target node.
        target: NodeId,
        /// Position in target's local coordinate space.
        local: LogicalPoint,
    },
    /// Button pressed down on node.
    Down {
        /// Target node.
        target: NodeId,
        /// Pressed button.
        button: PointerButton,
        /// Position in target's local coordinate space.
        local: LogicalPoint,
    },
    /// Button released on node.
    Up {
        /// Target node.
        target: NodeId,
        /// Released button.
        button: PointerButton,
        /// Position in target's local coordinate space.
        local: LogicalPoint,
    },
    /// Successful click / activation (down and up on the same target).
    Click {
        /// Target node.
        target: NodeId,
        /// Clicked button.
        button: PointerButton,
    },
    /// Interaction cancelled (e.g. capture loss or window deactivated).
    Cancel(NodeId),
    /// Wheel/trackpad scroll over node.
    Scroll {
        /// Target node.
        target: NodeId,
        /// Horizontal delta.
        delta_x: f32,
        /// Vertical delta.
        delta_y: f32,
    },
}

/// Pointer router maintaining hover, press, and capture state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PointerRouter {
    hovered: Option<NodeId>,
    pressed: Option<(NodeId, PointerButton)>,
    captured: Option<NodeId>,
}

impl PointerRouter {
    /// Currently hovered node.
    #[inline]
    pub fn hovered(&self) -> Option<NodeId> {
        self.hovered
    }

    /// Currently pressed node and button.
    #[inline]
    pub fn pressed(&self) -> Option<(NodeId, PointerButton)> {
        self.pressed
    }

    /// Node currently capturing the pointer.
    #[inline]
    pub fn captured(&self) -> Option<NodeId> {
        self.captured
    }

    /// Grant pointer capture to a node.
    pub fn set_capture(&mut self, node: NodeId) {
        self.captured = Some(node);
    }

    /// Explicitly release pointer capture.
    pub fn release_capture(&mut self) -> Option<RoutedPointerAction> {
        self.captured.take().map(RoutedPointerAction::Cancel)
    }

    /// Reset router on window deactivation or blur, returning cancellation actions.
    pub fn reset(&mut self) -> Vec<RoutedPointerAction> {
        let mut actions = Vec::new();
        if let Some(node) = self.captured.take() {
            actions.push(RoutedPointerAction::Cancel(node));
        }
        if let Some((node, _)) = self.pressed.take() {
            actions.push(RoutedPointerAction::Cancel(node));
        }
        if let Some(node) = self.hovered.take() {
            actions.push(RoutedPointerAction::Leave(node));
        }
        actions
    }

    /// Route a pointer event against hit-tested geometry.
    pub fn route(
        &mut self,
        event: PointerEvent,
        hit_test: impl FnOnce(LogicalPoint) -> Option<HitTestResult>,
    ) -> Vec<RoutedPointerAction> {
        let mut actions = Vec::new();

        // 1. If capture is active, route directly to the capturing node.
        if let Some(captured_node) = self.captured {
            match event.phase {
                PointerPhase::Move => {
                    actions.push(RoutedPointerAction::Move {
                        target: captured_node,
                        local: event.position,
                    });
                }
                PointerPhase::Down => {
                    if let Some(btn) = event.button {
                        actions.push(RoutedPointerAction::Down {
                            target: captured_node,
                            button: btn,
                            local: event.position,
                        });
                    }
                }
                PointerPhase::Up => {
                    if let Some(btn) = event.button {
                        actions.push(RoutedPointerAction::Up {
                            target: captured_node,
                            button: btn,
                            local: event.position,
                        });
                        if self.pressed == Some((captured_node, btn)) {
                            actions.push(RoutedPointerAction::Click {
                                target: captured_node,
                                button: btn,
                            });
                        }
                    }
                    self.captured = None;
                    self.pressed = None;
                }
                PointerPhase::Cancel => {
                    actions.push(RoutedPointerAction::Cancel(captured_node));
                    self.captured = None;
                    self.pressed = None;
                }
                PointerPhase::Scroll { delta_x, delta_y } => {
                    actions.push(RoutedPointerAction::Scroll {
                        target: captured_node,
                        delta_x,
                        delta_y,
                    });
                }
            }
            return actions;
        }

        // 2. Perform hit-test against committed geometry.
        let hit = hit_test(event.position);
        let current_target = hit.as_ref().map(|h| h.target);
        let local_point = hit.as_ref().map_or(event.position, |h| h.local_point);

        // 3. Update hover transitions.
        if current_target != self.hovered {
            if let Some(old) = self.hovered.take() {
                actions.push(RoutedPointerAction::Leave(old));
            }
            if let Some(new_target) = current_target {
                actions.push(RoutedPointerAction::Enter(new_target));
                self.hovered = Some(new_target);
            }
        }

        // 4. Handle event phases.
        match event.phase {
            PointerPhase::Move => {
                if let Some(target) = current_target {
                    actions.push(RoutedPointerAction::Move {
                        target,
                        local: local_point,
                    });
                }
            }
            PointerPhase::Down => {
                if let (Some(target), Some(btn)) = (current_target, event.button) {
                    self.pressed = Some((target, btn));
                    actions.push(RoutedPointerAction::Down {
                        target,
                        button: btn,
                        local: local_point,
                    });
                }
            }
            PointerPhase::Up => {
                if let (Some(target), Some(btn)) = (current_target, event.button) {
                    actions.push(RoutedPointerAction::Up {
                        target,
                        button: btn,
                        local: local_point,
                    });
                    if self.pressed == Some((target, btn)) {
                        actions.push(RoutedPointerAction::Click {
                            target,
                            button: btn,
                        });
                    }
                }
                self.pressed = None;
            }
            PointerPhase::Cancel => {
                if let Some((target, _)) = self.pressed.take() {
                    actions.push(RoutedPointerAction::Cancel(target));
                }
            }
            PointerPhase::Scroll { delta_x, delta_y } => {
                if let Some(target) = current_target {
                    actions.push(RoutedPointerAction::Scroll {
                        target,
                        delta_x,
                        delta_y,
                    });
                }
            }
        }

        actions
    }
}
