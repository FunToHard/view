//! Strongly typed geometry, coordinate spaces, and 2D affine transforms.
//!
//! Screen coordinates remain strictly at the platform boundary; `view-core` defines
//! only logical (DIPs), physical (raster/device pixels), and document (world/canvas)
//! coordinate systems. All coordinates, sizes, and matrices require finite values.

use std::ops::{Add, Sub};

use crate::CoreError;

/// Numeric precision epsilon for coordinate equality and singularity checks.
pub const EPSILON: f32 = 1e-5;

/// Non-zero, finite display scale factor (e.g. 1.0, 1.25, 1.5, 2.0).
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct ScaleFactor(f32);

impl ScaleFactor {
    /// Baseline 100% scale factor (1.0).
    pub const ONE: Self = Self(1.0);

    /// Construct a scale factor from a finite positive float.
    pub fn new(factor: f32) -> Result<Self, CoreError> {
        if factor.is_finite() && factor > 0.0 {
            Ok(Self(factor))
        } else {
            Err(CoreError::InvalidGeometry)
        }
    }

    /// Inner raw factor.
    #[inline]
    pub const fn get(self) -> f32 {
        self.0
    }
}

impl Default for ScaleFactor {
    fn default() -> Self {
        Self::ONE
    }
}

/// Point in logical window coordinates (device-independent pixels).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LogicalPoint {
    /// X coordinate.
    pub x: f32,
    /// Y coordinate.
    pub y: f32,
}

impl LogicalPoint {
    /// Origin (0.0, 0.0).
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    /// Create a logical point.
    #[inline]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Create a validated logical point, ensuring coordinates are finite.
    pub fn new_checked(x: f32, y: f32) -> Result<Self, CoreError> {
        let p = Self { x, y };
        if p.is_valid() {
            Ok(p)
        } else {
            Err(CoreError::InvalidGeometry)
        }
    }

    /// True if both coordinates are finite numbers.
    #[inline]
    pub fn is_valid(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }

    /// Convert to physical point at the given scale factor.
    #[inline]
    pub fn to_physical(self, scale: ScaleFactor) -> PhysicalPoint {
        PhysicalPoint {
            x: self.x * scale.get(),
            y: self.y * scale.get(),
        }
    }

    /// Map to document space through the inverse of a 2D transform.
    pub fn to_document(self, transform: &Transform2D) -> Option<DocumentPoint> {
        transform.inverse().map(|inv| {
            let mapped = inv.transform_point(self);
            DocumentPoint {
                x: mapped.x,
                y: mapped.y,
            }
        })
    }
}

impl Add for LogicalPoint {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl Sub for LogicalPoint {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

/// Size in logical window coordinates (device-independent pixels).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LogicalSize {
    /// Width in logical units.
    pub width: f32,
    /// Height in logical units.
    pub height: f32,
}

impl LogicalSize {
    /// Zero size (0.0 x 0.0).
    pub const ZERO: Self = Self {
        width: 0.0,
        height: 0.0,
    };

    /// Create a logical size.
    #[inline]
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    /// Create a validated logical size, ensuring dimensions are finite and non-negative.
    pub fn new_checked(width: f32, height: f32) -> Result<Self, CoreError> {
        let s = Self { width, height };
        if s.is_valid() {
            Ok(s)
        } else {
            Err(CoreError::InvalidGeometry)
        }
    }

    /// True if dimensions are finite and non-negative.
    #[inline]
    pub fn is_valid(self) -> bool {
        self.width.is_finite() && self.height.is_finite() && self.width >= 0.0 && self.height >= 0.0
    }

    /// Convert to physical size at the given scale factor.
    #[inline]
    pub fn to_physical(self, scale: ScaleFactor) -> PhysicalSize {
        PhysicalSize {
            width: self.width * scale.get(),
            height: self.height * scale.get(),
        }
    }

    /// Clamp size dimensions between min and max bounds.
    #[inline]
    pub fn clamp(self, min: Self, max: Self) -> Self {
        Self {
            width: self.width.clamp(min.width, max.width),
            height: self.height.clamp(min.height, max.height),
        }
    }
}

/// Insets (margins/padding) in logical window coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LogicalInsets {
    /// Top inset.
    pub top: f32,
    /// Right inset.
    pub right: f32,
    /// Bottom inset.
    pub bottom: f32,
    /// Left inset.
    pub left: f32,
}

impl LogicalInsets {
    /// Zero insets.
    pub const ZERO: Self = Self {
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
    };

