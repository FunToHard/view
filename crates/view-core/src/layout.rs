//! Layout contracts: constraints, pure measurement, arrangements, and caching.
//!
//! Measurement is pure with respect to side effects. Primitives include Padding,
//! Alignment, Flex (Row/Column), and Stack.

use crate::{
    CoreError, Dirty,
    geometry::{LogicalInsets, LogicalPoint, LogicalRect, LogicalSize},
};

/// 2D box constraints on width and height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Constraints {
    /// Minimum width (non-negative and finite).
    pub min_width: f32,
    /// Maximum width (>= min_width, finite or +infinity).
    pub max_width: f32,
    /// Minimum height (non-negative and finite).
    pub min_height: f32,
    /// Maximum height (>= min_height, finite or +infinity).
    pub max_height: f32,
}

impl Constraints {
    /// Create validated box constraints.
    pub fn new(
        min_width: f32,
        max_width: f32,
        min_height: f32,
        max_height: f32,
    ) -> Result<Self, CoreError> {
        let c = Self {
            min_width,
            max_width,
            min_height,
            max_height,
        };
        if c.is_valid() {
            Ok(c)
        } else {
            Err(CoreError::InvalidConstraints)
        }
    }

    /// True if constraints are mathematically sound:
    /// min >= 0, max >= min, min is finite, max is not NaN.
    pub fn is_valid(self) -> bool {
        self.min_width.is_finite()
            && self.min_height.is_finite()
            && !self.max_width.is_nan()
            && !self.max_height.is_nan()
            && self.min_width >= 0.0
            && self.min_height >= 0.0
            && self.max_width >= self.min_width
            && self.max_height >= self.min_height
    }

    /// Tight constraints enforcing an exact size.
    pub fn tight(size: LogicalSize) -> Self {
        Self {
            min_width: size.width,
            max_width: size.width,
            min_height: size.height,
            max_height: size.height,
        }
    }

    /// Loose constraints with minimum 0 up to max dimensions.
    pub fn loose(max_size: LogicalSize) -> Self {
        Self {
            min_width: 0.0,
            max_width: max_size.width,
            min_height: 0.0,
            max_height: max_size.height,
        }
    }

    /// Unbounded constraints from 0 to infinity.
    pub fn unbounded() -> Self {
        Self {
            min_width: 0.0,
            max_width: f32::INFINITY,
            min_height: 0.0,
            max_height: f32::INFINITY,
        }
    }

    /// True if both width and height are tightly constrained.
    #[inline]
    pub fn is_tight(self) -> bool {
        self.min_width == self.max_width && self.min_height == self.max_height
    }

    /// True if maximum dimensions are finite.
    #[inline]
    pub fn is_bounded(self) -> bool {
        self.max_width.is_finite() && self.max_height.is_finite()
    }

    /// Clamp a size into these constraints.
    pub fn constrain(self, size: LogicalSize) -> LogicalSize {
        LogicalSize {
            width: size.width.clamp(self.min_width, self.max_width),
            height: size.height.clamp(self.min_height, self.max_height),
        }
    }

    /// Deflate constraints by insets (e.g. for padding).
    pub fn deflate(self, insets: LogicalInsets) -> Self {
        let min_w = (self.min_width - insets.horizontal()).max(0.0);
        let max_w = if self.max_width.is_finite() {
            (self.max_width - insets.horizontal()).max(0.0)
        } else {
            f32::INFINITY
        };
        let min_h = (self.min_height - insets.vertical()).max(0.0);
        let max_h = if self.max_height.is_finite() {
            (self.max_height - insets.vertical()).max(0.0)
        } else {
            f32::INFINITY
        };
        Self {
            min_width: min_w,
            max_width: max_w.max(min_w),
            min_height: min_h,
            max_height: max_h.max(min_h),
        }
    }
}

