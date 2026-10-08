//! Platform services: cursor management and IME/text input integration.

use view_core::{LogicalRect, ScaleFactor};
use winit::window::Window as WinitWindow;

use crate::error::PlatformError;

/// Standard platform cursor shapes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CursorIcon {
    /// Default arrow pointer.
    #[default]
    Default,
    /// Pointing hand (links, buttons).
    Pointer,
    /// I-beam text insertion caret.
    Text,
    /// Precision selection crosshair.
    Crosshair,
    /// Omnidirectional move / pan.
    Move,
    /// Action not allowed / disabled.
    NotAllowed,
    /// Open hand for dragging.
    Grab,
    /// Closed hand during active drag.
    Grabbing,
    /// Column / horizontal resize.
    ColResize,
    /// Row / vertical resize.
    RowResize,
    /// Waiting / busy.
    Wait,
    /// Busy with background progress.
    Progress,
}

impl CursorIcon {
    /// Convert to winit's cursor icon.
    pub fn to_winit(self) -> winit::window::CursorIcon {
        match self {
            Self::Default => winit::window::CursorIcon::Default,
            Self::Pointer => winit::window::CursorIcon::Pointer,
            Self::Text => winit::window::CursorIcon::Text,
            Self::Crosshair => winit::window::CursorIcon::Crosshair,
            Self::Move => winit::window::CursorIcon::Move,
            Self::NotAllowed => winit::window::CursorIcon::NotAllowed,
            Self::Grab => winit::window::CursorIcon::Grab,
            Self::Grabbing => winit::window::CursorIcon::Grabbing,
            Self::ColResize => winit::window::CursorIcon::ColResize,
            Self::RowResize => winit::window::CursorIcon::RowResize,
            Self::Wait => winit::window::CursorIcon::Wait,
            Self::Progress => winit::window::CursorIcon::Progress,
        }
    }
}

/// Service managing cursor shape and visibility.
pub struct CursorService;

impl CursorService {
    /// Update the cursor icon for a window.
    pub fn set_cursor(window: &WinitWindow, icon: CursorIcon) -> Result<(), PlatformError> {
        window.set_cursor(icon.to_winit());
        Ok(())
    }

    /// Set cursor visibility.
    pub fn set_cursor_visible(window: &WinitWindow, visible: bool) -> Result<(), PlatformError> {
        window.set_cursor_visible(visible);
        Ok(())
    }
}

/// Service managing platform IME (Input Method Editor) and candidate window positioning.
pub struct ImeService;

impl ImeService {
    /// Enable or disable IME text composition events for a window.
    pub fn set_ime_allowed(window: &WinitWindow, allowed: bool) -> Result<(), PlatformError> {
        window.set_ime_allowed(allowed);
        Ok(())
    }

    /// Report the active text caret/candidate positioning area in window logical coordinates.
    pub fn set_ime_cursor_area(
        window: &WinitWindow,
        area: LogicalRect,
        scale: ScaleFactor,
    ) -> Result<(), PlatformError> {
        let physical = area.to_physical(scale);
        window.set_ime_cursor_area(
            winit::dpi::PhysicalPosition::new(physical.origin.x as i32, physical.origin.y as i32),
            winit::dpi::PhysicalSize::new(physical.size.width as u32, physical.size.height as u32),
        );
        Ok(())
    }
}