    /// Uniform insets on all sides.
    #[inline]
    pub const fn all(value: f32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    /// Symmetric vertical and horizontal insets.
    #[inline]
    pub const fn symmetric(vertical: f32, horizontal: f32) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }

    /// Explicit individual insets.
    #[inline]
    pub const fn new(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    /// True if all insets are finite and non-negative.
    #[inline]
    pub fn is_valid(self) -> bool {
        self.top.is_finite()
            && self.right.is_finite()
            && self.bottom.is_finite()
            && self.left.is_finite()
            && self.top >= 0.0
            && self.right >= 0.0
            && self.bottom >= 0.0
            && self.left >= 0.0
    }

    /// Combined horizontal insets (left + right).
    #[inline]
    pub fn horizontal(self) -> f32 {
        self.left + self.right
    }

    /// Combined vertical insets (top + bottom).
    #[inline]
    pub fn vertical(self) -> f32 {
        self.top + self.bottom
    }
}

/// Rectangle in logical window coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LogicalRect {
    /// Top-left origin.
    pub origin: LogicalPoint,
    /// Size.
    pub size: LogicalSize,
}

impl LogicalRect {
    /// Zero rect.
    pub const ZERO: Self = Self {
        origin: LogicalPoint::ZERO,
        size: LogicalSize::ZERO,
    };

    /// Create from origin and size.
    #[inline]
    pub const fn new(origin: LogicalPoint, size: LogicalSize) -> Self {
        Self { origin, size }
    }

    /// Create from (x, y, width, height).
    #[inline]
    pub const fn from_xywh(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            origin: LogicalPoint::new(x, y),
            size: LogicalSize::new(width, height),
        }
    }

    /// Create from (left, top, right, bottom).
    pub fn from_ltrb(left: f32, top: f32, right: f32, bottom: f32) -> Result<Self, CoreError> {
        if !left.is_finite() || !top.is_finite() || !right.is_finite() || !bottom.is_finite() {
            return Err(CoreError::InvalidGeometry);
        }
        if right < left || bottom < top {
            return Err(CoreError::InvalidGeometry);
        }
        Ok(Self {
            origin: LogicalPoint::new(left, top),
            size: LogicalSize::new(right - left, bottom - top),
        })
    }

    /// True if origin and size are finite and non-negative.
    #[inline]
    pub fn is_valid(self) -> bool {
        self.origin.is_valid() && self.size.is_valid()
    }

    /// Left edge.
    #[inline]
    pub fn left(self) -> f32 {
        self.origin.x
    }

    /// Top edge.
    #[inline]
    pub fn top(self) -> f32 {
        self.origin.y
    }

    /// Right edge.
    #[inline]
    pub fn right(self) -> f32 {
        self.origin.x + self.size.width
    }

    /// Bottom edge.
    #[inline]
    pub fn bottom(self) -> f32 {
        self.origin.y + self.size.height
    }

    /// Width.
    #[inline]
    pub fn width(self) -> f32 {
        self.size.width
    }

    /// Height.
    #[inline]
    pub fn height(self) -> f32 {
        self.size.height
    }

    /// Center point.
    #[inline]
    pub fn center(self) -> LogicalPoint {
        LogicalPoint {
            x: self.origin.x + self.size.width * 0.5,
            y: self.origin.y + self.size.height * 0.5,
        }
    }

    /// True if point lies within the rectangle (inclusive of edges).
    #[inline]
    pub fn contains(self, point: LogicalPoint) -> bool {
        point.x >= self.left()
            && point.x <= self.right()
            && point.y >= self.top()
            && point.y <= self.bottom()
    }

    /// True if this rect intersects another rect.
    #[inline]
    pub fn intersects(self, other: Self) -> bool {
        self.left() < other.right()
            && self.right() > other.left()
            && self.top() < other.bottom()
            && self.bottom() > other.top()
    }

    /// Compute intersection with another rect, if any.
    pub fn intersection(self, other: Self) -> Option<Self> {
        let left = self.left().max(other.left());
        let top = self.top().max(other.top());
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        if right >= left && bottom >= top {
            Some(Self {
                origin: LogicalPoint::new(left, top),
                size: LogicalSize::new(right - left, bottom - top),
            })
        } else {
            None
        }
    }

    /// Smallest bounding box enclosing both rects.
    pub fn union(self, other: Self) -> Self {
        let left = self.left().min(other.left());
        let top = self.top().min(other.top());
        let right = self.right().max(other.right());
        let bottom = self.bottom().max(other.bottom());
        Self {
            origin: LogicalPoint::new(left, top),
            size: LogicalSize::new(right - left, bottom - top),
        }
    }

    /// Deflate rectangle by insets. Clamps size to zero if insets exceed rect dimensions.
    pub fn deflate(self, insets: LogicalInsets) -> Self {
        let x = self.origin.x + insets.left;
        let y = self.origin.y + insets.top;
        let width = (self.size.width - insets.horizontal()).max(0.0);
        let height = (self.size.height - insets.vertical()).max(0.0);
        Self {
            origin: LogicalPoint::new(x, y),
            size: LogicalSize::new(width, height),
        }
    }

    /// Inflate rectangle by insets.
    pub fn inflate(self, insets: LogicalInsets) -> Self {
        Self {
            origin: LogicalPoint::new(self.origin.x - insets.left, self.origin.y - insets.top),
            size: LogicalSize::new(
                self.size.width + insets.horizontal(),
                self.size.height + insets.vertical(),
            ),
        }
    }

    /// Convert to physical rect at the given scale factor.
    #[inline]
    pub fn to_physical(self, scale: ScaleFactor) -> PhysicalRect {
        PhysicalRect {
            origin: self.origin.to_physical(scale),
            size: self.size.to_physical(scale),
        }
    }
}

