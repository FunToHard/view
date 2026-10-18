//! Backend-independent canvas snapshots. Documents and action effects remain app-owned.
//! Coordinates are f64 until rebased into viewport-local logical f32 geometry.

use std::collections::BTreeSet;

use crate::{CoreError, LogicalPoint, LogicalRect, LogicalSize, Revision, Role, SemanticAction};

/// High precision document coordinate; distinct from legacy f32 `DocumentPoint`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CanvasPoint {
    /// Document X (positive right).
    pub x: f64,
    /// Document Y (positive down).
    pub y: f64,
}

impl CanvasPoint {
    /// Construct a finite document position.
    pub fn new(x: f64, y: f64) -> Result<Self, CoreError> {
        if x.is_finite() && y.is_finite() {
            Ok(Self { x, y })
        } else {
            Err(CoreError::InvalidGeometry)
        }
    }
}

/// Axis-aligned half-open document bounds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasRect {
    /// Inclusive minimum corner.
    pub min: CanvasPoint,
    /// Exclusive maximum corner.
    pub max: CanvasPoint,
}

impl CanvasRect {
    /// Validate finite ordered corners. Empty rectangles are allowed.
    pub fn new(min: CanvasPoint, max: CanvasPoint) -> Result<Self, CoreError> {
        CanvasPoint::new(min.x, min.y)?;
        CanvasPoint::new(max.x, max.y)?;
        if min.x > max.x || min.y > max.y {
            return Err(CoreError::InvalidGeometry);
        }
        Ok(Self { min, max })
    }

    fn contains(self, p: CanvasPoint) -> bool {
        p.x >= self.min.x && p.y >= self.min.y && p.x < self.max.x && p.y < self.max.y
    }
}

/// Pan/zoom camera mapping document coordinates to viewport-local logical units.
/// DPI scaling and window placement happen after this mapping.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasView {
    origin: CanvasPoint,
    zoom: f64,
    size: LogicalSize,
}

impl CanvasView {
    /// Construct with a document origin, positive zoom and finite nonnegative size.
    pub fn new(origin: CanvasPoint, zoom: f64, size: LogicalSize) -> Result<Self, CoreError> {
        CanvasPoint::new(origin.x, origin.y)?;
        if !zoom.is_finite() || zoom <= 0.0 || !size.is_valid() {
            return Err(CoreError::InvalidGeometry);
        }
        let view = Self { origin, zoom, size };
        view.document_at(LogicalPoint::new(size.width, size.height))?;
        Ok(view)
    }

    /// Document point at the viewport's top-left.
    pub fn origin(self) -> CanvasPoint {
        self.origin
    }

    /// Logical units per document unit.
    pub fn zoom(self) -> f64 {
        self.zoom
    }

    /// Logical viewport extent; zero suspends hit and semantic regions.
    pub fn size(self) -> LogicalSize {
        self.size
    }

    /// Convert input to document space. Points outside the viewport are allowed
    /// for captured drags; hit testing separately applies the viewport clip.
    pub fn document_at(self, point: LogicalPoint) -> Result<CanvasPoint, CoreError> {
        if !point.is_valid() {
            return Err(CoreError::InvalidGeometry);
        }
        CanvasPoint::new(
            self.origin.x + f64::from(point.x) / self.zoom,
            self.origin.y + f64::from(point.y) / self.zoom,
        )
    }

    /// Rebase in f64 before conversion. Reject overflow or rounding exceeding
    /// 1/256 logical unit on either axis; callers may clip/tile far-away content.
    pub fn project(self, point: CanvasPoint) -> Result<LogicalPoint, CoreError> {
        CanvasPoint::new(point.x, point.y)?;
        fn narrow(value: f64) -> Result<f32, CoreError> {
            let result = value as f32;
            if !result.is_finite() || (f64::from(result) - value).abs() > 1.0 / 256.0 {
                Err(CoreError::InvalidGeometry)
            } else {
                Ok(result)
            }
        }
        Ok(LogicalPoint::new(
            narrow((point.x - self.origin.x) * self.zoom)?,
            narrow((point.y - self.origin.y) * self.zoom)?,
        ))
    }

    /// Move visible content by a logical delta; failed changes are atomic.
    pub fn pan(&mut self, delta: LogicalPoint) -> Result<(), CoreError> {
        if !delta.is_valid() {
            return Err(CoreError::InvalidGeometry);
        }
        *self = Self::new(
            CanvasPoint::new(
                self.origin.x - f64::from(delta.x) / self.zoom,
                self.origin.y - f64::from(delta.y) / self.zoom,
            )?,
            self.zoom,
            self.size,
        )?;
        Ok(())
    }

    /// Change zoom while preserving the document point under the logical anchor.
    pub fn zoom_at(&mut self, anchor: LogicalPoint, zoom: f64) -> Result<(), CoreError> {
        let point = self.document_at(anchor)?;
        let origin = CanvasPoint::new(
            point.x - f64::from(anchor.x) / zoom,
            point.y - f64::from(anchor.y) / zoom,
        )?;
        *self = Self::new(origin, zoom, self.size)?;
        Ok(())
    }

    /// Resize without changing document origin or zoom.
    pub fn resize(&mut self, size: LogicalSize) -> Result<(), CoreError> {
        *self = Self::new(self.origin, self.zoom, size)?;
        Ok(())
    }

