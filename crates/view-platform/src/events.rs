//! Native event translation and input normalization.

use view_core::{
    Key, KeyPhase, KeyboardEvent, LogicalPoint, LogicalSize, Modifiers, NamedKey, PhysicalSize,
    PointerButton, PointerEvent, PointerPhase, ScaleFactor, WindowId,
};
use winit::event::{ElementState, Ime, MouseButton, MouseScrollDelta, WindowEvent};
use winit::keyboard::{Key as WinitKey, NamedKey as WinitNamedKey};

/// Input Method Editor (IME) text composition events.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImeEvent {
    /// IME text composition has been enabled.
    Enabled,
    /// IME preedit string update with optional cursor byte range (start, end).
    Preedit(String, Option<(usize, usize)>),
    /// IME commit string inserted into text stream.
    Commit(String),
    /// IME text composition has been disabled.
    Disabled,
}

/// Normalized window events delivered to the framework shell and application.
#[derive(Clone, Debug, PartialEq)]
pub enum PlatformWindowEvent {
    /// Pointer movement, button click, capture or scroll.
    Pointer(PointerEvent),
    /// Physical or virtual keyboard event.
    Keyboard(KeyboardEvent),
    /// Text composition / IME stream.
    Ime(ImeEvent),
    /// Window client area resized.
    Resized {
        /// Updated client area logical size.
        logical_size: LogicalSize,
        /// Updated client area physical size in pixels.
        physical_size: PhysicalSize,
    },
    /// Monitor DPI or scale factor changed.
    ScaleFactorChanged(ScaleFactor),
    /// Window position changed on screen (in desktop coordinates).
    Moved {
        /// Client screen origin (top-left).
        screen_origin: (i32, i32),
    },
    /// Window received or lost input focus.
    Focused(bool),
    /// Window was minimized or restored.
    Occluded(bool),
    /// OS or user requested closing the window.
    CloseRequested,
    /// Window was destroyed and native resources released.
    Destroyed,
    /// Window needs a redraw.
    RedrawRequested,
}

/// State tracking for pointer positions and modifiers across native events.
#[derive(Clone, Debug, Default)]
pub struct InputStateTracker {
    /// Last known cursor position in window logical coordinates.
    pub last_cursor_position: Option<LogicalPoint>,
    /// Active modifiers state.
    pub modifiers: Modifiers,
    /// Currently held mouse buttons.
    pub pressed_buttons: Vec<PointerButton>,
    /// Last event timestamp in microseconds.
    pub timestamp_ms: u64,
}

impl InputStateTracker {
    /// Create a new tracker with default state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Update internal modifier state from winit modifiers.
    pub fn update_modifiers(&mut self, state: winit::keyboard::ModifiersState) {
        self.modifiers = Modifiers {
            shift: state.shift_key(),
            ctrl: state.control_key(),
            alt: state.alt_key(),
            meta: state.super_key(),
        };
    }

    /// Reset pressed buttons and return cancel events if buttons were held down.
    pub fn cancel_held_buttons(&mut self) -> Vec<PointerEvent> {
        let mut events = Vec::new();
        let pos = self.last_cursor_position.unwrap_or(LogicalPoint::ZERO);

        for &btn in &self.pressed_buttons {
            events.push(PointerEvent {
                phase: PointerPhase::Cancel,
                position: pos,
                button: Some(btn),
                modifiers: self.modifiers,
                timestamp: self.timestamp_ms,
            });
        }
        self.pressed_buttons.clear();
        events
    }
}