/// 2D alignment point in normalized coordinates `[-1.0, 1.0]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Alignment {
    /// Horizontal alignment: -1.0 is left, 0.0 is center, 1.0 is right.
    pub x: f32,
    /// Vertical alignment: -1.0 is top, 0.0 is center, 1.0 is bottom.
    pub y: f32,
}

impl Default for Alignment {
    fn default() -> Self {
        Self::TOP_LEFT
    }
}

impl Alignment {
    /// Top left (-1.0, -1.0).
    pub const TOP_LEFT: Self = Self { x: -1.0, y: -1.0 };
    /// Top center (0.0, -1.0).
    pub const TOP_CENTER: Self = Self { x: 0.0, y: -1.0 };
    /// Top right (1.0, -1.0).
    pub const TOP_RIGHT: Self = Self { x: 1.0, y: -1.0 };
    /// Center left (-1.0, 0.0).
    pub const CENTER_LEFT: Self = Self { x: -1.0, y: 0.0 };
    /// Center (0.0, 0.0).
    pub const CENTER: Self = Self { x: 0.0, y: 0.0 };
    /// Center right (1.0, 0.0).
    pub const CENTER_RIGHT: Self = Self { x: 1.0, y: 0.0 };
    /// Bottom left (-1.0, 1.0).
    pub const BOTTOM_LEFT: Self = Self { x: -1.0, y: 1.0 };
    /// Bottom center (0.0, 1.0).
    pub const BOTTOM_CENTER: Self = Self { x: 0.0, y: 1.0 };
    /// Bottom right (1.0, 1.0).
    pub const BOTTOM_RIGHT: Self = Self { x: 1.0, y: 1.0 };

    /// Calculate the top-left offset to place a child of `child_size` inside `parent_size`.
    pub fn align(self, parent_size: LogicalSize, child_size: LogicalSize) -> LogicalPoint {
        let rem_w = (parent_size.width - child_size.width).max(0.0);
        let rem_h = (parent_size.height - child_size.height).max(0.0);
        LogicalPoint {
            x: rem_w * (self.x + 1.0) * 0.5,
            y: rem_h * (self.y + 1.0) * 0.5,
        }
    }
}

/// Axis for flex layouts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    /// Horizontal (Row).
    Horizontal,
    /// Vertical (Column).
    Vertical,
}

/// Alignment along the main axis of a flex container.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MainAxisAlignment {
    /// Pack children toward the start.
    #[default]
    Start,
    /// Center children along the main axis.
    Center,
    /// Pack children toward the end.
    End,
    /// Distribute remaining space evenly between children.
    SpaceBetween,
    /// Distribute space evenly with half-space at the ends.
    SpaceAround,
    /// Distribute space evenly with full space around all items.
    SpaceEvenly,
}

/// Alignment along the cross axis of a flex container.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CrossAxisAlignment {
    /// Place children at start of cross axis.
    #[default]
    Start,
    /// Center children along cross axis.
    Center,
    /// Place children at end of cross axis.
    End,
    /// Stretch children to fill cross axis.
    Stretch,
}

/// Specification for a child in a Flex (Row/Column) layout.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlexItem {
    /// Flex grow factor (0.0 means unexpanded/intrinsic).
    pub flex_factor: f32,
}

impl Default for FlexItem {
    fn default() -> Self {
        Self { flex_factor: 0.0 }
    }
}

impl FlexItem {
    /// Create a fixed non-flex item.
    pub const FIXED: Self = Self { flex_factor: 0.0 };
    /// Create an expanded item with factor 1.0.
    pub const EXPANDED: Self = Self { flex_factor: 1.0 };
}

/// Pure measurement and arrangement for Row and Column layouts.
pub struct FlexLayout {
    /// Main direction.
    pub axis: Axis,
    /// Main axis alignment.
    pub main_axis_alignment: MainAxisAlignment,
    /// Cross axis alignment.
    pub cross_axis_alignment: CrossAxisAlignment,
    /// Spacing between consecutive items.
    pub spacing: f32,
}

