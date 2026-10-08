//! Scrolling contracts: scroll state, nested boundary chaining, keyboard scrolling,
//! reveal-target, and virtual content realization.

use std::ops::Range;

use crate::{
    geometry::{LogicalPoint, LogicalRect, LogicalSize},
    input::{Key, NamedKey},
};

/// Policy for propagating unconsumed scroll deltas at boundary limits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScrollChaining {
    /// Propagate unconsumed delta to parent scroll container.
    #[default]
    Chain,
    /// Clamp at boundaries, never propagating to parents.
    Clamp,
}

/// 2D scroll state for scrollable containers.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScrollState {
    /// Current scroll offset in logical units.
    pub offset: LogicalPoint,
    /// Total extent of scrollable content.
    pub content_size: LogicalSize,
    /// Visible viewport size.
    pub viewport_size: LogicalSize,
    /// Chaining policy at boundaries.
    pub chaining: ScrollChaining,
}

impl ScrollState {
    /// Create a new scroll state.
    pub fn new(content_size: LogicalSize, viewport_size: LogicalSize) -> Self {
        Self {
            offset: LogicalPoint::ZERO,
            content_size,
            viewport_size,
            chaining: ScrollChaining::default(),
        }
    }

    /// Maximum valid scroll offset.
    pub fn max_offset(&self) -> LogicalPoint {
        LogicalPoint {
            x: (self.content_size.width - self.viewport_size.width).max(0.0),
            y: (self.content_size.height - self.viewport_size.height).max(0.0),
        }
    }

    /// Scroll by delta. Returns `(consumed_delta, unconsumed_delta)`.
    pub fn scroll_by(&mut self, delta: LogicalPoint) -> (LogicalPoint, LogicalPoint) {
        let max = self.max_offset();
        let target_x = self.offset.x + delta.x;
        let target_y = self.offset.y + delta.y;

        let clamped_x = target_x.clamp(0.0, max.x);
        let clamped_y = target_y.clamp(0.0, max.y);

        let consumed = LogicalPoint {
            x: clamped_x - self.offset.x,
            y: clamped_y - self.offset.y,
        };

        let unconsumed = match self.chaining {
            ScrollChaining::Chain => LogicalPoint {
                x: delta.x - consumed.x,
                y: delta.y - consumed.y,
            },
            ScrollChaining::Clamp => LogicalPoint::ZERO,
        };

        self.offset = LogicalPoint::new(clamped_x, clamped_y);
        (consumed, unconsumed)
    }

    /// Set offset clamped to valid bounds.
    pub fn scroll_to(&mut self, offset: LogicalPoint) {
        let max = self.max_offset();
        self.offset = LogicalPoint {
            x: offset.x.clamp(0.0, max.x),
            y: offset.y.clamp(0.0, max.y),
        };
    }

    /// Handle standard keyboard scrolling (arrow keys, page up/down, home/end).
    /// Returns true if scroll position changed.
    pub fn handle_key(&mut self, key: &Key, line_step: f32) -> bool {
        let old_offset = self.offset;
        match key {
            Key::Named(NamedKey::ArrowDown) => {
                self.scroll_by(LogicalPoint::new(0.0, line_step));
            }
            Key::Named(NamedKey::ArrowUp) => {
                self.scroll_by(LogicalPoint::new(0.0, -line_step));
            }
            Key::Named(NamedKey::ArrowRight) => {
                self.scroll_by(LogicalPoint::new(line_step, 0.0));
            }
            Key::Named(NamedKey::ArrowLeft) => {
                self.scroll_by(LogicalPoint::new(-line_step, 0.0));
            }
            Key::Named(NamedKey::PageDown) => {
                self.scroll_by(LogicalPoint::new(0.0, self.viewport_size.height));
            }
            Key::Named(NamedKey::PageUp) => {
                self.scroll_by(LogicalPoint::new(0.0, -self.viewport_size.height));
            }
            Key::Named(NamedKey::Home) => {
                self.scroll_to(LogicalPoint::new(self.offset.x, 0.0));
            }
            Key::Named(NamedKey::End) => {
                let max = self.max_offset();
                self.scroll_to(LogicalPoint::new(self.offset.x, max.y));
            }
            _ => return false,
        }
        self.offset != old_offset
    }

    /// Minimally adjust offset to bring `target` into the visible viewport rect.
    pub fn reveal_rect(&mut self, target: LogicalRect) {
        let mut new_x = self.offset.x;
        let mut new_y = self.offset.y;

        let vp_right = self.offset.x + self.viewport_size.width;
        let vp_bottom = self.offset.y + self.viewport_size.height;

        if target.left() < self.offset.x {
            new_x = target.left();
        } else if target.right() > vp_right {
            new_x = target.right() - self.viewport_size.width;
        }

        if target.top() < self.offset.y {
            new_y = target.top();
        } else if target.bottom() > vp_bottom {
            new_y = target.bottom() - self.viewport_size.height;
        }

        self.scroll_to(LogicalPoint::new(new_x, new_y));
    }
}

/// Realization contract for virtualized linear collections.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VirtualScroll {
    /// Total number of logical items.
    pub total_items: usize,
    /// Fixed height/extent per item.
    pub item_extent: f32,
}

impl VirtualScroll {
    /// Create a virtual scroll realization specifier.
    pub fn new(total_items: usize, item_extent: f32) -> Self {
        Self {
            total_items,
            item_extent: item_extent.max(1.0),
        }
    }

    /// Total content size based on item count and extent.
    pub fn total_content_size(&self, width: f32) -> LogicalSize {
        LogicalSize::new(width, self.total_items as f32 * self.item_extent)
    }

    /// Calculate the visible range of item indices given viewport offset and height.
    ///
    /// Expands the range by `buffer_items` above and below to prevent flashing during fast scrolls.
    pub fn visible_range(
        &self,
        scroll_y: f32,
        viewport_height: f32,
        buffer_items: usize,
    ) -> Range<usize> {
        if self.total_items == 0 || self.item_extent <= 0.0 {
            return 0..0;
        }

        let first_visible = (scroll_y / self.item_extent).floor().max(0.0) as usize;
        let visible_count = (viewport_height / self.item_extent).ceil() as usize + 1;

        let start = first_visible.saturating_sub(buffer_items);
        let end = (first_visible + visible_count + buffer_items).min(self.total_items);

        start..end
    }
}
