//! CPU triangle picking and asynchronous-result authority, separate from UI focus.
use super::*;
use view_core::{Arena, ArenaId, LogicalPoint, LogicalRect, LogicalSize};

/// Revisions identifying the exact scene/camera/viewport used by a query.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PickStamp {
    /// Scene snapshot revision.
    pub scene: Revision,
    /// Camera revision in the application's camera stream.
    pub camera: Revision,
    /// Viewport geometry/DPI revision in the application's viewport stream.
    pub viewport: Revision,
}
/// Immutable worker request; only PickTracker can issue its identity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PickRequest {
    namespace: ArenaId,
    sequence: Revision,
    stamp: PickStamp,
    point: LogicalPoint,
    size: LogicalSize,
}
/// Closest triangle hit, in world units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PickHit {
    /// App object identity.
    pub object: u64,
    /// Triangle index within the mesh.
    pub triangle: usize,
    /// World point on that triangle.
    pub position: [f64; 3],
    /// Distance along the normalized ray from the camera eye.
    pub distance: f64,
}
/// Worker result retains the complete request, including ownership and revisions.
#[derive(Clone, Debug)]
pub struct PickResult {
    request: PickRequest,
    hit: Option<PickHit>,
}
/// Latest-request gate. Dropping/replacing this owner invalidates all old results.
pub struct PickTracker {
    namespace: ArenaId,
    sequence: Revision,
    pending: Option<PickRequest>,
}
impl PickTracker {
    /// Allocate a process-unique request namespace, without a worker or executor.
    pub fn new() -> Result<Self, view_core::CoreError> {
        Ok(Self {
            namespace: Arena::<()>::new()?.id(),
            sequence: Revision::INITIAL,
            pending: None,
        })
    }
    /// Issue a replaceable query in viewport-local logical coordinates.
    pub fn request(
        &mut self,
        stamp: PickStamp,
        point: LogicalPoint,
        size: LogicalSize,
    ) -> Result<PickRequest, RenderError> {
        if !point.is_valid() || !size.is_valid() || size.width <= 0.0 || size.height <= 0.0 {
            return Err(invalid("invalid pick viewport"));
        }
        let sequence = self
            .sequence
            .checked_next()
            .map_err(|e| invalid(&e.to_string()))?;
        let request = PickRequest {
            namespace: self.namespace,
            sequence,
            stamp,
            point,
            size,
        };
        self.sequence = sequence;
        self.pending = Some(request);
        Ok(request)
    }
    /// Revoke a pending pick on gesture start, selection change or owner removal.
    pub fn cancel(&mut self) {
        self.pending = None;
    }
    /// Accept once only if request identity and all current revisions still match.
    /// Outer None means stale; Some(None) is an authoritative empty hit.
    pub fn accept(&mut self, result: PickResult, current: PickStamp) -> Option<Option<PickHit>> {
        if self.pending != Some(result.request) || result.request.stamp != current {
            return None;
        }
        self.pending = None;
        Some(result.hit)
    }
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| a[i] * b[i]).sum()
}
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