impl FlexLayout {
    /// Pure measure and arrange of children in flex container.
    pub fn layout(
        &self,
        constraints: Constraints,
        children: &[(FlexItem, LogicalSize)],
    ) -> (LogicalSize, Vec<LogicalRect>) {
        if children.is_empty() {
            let size = constraints.constrain(LogicalSize::ZERO);
            return (size, Vec::new());
        }

        let num_children = children.len();
        let total_spacing = self.spacing * (num_children.saturating_sub(1) as f32);

        // Separate fixed sizes and flex factors.
        let mut fixed_main_total = total_spacing;
        let mut total_flex = 0.0f32;
        let mut max_cross = 0.0f32;

        for (item, size) in children {
            let (main, cross) = match self.axis {
                Axis::Horizontal => (size.width, size.height),
                Axis::Vertical => (size.height, size.width),
            };
            if item.flex_factor > 0.0 {
                total_flex += item.flex_factor;
            } else {
                fixed_main_total += main;
            }
            max_cross = max_cross.max(cross);
        }

        let max_main = match self.axis {
            Axis::Horizontal => constraints.max_width,
            Axis::Vertical => constraints.max_height,
        };
        let min_main = match self.axis {
            Axis::Horizontal => constraints.min_width,
            Axis::Vertical => constraints.min_height,
        };

        // Determine final main size and distribute to flex items.
        let remaining_space = if max_main.is_finite() && total_flex > 0.0 {
            (max_main - fixed_main_total).max(0.0)
        } else {
            0.0
        };

        let mut child_sizes = Vec::with_capacity(num_children);
        let mut actual_main_total = total_spacing;

        for (item, size) in children {
            let (mut main, mut cross) = match self.axis {
                Axis::Horizontal => (size.width, size.height),
                Axis::Vertical => (size.height, size.width),
            };
            if item.flex_factor > 0.0 && total_flex > 0.0 {
                main = remaining_space * (item.flex_factor / total_flex);
            }
            if self.cross_axis_alignment == CrossAxisAlignment::Stretch {
                let cross_max = match self.axis {
                    Axis::Horizontal => constraints.max_height,
                    Axis::Vertical => constraints.max_width,
                };
                if cross_max.is_finite() {
                    cross = cross_max;
                }
            }
            actual_main_total += main;
            max_cross = max_cross.max(cross);
            child_sizes.push((main, cross));
        }

        let final_main = actual_main_total.clamp(min_main, max_main);
        let final_cross = match self.axis {
            Axis::Horizontal => max_cross.clamp(constraints.min_height, constraints.max_height),
            Axis::Vertical => max_cross.clamp(constraints.min_width, constraints.max_width),
        };

        let container_size = match self.axis {
            Axis::Horizontal => LogicalSize::new(final_main, final_cross),
            Axis::Vertical => LogicalSize::new(final_cross, final_main),
        };

        // Arrange along main axis according to MainAxisAlignment.
        let free_main = (final_main - actual_main_total).max(0.0);
        let (leading_space, gap_space) = match self.main_axis_alignment {
            MainAxisAlignment::Start => (0.0, self.spacing),
            MainAxisAlignment::End => (free_main, self.spacing),
            MainAxisAlignment::Center => (free_main * 0.5, self.spacing),
            MainAxisAlignment::SpaceBetween => {
                let gap = if num_children > 1 {
                    self.spacing + free_main / (num_children - 1) as f32
                } else {
                    0.0
                };
                (0.0, gap)
            }
            MainAxisAlignment::SpaceAround => {
                let gap = free_main / num_children as f32;
                (gap * 0.5, self.spacing + gap)
            }
            MainAxisAlignment::SpaceEvenly => {
                let gap = free_main / (num_children + 1) as f32;
                (gap, self.spacing + gap)
            }
        };

        let mut current_main = leading_space;
        let mut rects = Vec::with_capacity(num_children);

        for (main, cross) in child_sizes {
            let cross_pos = match self.cross_axis_alignment {
                CrossAxisAlignment::Start | CrossAxisAlignment::Stretch => 0.0,
                CrossAxisAlignment::End => (final_cross - cross).max(0.0),
                CrossAxisAlignment::Center => (final_cross - cross).max(0.0) * 0.5,
            };

            let rect = match self.axis {
                Axis::Horizontal => LogicalRect::from_xywh(current_main, cross_pos, main, cross),
                Axis::Vertical => LogicalRect::from_xywh(cross_pos, current_main, cross, main),
            };
            rects.push(rect);
            current_main += main + gap_space;
        }

        (container_size, rects)
    }
}