/// Point in physical device pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PhysicalPoint {
    /// X in physical pixels.
    pub x: f32,
    /// Y in physical pixels.
    pub y: f32,
}

impl PhysicalPoint {
    /// Origin (0.0, 0.0).
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    /// Create a physical point.
    #[inline]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Convert to logical coordinates using the given scale factor.
    #[inline]
    pub fn to_logical(self, scale: ScaleFactor) -> LogicalPoint {
        LogicalPoint {
            x: self.x / scale.get(),
            y: self.y / scale.get(),
        }
    }
}

/// Size in physical device pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PhysicalSize {
    /// Width in physical pixels.
    pub width: f32,
    /// Height in physical pixels.
    pub height: f32,
}

impl PhysicalSize {
    /// Zero size.
    pub const ZERO: Self = Self {
        width: 0.0,
        height: 0.0,
    };

    /// Create a physical size.
    #[inline]
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    /// Create from integer pixel counts.
    #[inline]
    pub const fn from_pixels(width: u32, height: u32) -> Self {
        Self {
            width: width as f32,
            height: height as f32,
        }
    }

    /// Convert to logical size at the given scale factor.
    #[inline]
    pub fn to_logical(self, scale: ScaleFactor) -> LogicalSize {
        LogicalSize {
            width: self.width / scale.get(),
            height: self.height / scale.get(),
        }
    }
}

/// Rectangle in physical device pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PhysicalRect {
    /// Top-left origin in physical pixels.
    pub origin: PhysicalPoint,
    /// Size in physical pixels.
    pub size: PhysicalSize,
}

impl PhysicalRect {
    /// Convert to logical rect at the given scale factor.
    #[inline]
    pub fn to_logical(self, scale: ScaleFactor) -> LogicalRect {
        LogicalRect {
            origin: self.origin.to_logical(scale),
            size: self.size.to_logical(scale),
        }
    }
}

/// Point in document/world space (independent of view zooming/panning).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DocumentPoint {
    /// X in document space.
    pub x: f32,
    /// Y in document space.
    pub y: f32,
}

impl DocumentPoint {
    /// Origin.
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    /// Create a document point.
    #[inline]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Map to logical window coordinates using a transform.
    pub fn to_logical(self, transform: &Transform2D) -> LogicalPoint {
        transform.transform_point(LogicalPoint::new(self.x, self.y))
    }
}

/// Size in document/world space.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DocumentSize {
    /// Width in document space.
    pub width: f32,
    /// Height in document space.
    pub height: f32,
}

/// Rectangle in document/world space.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DocumentRect {
    /// Origin in document space.
    pub origin: DocumentPoint,
    /// Size in document space.
    pub size: DocumentSize,
}

/// 2D affine transformation matrix: `[a, b, c, d, tx, ty]`.
///
/// Maps `(x, y)` to:
/// `x' = a * x + c * y + tx`
/// `y' = b * x + d * y + ty`
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform2D {
    matrix: [f32; 6],
}

impl Transform2D {
    /// Identity matrix.
    pub const IDENTITY: Self = Self {
        matrix: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
    };