/// Translate a winit `WindowEvent` into an optional `PlatformWindowEvent`.
pub fn translate_window_event(
    event: &WindowEvent,
    _window_id: WindowId,
    scale_factor: ScaleFactor,
    tracker: &mut InputStateTracker,
    timestamp_ms: u64,
) -> Option<PlatformWindowEvent> {
    tracker.timestamp_ms = timestamp_ms;

    match event {
        WindowEvent::Resized(physical_size) => {
            let phys = PhysicalSize::from_pixels(physical_size.width, physical_size.height);
            let log = phys.to_logical(scale_factor);
            Some(PlatformWindowEvent::Resized {
                logical_size: log,
                physical_size: phys,
            })
        }

        WindowEvent::Moved(pos) => Some(PlatformWindowEvent::Moved {
            screen_origin: (pos.x, pos.y),
        }),

        WindowEvent::CloseRequested => Some(PlatformWindowEvent::CloseRequested),

        WindowEvent::Destroyed => Some(PlatformWindowEvent::Destroyed),

        WindowEvent::Focused(focused) => Some(PlatformWindowEvent::Focused(*focused)),

        WindowEvent::Occluded(occluded) => Some(PlatformWindowEvent::Occluded(*occluded)),

        WindowEvent::ScaleFactorChanged {
            scale_factor: factor,
            ..
        } => {
            let scale = ScaleFactor::new(*factor as f32).unwrap_or(ScaleFactor::ONE);
            Some(PlatformWindowEvent::ScaleFactorChanged(scale))
        }

        WindowEvent::ModifiersChanged(modifiers) => {
            tracker.update_modifiers(modifiers.state());
            None
        }

        WindowEvent::CursorMoved { position, .. } => {
            let logical_pos = LogicalPoint::new(
                (position.x as f32 / scale_factor.get()).max(0.0),
                (position.y as f32 / scale_factor.get()).max(0.0),
            );
            tracker.last_cursor_position = Some(logical_pos);

            Some(PlatformWindowEvent::Pointer(PointerEvent {
                phase: PointerPhase::Move,
                position: logical_pos,
                button: tracker.pressed_buttons.first().copied(),
                modifiers: tracker.modifiers,
                timestamp: timestamp_ms,
            }))
        }

        WindowEvent::CursorLeft { .. } => {
            let pos = tracker.last_cursor_position.unwrap_or(LogicalPoint::ZERO);
            Some(PlatformWindowEvent::Pointer(PointerEvent {
                phase: PointerPhase::Cancel,
                position: pos,
                button: None,
                modifiers: tracker.modifiers,
                timestamp: timestamp_ms,
            }))
        }

        WindowEvent::MouseInput { state, button, .. } => {
            let btn = match button {
                MouseButton::Left => Some(PointerButton::Primary),
                MouseButton::Right => Some(PointerButton::Secondary),
                MouseButton::Middle => Some(PointerButton::Auxiliary),
                _ => None,
            };

            let pos = tracker.last_cursor_position.unwrap_or(LogicalPoint::ZERO);

            let phase = match state {
                ElementState::Pressed => {
                    if let Some(b) = btn
                        && !tracker.pressed_buttons.contains(&b)
                    {
                        tracker.pressed_buttons.push(b);
                    }
                    PointerPhase::Down
                }
                ElementState::Released => {
                    if let Some(b) = btn {
                        tracker.pressed_buttons.retain(|&x| x != b);
                    }
                    PointerPhase::Up
                }
            };

            Some(PlatformWindowEvent::Pointer(PointerEvent {
                phase,
                position: pos,
                button: btn,
                modifiers: tracker.modifiers,
                timestamp: timestamp_ms,
            }))
        }

        WindowEvent::MouseWheel { delta, .. } => {
            let (dx, dy) = match delta {
                MouseScrollDelta::LineDelta(x, y) => (*x * 24.0, *y * 24.0),
                MouseScrollDelta::PixelDelta(pos) => (
                    pos.x as f32 / scale_factor.get(),
                    pos.y as f32 / scale_factor.get(),
                ),
            };

            let pos = tracker.last_cursor_position.unwrap_or(LogicalPoint::ZERO);

            Some(PlatformWindowEvent::Pointer(PointerEvent {
                phase: PointerPhase::Scroll {
                    delta_x: dx,
                    delta_y: dy,
                },
                position: pos,
                button: None,
                modifiers: tracker.modifiers,
                timestamp: timestamp_ms,
            }))
        }

        WindowEvent::KeyboardInput { event, .. } => {
            let phase = match (event.state, event.repeat) {
                (ElementState::Pressed, false) => KeyPhase::Down,
                (ElementState::Pressed, true) => KeyPhase::Repeat,
                (ElementState::Released, _) => KeyPhase::Up,
            };

            let key = match &event.logical_key {
                WinitKey::Named(named) => match named {
                    WinitNamedKey::Tab => Key::Named(NamedKey::Tab),
                    WinitNamedKey::Enter => Key::Named(NamedKey::Enter),
                    WinitNamedKey::Escape => Key::Named(NamedKey::Escape),
                    WinitNamedKey::Space => Key::Named(NamedKey::Space),
                    WinitNamedKey::ArrowUp => Key::Named(NamedKey::ArrowUp),
                    WinitNamedKey::ArrowDown => Key::Named(NamedKey::ArrowDown),
                    WinitNamedKey::ArrowLeft => Key::Named(NamedKey::ArrowLeft),
                    WinitNamedKey::ArrowRight => Key::Named(NamedKey::ArrowRight),
                    WinitNamedKey::Home => Key::Named(NamedKey::Home),
                    WinitNamedKey::End => Key::Named(NamedKey::End),
                    WinitNamedKey::PageUp => Key::Named(NamedKey::PageUp),
                    WinitNamedKey::PageDown => Key::Named(NamedKey::PageDown),
                    WinitNamedKey::Backspace => Key::Named(NamedKey::Backspace),
                    WinitNamedKey::Delete => Key::Named(NamedKey::Delete),
                    _ => Key::Unidentified,
                },
                WinitKey::Character(s) => Key::Character(s.to_string()),
                _ => Key::Unidentified,
            };

            Some(PlatformWindowEvent::Keyboard(KeyboardEvent {
                phase,
                key,
                modifiers: tracker.modifiers,
                timestamp: timestamp_ms,
            }))
        }

        WindowEvent::Ime(ime) => {
            let ime_event = match ime {
                Ime::Enabled => ImeEvent::Enabled,
                Ime::Preedit(text, cursor) => ImeEvent::Preedit(text.clone(), *cursor),
                Ime::Commit(text) => ImeEvent::Commit(text.clone()),
                Ime::Disabled => ImeEvent::Disabled,
            };
            Some(PlatformWindowEvent::Ime(ime_event))
        }

        WindowEvent::RedrawRequested => Some(PlatformWindowEvent::RedrawRequested),

        _ => None,
    }
}