/// Pure stack layout (layers children atop each other).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StackLayout {
    /// Default child alignment within the stack.
    pub alignment: Alignment,
}

impl StackLayout {
    /// Pure measure and arrange of children in stack.
    pub fn layout(
        &self,
        constraints: Constraints,
        children: &[LogicalSize],
    ) -> (LogicalSize, Vec<LogicalRect>) {
        let mut max_w = 0.0f32;
        let mut max_h = 0.0f32;
        for size in children {
            max_w = max_w.max(size.width);
            max_h = max_h.max(size.height);
        }
        let final_size = constraints.constrain(LogicalSize::new(max_w, max_h));
        let rects = children
            .iter()
            .map(|&child_size| {
                let offset = self.alignment.align(final_size, child_size);
                LogicalRect::new(offset, child_size)
            })
            .collect();
        (final_size, rects)
    }
}

/// Pure padding layout.
pub struct PaddingLayout {
    /// Insets around the child.
    pub insets: LogicalInsets,
}

impl PaddingLayout {
    /// Pure measure and arrange of child in padded container.
    pub fn layout(
        &self,
        constraints: Constraints,
        child_size: Option<LogicalSize>,
    ) -> (LogicalSize, Option<LogicalRect>) {
        let child_constraints = constraints.deflate(self.insets);
        let actual_child = child_size.map(|s| child_constraints.constrain(s));
        let total_w = actual_child.map_or(0.0, |s| s.width) + self.insets.horizontal();
        let total_h = actual_child.map_or(0.0, |s| s.height) + self.insets.vertical();
        let final_size = constraints.constrain(LogicalSize::new(total_w, total_h));
        let child_rect = actual_child
            .map(|s| LogicalRect::new(LogicalPoint::new(self.insets.left, self.insets.top), s));
        (final_size, child_rect)
    }
}

/// Cache for node layout measurements and arrangements.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LayoutCache {
    last_constraints: Option<Constraints>,
    measured_size: Option<LogicalSize>,
    arranged_rects: Option<Vec<LogicalRect>>,
}

impl LayoutCache {
    /// Invalidate cache.
    pub fn clear(&mut self) {
        self.last_constraints = None;
        self.measured_size = None;
        self.arranged_rects = None;
    }

    /// Check if cached measurement is valid for given constraints and dirty state.
    pub fn is_valid(&self, constraints: Constraints, dirty: Dirty) -> bool {
        if dirty.contains(Dirty::LAYOUT) {
            return false;
        }
        self.last_constraints == Some(constraints) && self.measured_size.is_some()
    }

    /// Get cached size if valid.
    pub fn get(&self, constraints: Constraints, dirty: Dirty) -> Option<LogicalSize> {
        if self.is_valid(constraints, dirty) {
            self.measured_size
        } else {
            None
        }
    }

    /// Store a pure measurement result.
    pub fn store(&mut self, constraints: Constraints, size: LogicalSize) {
        self.last_constraints = Some(constraints);
        self.measured_size = Some(size);
    }

    /// Store arranged rects.
    pub fn store_rects(&mut self, rects: Vec<LogicalRect>) {
        self.arranged_rects = Some(rects);
    }

    /// Get arranged rects.
    pub fn rects(&self) -> Option<&[LogicalRect]> {
        self.arranged_rects.as_deref()
    }
}