    /// Create from raw matrix elements `[a, b, c, d, tx, ty]`.
    pub fn new(matrix: [f32; 6]) -> Result<Self, CoreError> {
        let t = Self { matrix };
        if t.is_valid() {
            Ok(t)
        } else {
            Err(CoreError::InvalidGeometry)
        }
    }

    /// Translation transform.
    pub fn translation(tx: f32, ty: f32) -> Self {
        Self {
            matrix: [1.0, 0.0, 0.0, 1.0, tx, ty],
        }
    }

    /// Scale transform.
    pub fn scale(sx: f32, sy: f32) -> Self {
        Self {
            matrix: [sx, 0.0, 0.0, sy, 0.0, 0.0],
        }
    }

    /// Rotation transform by angle in radians.
    pub fn rotation(radians: f32) -> Self {
        let cos = radians.cos();
        let sin = radians.sin();
        Self {
            matrix: [cos, sin, -sin, cos, 0.0, 0.0],
        }
    }

    /// True if all matrix coefficients are finite.
    #[inline]
    pub fn is_valid(self) -> bool {
        self.matrix.iter().all(|val| val.is_finite())
    }

    /// Raw matrix coefficients `[a, b, c, d, tx, ty]`.
    #[inline]
    pub const fn matrix(self) -> [f32; 6] {
        self.matrix
    }

    /// Determinant: `a * d - b * c`.
    #[inline]
    pub fn determinant(self) -> f32 {
        self.matrix[0] * self.matrix[3] - self.matrix[1] * self.matrix[2]
    }

    /// Compose transforms: `self.then(&other)` applies `self`, then `other`.
    pub fn then(&self, other: &Self) -> Self {
        let [a1, b1, c1, d1, tx1, ty1] = self.matrix;
        let [a2, b2, c2, d2, tx2, ty2] = other.matrix;
        Self {
            matrix: [
                a2 * a1 + c2 * b1,
                b2 * a1 + d2 * b1,
                a2 * c1 + c2 * d1,
                b2 * c1 + d2 * d1,
                a2 * tx1 + c2 * ty1 + tx2,
                b2 * tx1 + d2 * ty1 + ty2,
            ],
        }
    }

    /// Invert matrix if determinant is non-zero and finite.
    pub fn inverse(&self) -> Option<Self> {
        let det = self.determinant();
        if !det.is_finite() || det.abs() < EPSILON {
            return None;
        }
        let inv_det = 1.0 / det;
        let [a, b, c, d, tx, ty] = self.matrix;
        let inv_a = d * inv_det;
        let inv_b = -b * inv_det;
        let inv_c = -c * inv_det;
        let inv_d = a * inv_det;
        let inv_tx = (c * ty - d * tx) * inv_det;
        let inv_ty = (b * tx - a * ty) * inv_det;
        let result = Self {
            matrix: [inv_a, inv_b, inv_c, inv_d, inv_tx, inv_ty],
        };
        if result.is_valid() {
            Some(result)
        } else {
            None
        }
    }

    /// Transform a logical point: `(x', y')`.
    #[inline]
    pub fn transform_point(&self, point: LogicalPoint) -> LogicalPoint {
        let [a, b, c, d, tx, ty] = self.matrix;
        LogicalPoint {
            x: a * point.x + c * point.y + tx,
            y: b * point.x + d * point.y + ty,
        }
    }

    /// Transform a bounding box by transforming all 4 corners and returning the enclosing AABB.
    pub fn transform_rect(&self, rect: LogicalRect) -> LogicalRect {
        let p1 = self.transform_point(LogicalPoint::new(rect.left(), rect.top()));
        let p2 = self.transform_point(LogicalPoint::new(rect.right(), rect.top()));
        let p3 = self.transform_point(LogicalPoint::new(rect.right(), rect.bottom()));
        let p4 = self.transform_point(LogicalPoint::new(rect.left(), rect.bottom()));

        let min_x = p1.x.min(p2.x).min(p3.x).min(p4.x);
        let max_x = p1.x.max(p2.x).max(p3.x).max(p4.x);
        let min_y = p1.y.min(p2.y).min(p3.y).min(p4.y);
        let max_y = p1.y.max(p2.y).max(p3.y).max(p4.y);

        LogicalRect {
            origin: LogicalPoint::new(min_x, min_y),
            size: LogicalSize::new(max_x - min_x, max_y - min_y),
        }
    }
}

impl Default for Transform2D {
    fn default() -> Self {
        Self::IDENTITY
    }
}