    fn clipped(self, bounds: CanvasRect) -> Result<Option<LogicalRect>, CoreError> {
        let end = self.document_at(LogicalPoint::new(self.size.width, self.size.height))?;
        let min = CanvasPoint {
            x: bounds.min.x.max(self.origin.x),
            y: bounds.min.y.max(self.origin.y),
        };
        let max = CanvasPoint {
            x: bounds.max.x.min(end.x),
            y: bounds.max.y.min(end.y),
        };
        if min.x >= max.x || min.y >= max.y {
            return Ok(None);
        }
        let a = self.project(min)?;
        let b = self.project(max)?;
        Ok(Some(LogicalRect::new(
            a,
            LogicalSize::new(b.x - a.x, b.y - a.y),
        )))
    }
}

/// App-owned stable document identity, scoped to one canvas (not a runtime node).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CanvasId(pub u64);

/// Named object metadata for translation into runtime-owned semantic nodes.
#[derive(Clone, Debug, PartialEq)]
pub struct CanvasSemantics {
    /// Accessible object name.
    pub name: String,
    /// Declared role.
    pub role: Role,
    /// Supported actions; execution remains in the application's action queue.
    pub actions: Vec<SemanticAction>,
}

/// One ordered retained draw payload and its document-space interaction region.
/// `D` may be an application primitive or renderer record; core never encodes it.
#[derive(Clone, Debug, PartialEq)]
pub struct CanvasRecord<D> {
    /// Stable app-owned identity, unique in a snapshot.
    pub id: CanvasId,
    /// Geometry shared by rectangular hits and semantic bounds.
    pub bounds: CanvasRect,
    /// Whether this record participates in pointer targeting.
    pub hit_enabled: bool,
    /// Optional named semantic region, independent of pointer targeting.
    pub semantics: Option<CanvasSemantics>,
    /// App-owned drawing description in document coordinates.
    pub draw: D,
}

/// Visible semantic region tagged with the canvas revision that produced it.
#[derive(Clone, Debug, PartialEq)]
pub struct CanvasSemanticRegion {
    /// App-owned object identity.
    pub id: CanvasId,
    /// Snapshot revision; validate before dispatching an action.
    pub revision: Revision,
    /// Clipped viewport-local logical bounds.
    pub bounds: LogicalRect,
    /// Declared semantic behavior.
    pub semantics: CanvasSemantics,
}

/// Coherent bounded canvas description. Mutations publish a new local revision;
/// skipped on-demand builds retain records and revision without running effects.
#[derive(Clone, Debug)]
pub struct Canvas2D<D> {
    view: CanvasView,
    records: Vec<CanvasRecord<D>>,
    revision: Revision,
    limit: usize,
}

impl<D> Canvas2D<D> {
    /// Create an empty canvas with an explicit record-count budget.
    pub fn new(view: CanvasView, limit: usize) -> Self {
        Self {
            view,
            records: Vec::new(),
            revision: Revision::INITIAL,
            limit,
        }
    }

    /// Current coherent snapshot revision, local to this canvas.
    pub fn revision(&self) -> Revision {
        self.revision
    }

    /// Current view mapping, also used for drawing.
    pub fn view(&self) -> CanvasView {
        self.view
    }

    /// Records in back-to-front drawing order.
    pub fn records(&self) -> &[CanvasRecord<D>] {
        &self.records
    }

    /// Publish a pan, zoom or resize without rebuilding document records.
    pub fn set_view(&mut self, view: CanvasView) -> Result<Revision, CoreError> {
        if view != self.view {
            let next = self.revision.checked_next()?;
            self.view = view;
            self.revision = next;
        }
        Ok(self.revision)
    }

    /// Replace records atomically. `None` is a skipped on-demand build; an empty
    /// `Some` explicitly clears the canvas. Both authoring modes share this state.
    pub fn update(&mut self, records: Option<Vec<CanvasRecord<D>>>) -> Result<Revision, CoreError> {
        let Some(records) = records else {
            return Ok(self.revision);
        };
        if records.len() > self.limit {
            return Err(CoreError::CapacityExhausted);
        }
        let mut ids = BTreeSet::new();
        for record in &records {
            CanvasRect::new(record.bounds.min, record.bounds.max)?;
            if !ids.insert(record.id) {
                return Err(CoreError::DuplicateKey);
            }
        }
        let next = self.revision.checked_next()?;
        self.records = records;
        self.revision = next;
        Ok(next)
    }

    /// Target the last painted enabled rectangle against this snapshot's camera.
    /// Pass only viewport-local input routed to the owning runtime canvas node.
    pub fn hit_test(&self, point: LogicalPoint) -> Result<Option<CanvasId>, CoreError> {
        let document = self.view.document_at(point)?;
        if point.x < 0.0
            || point.y < 0.0
            || point.x >= self.view.size.width
            || point.y >= self.view.size.height
        {
            return Ok(None);
        }
        Ok(self
            .records
            .iter()
            .rev()
            .find(|r| r.hit_enabled && r.bounds.contains(document))
            .map(|r| r.id))
    }

    /// Clip/project named regions using the same camera as input and drawing.
    /// The adapter supplies runtime node identity, window placement and lifecycle.
    pub fn semantic_regions(&self) -> Result<Vec<CanvasSemanticRegion>, CoreError> {
        let mut regions = Vec::new();
        for record in &self.records {
            if let Some(semantics) = &record.semantics
                && let Some(bounds) = self.view.clipped(record.bounds)?
            {
                regions.push(CanvasSemanticRegion {
                    id: record.id,
                    revision: self.revision,
                    bounds,
                    semantics: semantics.clone(),
                });
            }
        }
        Ok(regions)
    }
}
