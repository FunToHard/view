//! DPI awareness, text scale factor, and multi-monitor screen coordinate conversions.
//!
//! Handles per-monitor scale factors, text scale settings, and virtual desktop coordinates
//! including secondary monitors placed at negative desktop coordinates.

use view_core::{
    LogicalPoint, LogicalRect, LogicalSize, PhysicalPoint, PhysicalRect, PhysicalSize, ScaleFactor,
};

/// Coordinate and DPI state for a window, including screen position in virtual desktop space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DpiState {
    /// Window display scale factor.
    pub scale_factor: ScaleFactor,
    /// System text-scale factor (1.0 = 100%, 1.25 = 125%, etc.).
    pub text_scale: f32,
    /// Position of the client area top-left corner on the virtual desktop (can be negative).
    pub client_screen_origin: (i32, i32),
}

impl Default for DpiState {
    fn default() -> Self {
        Self {
            scale_factor: ScaleFactor::ONE,
            text_scale: 1.0,
            client_screen_origin: (0, 0),
        }
    }
}

impl DpiState {
    /// Create a new DPI state with scale factor, text scale, and screen position.
    pub fn new(
        scale_factor: ScaleFactor,
        text_scale: f32,
        client_screen_origin: (i32, i32),
    ) -> Self {
        Self {
            scale_factor,
            text_scale: if text_scale.is_finite() && text_scale > 0.0 {
                text_scale
            } else {
                1.0
            },
            client_screen_origin,
        }
    }

    /// Combined effective font scale factor (display scale * text scale).
    #[inline]
    pub fn effective_text_scale(&self) -> f32 {
        self.scale_factor.get() * self.text_scale
    }

    /// Convert client logical point to client physical point.
    #[inline]
    pub fn logical_to_physical_point(&self, point: LogicalPoint) -> PhysicalPoint {
        point.to_physical(self.scale_factor)
    }

    /// Convert client physical point to client logical point.
    #[inline]
    pub fn physical_to_logical_point(&self, point: PhysicalPoint) -> LogicalPoint {
        point.to_logical(self.scale_factor)
    }

    /// Convert client logical size to client physical size.
    #[inline]
    pub fn logical_to_physical_size(&self, size: LogicalSize) -> PhysicalSize {
        size.to_physical(self.scale_factor)
    }

    /// Convert client physical size to client logical size.
    #[inline]
    pub fn physical_to_logical_size(&self, size: PhysicalSize) -> LogicalSize {
        size.to_logical(self.scale_factor)
    }

    /// Convert client logical rect to client physical rect.
    #[inline]
    pub fn logical_to_physical_rect(&self, rect: LogicalRect) -> PhysicalRect {
        rect.to_physical(self.scale_factor)
    }

    /// Convert client physical rect to client logical rect.
    #[inline]
    pub fn physical_to_logical_rect(&self, rect: PhysicalRect) -> LogicalRect {
        rect.to_logical(self.scale_factor)
    }

    /// Convert a client physical point to absolute virtual desktop screen coordinates.
    pub fn client_physical_to_screen(&self, point: PhysicalPoint) -> (i32, i32) {
        (
            self.client_screen_origin
                .0
                .saturating_add(point.x.round() as i32),
            self.client_screen_origin
                .1
                .saturating_add(point.y.round() as i32),
        )
    }

    /// Convert absolute virtual desktop screen coordinates to a client physical point.
    pub fn screen_to_client_physical(&self, screen_coord: (i32, i32)) -> PhysicalPoint {
        PhysicalPoint::new(
            (screen_coord.0.saturating_sub(self.client_screen_origin.0)) as f32,
            (screen_coord.1.saturating_sub(self.client_screen_origin.1)) as f32,
        )
    }

    /// Convert client logical point to absolute virtual desktop screen coordinates.
    pub fn client_logical_to_screen(&self, point: LogicalPoint) -> (i32, i32) {
        let physical = self.logical_to_physical_point(point);
        self.client_physical_to_screen(physical)
    }

    /// Convert absolute virtual desktop screen coordinates to a client logical point.
    pub fn screen_to_client_logical(&self, screen_coord: (i32, i32)) -> LogicalPoint {
        let physical = self.screen_to_client_physical(screen_coord);
        self.physical_to_logical_point(physical)
    }
}
