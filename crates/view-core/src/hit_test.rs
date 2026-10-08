//! Hit-testing contracts: clip/transform chains, paint-order traversal, and custom hit shapes.
//!
//! Traversal is in reverse paint order (front-to-back): later siblings before earlier
//! siblings, children before parent. Points outside accumulated clips are rejected.

use crate::{
    NodeId,
    geometry::{LogicalPoint, LogicalRect, Transform2D},
};

/// Accumulated clip rectangle in window logical space.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ClipChain {
    clip: Option<LogicalRect>,
}

impl ClipChain {
    /// No clipping.
    pub const NONE: Self = Self { clip: None };

    /// Create with an initial clip rect.
    pub fn new(clip: LogicalRect) -> Self {
        Self { clip: Some(clip) }
    }

    /// Intersect with an additional clip defined in local space, transformed by `transform`.
    pub fn intersect(&self, local_clip: Option<LogicalRect>, transform: &Transform2D) -> Self {
        match (self.clip, local_clip) {
            (None, None) => Self::NONE,
            (Some(c), None) => Self { clip: Some(c) },
            (None, Some(lc)) => Self {
                clip: Some(transform.transform_rect(lc)),
            },
            (Some(c), Some(lc)) => {
                let transformed_lc = transform.transform_rect(lc);
                Self {
                    clip: Some(
                        c.intersection(transformed_lc)
                            .unwrap_or(LogicalRect::from_xywh(0.0, 0.0, 0.0, 0.0)),
                    ),
                }
            }
        }
    }

    /// True if point is inside the accumulated clip (or if no clip is active).
    pub fn contains(&self, point: LogicalPoint) -> bool {
        match self.clip {
            Some(c) => c.size.width > 0.0 && c.size.height > 0.0 && c.contains(point),
            None => true,
        }
    }

    /// Active accumulated clip rectangle, if any.
    pub fn rect(&self) -> Option<LogicalRect> {
        self.clip
    }
}

/// Accumulated 2D transformation from window root to the current node.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TransformChain {
    accumulated: Transform2D,
}

impl TransformChain {
    /// Identity transform chain.
    pub const IDENTITY: Self = Self {
        accumulated: Transform2D::IDENTITY,
    };

    /// Create from a root transform.
    pub fn new(transform: Transform2D) -> Self {
        Self {
            accumulated: transform,
        }
    }

    /// Compose a child-local transform before the accumulated parent transform.
    pub fn then(&self, local: &Transform2D) -> Self {
        Self {
            accumulated: local.then(&self.accumulated),
        }
    }

    /// Accumulated transform.
    #[inline]
    pub fn transform(&self) -> Transform2D {
        self.accumulated
    }

    /// Map a point from window logical coordinates into local space.
    pub fn inverse_transform_point(&self, window_point: LogicalPoint) -> Option<LogicalPoint> {
        self.accumulated
            .inverse()
            .map(|inv| inv.transform_point(window_point))
    }
}

/// Custom hit-test behavior for custom widget geometry (e.g. shapes, holes, transparent pass-through).
pub trait HitTestContract {
    /// Return true if the local point lies within the hittable region of the node.
    fn hit_test(&self, local_point: LogicalPoint, bounds: LogicalRect) -> bool;
}

/// Default hit test: bounding box containment.
#[derive(Clone, Copy, Debug, Default)]
pub struct BoundingBoxHit;

impl HitTestContract for BoundingBoxHit {
    #[inline]
    fn hit_test(&self, local_point: LogicalPoint, bounds: LogicalRect) -> bool {
        bounds.contains(local_point)
    }
}

/// Transparent/pass-through: never hits this node, allows events to reach items underneath.
#[derive(Clone, Copy, Debug, Default)]
pub struct PassthroughHit;

impl HitTestContract for PassthroughHit {
    #[inline]
    fn hit_test(&self, _local_point: LogicalPoint, _bounds: LogicalRect) -> bool {
        false
    }
}

/// Elliptical/circular hit test within bounds.
#[derive(Clone, Copy, Debug, Default)]
pub struct EllipseHit;

impl HitTestContract for EllipseHit {
    fn hit_test(&self, local_point: LogicalPoint, bounds: LogicalRect) -> bool {
        if bounds.size.width <= 0.0 || bounds.size.height <= 0.0 {
            return false;
        }
        let center = bounds.center();
        let rx = bounds.size.width * 0.5;
        let ry = bounds.size.height * 0.5;
        let dx = (local_point.x - center.x) / rx;
        let dy = (local_point.y - center.y) / ry;
        dx * dx + dy * dy <= 1.0
    }
}

/// Successful hit outcome.
#[derive(Clone, Debug, PartialEq)]
pub struct HitTestResult {
    /// Innermost node hit.
    pub target: NodeId,
    /// Hit path from target up to window root.
    pub path: Vec<NodeId>,
    /// Point in target's local coordinate space.
    pub local_point: LogicalPoint,
}

/// Description of a positioned, transformed, and clipped node entry for hit testing.
pub struct HitNodeEntry<'a> {
    /// Node identity.
    pub id: NodeId,
    /// Arranged bounds in local parent coordinates.
    pub local_bounds: LogicalRect,
    /// Local transform.
    pub local_transform: Transform2D,
    /// Local clip rect, if any.
    pub local_clip: Option<LogicalRect>,
    /// Children in paint order (declaration order).
    pub children: &'a [HitNodeEntry<'a>],
    /// Custom hit contract.
    pub hit_contract: &'a dyn HitTestContract,
}

impl<'a> HitNodeEntry<'a> {
    /// Perform paint-order-aware hit test against this subtree.
    pub fn hit_test(
        &self,
        window_point: LogicalPoint,
        parent_clip: ClipChain,
        parent_transform: TransformChain,
    ) -> Option<HitTestResult> {
        let current_transform = parent_transform
            .then(&Transform2D::translation(
                self.local_bounds.origin.x,
                self.local_bounds.origin.y,
            ))
            .then(&self.local_transform);
        let current_clip = parent_clip.intersect(self.local_clip, &current_transform.transform());

        // 1. If point is outside accumulated clip, reject entire subtree.
        if !current_clip.contains(window_point) {
            return None;
        }

        // 2. Map point to this node's local coordinate space.
        let local_point = current_transform.inverse_transform_point(window_point)?;

        // 3. Test children in reverse paint order (front-to-back: later siblings first).
        for child in self.children.iter().rev() {
            if let Some(mut result) = child.hit_test(window_point, current_clip, current_transform)
            {
                result.path.push(self.id);
                return Some(result);
            }
        }

        // 4. If no child hit, test this node's own hit geometry.
        if self.hit_contract.hit_test(
            local_point,
            LogicalRect::new(LogicalPoint::ZERO, self.local_bounds.size),
        ) {
            Some(HitTestResult {
                target: self.id,
                path: vec![self.id],
                local_point,
            })
        } else {
            None
        }
    }
}