impl SceneSnapshot {
    /// Deterministic double-sided triangle query, safe to execute against this
    /// immutable snapshot on a worker. No readback or scene/UI mutation occurs.
    pub fn pick(&self, request: PickRequest) -> Result<PickResult, RenderError> {
        if request.stamp.scene != self.revision {
            return Err(invalid("pick scene revision mismatch"));
        }
        let p = request.point;
        let size = request.size;
        if p.x < 0.0 || p.y < 0.0 || p.x >= size.width || p.y >= size.height {
            return Ok(PickResult { request, hit: None });
        }
        let aspect = f64::from(size.width) / f64::from(size.height);
        let tangent = (f64::from(self.camera.vertical_fov) * 0.5).tan();
        let local = [
            (2.0 * f64::from(p.x) / f64::from(size.width) - 1.0) * aspect * tangent,
            (1.0 - 2.0 * f64::from(p.y) / f64::from(size.height)) * tangent,
            -1.0,
        ];
        let view = self.camera.view.0;
        let eye = std::array::from_fn(|i| {
            -(0..3)
                .map(|k| f64::from(view[i][k]) * f64::from(view[3][k]))
                .sum::<f64>()
        });
        let direction: [f64; 3] =
            std::array::from_fn(|i| (0..3).map(|k| f64::from(view[i][k]) * local[k]).sum());
        let length = dot(direction, direction).sqrt();
        let direction = direction.map(|v| v / length);
        let vp = self.camera.view_projection(aspect as f32)?;
        let mut hit: Option<PickHit> = None;
        for object in &self.objects {
            for (triangle, indices) in object.mesh.indices.as_chunks::<3>().0.iter().enumerate() {
                let mut points = [[0.0; 3]; 3];
                for i in 0..3 {
                    let p = object.mesh.positions[indices[i] as usize];
                    let world = object
                        .transform
                        .matrix()
                        .transform([p[0], p[1], p[2], 1.0])?;
                    points[i] = [
                        f64::from(world[0]),
                        f64::from(world[1]),
                        f64::from(world[2]),
                    ];
                }
                let a = sub(points[1], points[0]);
                let b = sub(points[2], points[0]);
                let h = cross(direction, b);
                let det = dot(a, h);
                // Scale-relative degeneracy rejection, allowing small world units.
                if det.abs() <= f64::EPSILON * dot(a, a).sqrt() * dot(b, b).sqrt() * 8.0 {
                    continue;
                }
                let s = sub(eye, points[0]);
                let u = dot(s, h) / det;
                if !(0.0..=1.0).contains(&u) {
                    continue;
                }
                let q = cross(s, a);
                let v = dot(direction, q) / det;
                if v < 0.0 || u + v > 1.0 {
                    continue;
                }
                let distance = dot(b, q) / det;
                if distance < 0.0 || hit.is_some_and(|old| old.distance <= distance) {
                    continue;
                }
                let position: [f64; 3] = std::array::from_fn(|i| eye[i] + direction[i] * distance);
                let clip = vp.transform([
                    position[0] as f32,
                    position[1] as f32,
                    position[2] as f32,
                    1.0,
                ])?;
                if clip[3] <= 0.0 || clip[2] < 0.0 || clip[2] >= clip[3] {
                    continue;
                }
                hit = Some(PickHit {
                    object: object.id,
                    triangle,
                    position,
                    distance,
                });
            }
        }
        Ok(PickResult { request, hit })
    }
    /// Project a world-space gizmo anchor into a logical viewport. Off-frustum
    /// anchors return None. UI overlays still use the normal painter and router.
    pub fn project_overlay(
        &self,
        world: [f32; 3],
        viewport: LogicalRect,
    ) -> Result<Option<LogicalPoint>, RenderError> {
        if !viewport.is_valid() || viewport.size.width <= 0.0 || viewport.size.height <= 0.0 {
            return Err(invalid("invalid overlay viewport"));
        }
        let p = self
            .camera
            .view_projection(viewport.size.width / viewport.size.height)?
            .transform([world[0], world[1], world[2], 1.0])?;
        if p[3] <= 0.0 || p[2] < 0.0 || p[2] >= p[3] || p[0].abs() > p[3] || p[1].abs() > p[3] {
            return Ok(None);
        }
        Ok(Some(LogicalPoint::new(
            viewport.origin.x + (p[0] / p[3] + 1.0) * 0.5 * viewport.size.width,
            viewport.origin.y + (1.0 - p[1] / p[3]) * 0.5 * viewport.size.height,
        )))
    }
}

/// Named actions are independent of pixel picking and UI keyboard focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SceneAction {
    /// Change app-owned selection.
    Select,
    /// Ask the application to frame this object with its camera.
    Frame,
}
/// Declarative publication of supported commands for one app object.
#[derive(Clone, Debug)]
pub struct NamedObject {
    /// ID from the scene snapshot.
    pub id: u64,
    /// Unique human/automation name.
    pub name: String,
    /// Allowlisted actions.
    pub actions: Vec<SceneAction>,
}
/// Validated command, to enqueue through the application's typed action path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SceneCommand {
    /// Target app object.
    pub object: u64,
    /// Requested action.
    pub action: SceneAction,
    /// Revalidate before execution if document state has changed.
    pub revision: Revision,
}
/// Revision-scoped command catalog; contains no callbacks or document authority.
pub struct SceneCatalog {
    revision: Revision,
    objects: Vec<NamedObject>,
}
impl SceneCatalog {
    /// Reject unknown IDs, duplicate IDs/names and empty names atomically.
    pub fn new(snapshot: &SceneSnapshot, objects: Vec<NamedObject>) -> Result<Self, RenderError> {
        let mut ids = BTreeSet::new();
        let mut names = BTreeSet::new();
        for o in &objects {
            if o.name.trim().is_empty()
                || !ids.insert(o.id)
                || !names.insert(o.name.clone())
                || !snapshot.objects.iter().any(|s| s.id == o.id)
            {
                return Err(invalid("invalid named scene object"));
            }
        }
        Ok(Self {
            revision: snapshot.revision,
            objects,
        })
    }
    /// Resolve without pointer coordinates; reject stale/unsupported commands.
    pub fn command(
        &self,
        name: &str,
        action: SceneAction,
        current: Revision,
    ) -> Result<SceneCommand, RenderError> {
        if current != self.revision {
            return Err(invalid("stale scene catalog"));
        }
        let o = self
            .objects
            .iter()
            .find(|o| o.name == name && o.actions.contains(&action))
            .ok_or_else(|| invalid("unavailable scene action"))?;
        Ok(SceneCommand {
            object: o.id,
            action,
            revision: self.revision,
        })
    }
}
