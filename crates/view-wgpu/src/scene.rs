//! Minimal opaque 3D rendering, with no scene graph or document ownership.
//!
//! Right-handed coordinates, +Y up, camera forward -Z, column-major matrices
//! acting on column vectors. Perspective depth is 0 at near and 1 at far.
//! World units are application-defined; positions are f32 (rebase large worlds).

use std::{collections::BTreeSet, sync::Arc};
use view_core::Revision;
use wgpu::util::DeviceExt;

mod assets;
mod picking;
pub use assets::*;
pub use picking::*;

use crate::{
    AttachmentContract, ContributionMode, DeviceGeneration, GpuContext, RenderError, RenderProvider,
};

fn invalid(message: &str) -> RenderError {
    RenderError::Invalid(message.into())
}

/// Column-major finite 4x4 matrix. Composition uses `left * right`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Matrix4([[f32; 4]; 4]);

impl Matrix4 {
    /// Identity matrix.
    pub const IDENTITY: Self = Self([
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]);
    fn checked(columns: [[f32; 4]; 4]) -> Result<Self, RenderError> {
        if columns.iter().flatten().all(|v| v.is_finite()) {
            Ok(Self(columns))
        } else {
            Err(invalid("non-finite scene matrix"))
        }
    }
    /// Raw column-major data, suitable for WGSL mat4x4.
    pub fn columns(self) -> [[f32; 4]; 4] {
        self.0
    }
    /// Multiply matrices, applying `right` first; reject arithmetic overflow.
    pub fn multiply(self, right: Self) -> Result<Self, RenderError> {
        let mut result = [[0.0; 4]; 4];
        for (col, values) in result.iter_mut().enumerate() {
            for (row, value) in values.iter_mut().enumerate() {
                *value = (0..4)
                    .map(|k| f64::from(self.0[k][row]) * f64::from(right.0[col][k]))
                    .sum::<f64>() as f32;
            }
        }
        Self::checked(result)
    }
    /// Transform a homogeneous vector without perspective division.
    pub fn transform(self, vector: [f32; 4]) -> Result<[f32; 4], RenderError> {
        if !vector.iter().all(|v| v.is_finite()) {
            return Err(invalid("non-finite scene point"));
        }
        let mut result = [0.0; 4];
        for (row, value) in result.iter_mut().enumerate() {
            let magnitude = (0..4)
                .map(|k| (f64::from(self.0[k][row]) * f64::from(vector[k])).abs())
                .sum::<f64>();
            if magnitude > f64::from(f32::MAX) {
                return Err(invalid("scene point intermediate overflow"));
            }
            *value = (0..4)
                .map(|k| f64::from(self.0[k][row]) * f64::from(vector[k]))
                .sum::<f64>() as f32;
        }
        if result.iter().all(|v| v.is_finite()) {
            Ok(result)
        } else {
            Err(invalid("scene point overflow"))
        }
    }
}

/// Finite affine model transform. Degenerate scales are allowed (invisible geometry).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform3D(Matrix4);
impl Transform3D {
    /// No model transformation.
    pub const IDENTITY: Self = Self(Matrix4::IDENTITY);
    /// Construct an affine matrix; perspective belongs to the camera only.
    pub fn from_columns(columns: [[f32; 4]; 4]) -> Result<Self, RenderError> {
        let matrix = Matrix4::checked(columns)?;
        if [columns[0][3], columns[1][3], columns[2][3], columns[3][3]] != [0.0, 0.0, 0.0, 1.0] {
            return Err(invalid("model matrix must be affine"));
        }
        Ok(Self(matrix))
    }
    /// Translation in application world units.
    pub fn translation(offset: [f32; 3]) -> Result<Self, RenderError> {
        let mut columns = Matrix4::IDENTITY.0;
        columns[3] = [offset[0], offset[1], offset[2], 1.0];
        Self::from_columns(columns)
    }
    /// Translation component of this affine transform.
    pub fn translation_part(self) -> [f32; 3] {
        [self.0.0[3][0], self.0.0[3][1], self.0.0[3][2]]
    }
    /// Replace translation while preserving this transform's linear component.
    pub fn with_translation(self, offset: [f32; 3]) -> Result<Self, RenderError> {
        let mut columns = self.0.0;
        columns[3] = [offset[0], offset[1], offset[2], 1.0];
        Self::from_columns(columns)
    }
    fn normal_matrix(self) -> Result<[[f32; 4]; 3], RenderError> {
        let matrix = self.0.0;
        let a = [
            [
                f64::from(matrix[0][0]),
                f64::from(matrix[1][0]),
                f64::from(matrix[2][0]),
            ],
            [
                f64::from(matrix[0][1]),
                f64::from(matrix[1][1]),
                f64::from(matrix[2][1]),
            ],
            [
                f64::from(matrix[0][2]),
                f64::from(matrix[1][2]),
                f64::from(matrix[2][2]),
            ],
        ];
        let cofactors = [
            [
                a[1][1] * a[2][2] - a[1][2] * a[2][1],
                a[1][2] * a[2][0] - a[1][0] * a[2][2],
                a[1][0] * a[2][1] - a[1][1] * a[2][0],
            ],
            [
                a[0][2] * a[2][1] - a[0][1] * a[2][2],
                a[0][0] * a[2][2] - a[0][2] * a[2][0],
                a[0][1] * a[2][0] - a[0][0] * a[2][1],
            ],
            [
                a[0][1] * a[1][2] - a[0][2] * a[1][1],
                a[0][2] * a[1][0] - a[0][0] * a[1][2],
                a[0][0] * a[1][1] - a[0][1] * a[1][0],
            ],
        ];
        let determinant =
            a[0][0] * cofactors[0][0] + a[0][1] * cofactors[0][1] + a[0][2] * cofactors[0][2];
        if !determinant.is_finite() || determinant.abs() <= 1.0e-20 {
            return Err(invalid("lit scene object has a singular transform"));
        }
        let mut columns = [[0.0; 4]; 3];
        for col in 0..3 {
            for row in 0..3 {
                let value = cofactors[row][col] / determinant;
                if !value.is_finite() || value.abs() > f64::from(f32::MAX) {
                    return Err(invalid("scene normal matrix overflow"));
                }
                columns[col][row] = value as f32;
            }
        }
        // Only direction is used by the shader. A common positive scale leaves
        // the inverse-transpose result equivalent while bounding vertex products
        // before interpolation and fragment normalization.
        let magnitude = columns
            .iter()
            .flatten()
            .fold(0.0_f32, |maximum, value| maximum.max(value.abs()));
        for column in &mut columns {
            for value in column {
                *value /= magnitude;
            }
        }
        Ok(columns)
    }
    /// Independent scale on the local axes.
    pub fn scale(scale: [f32; 3]) -> Result<Self, RenderError> {
        Self::from_columns([
            [scale[0], 0.0, 0.0, 0.0],
            [0.0, scale[1], 0.0, 0.0],
            [0.0, 0.0, scale[2], 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ])
    }
    /// Right-handed rotation about +Y, in radians.
    pub fn rotation_y(radians: f32) -> Result<Self, RenderError> {
        let (s, c) = radians.sin_cos();
        Self::from_columns([
            [c, 0.0, -s, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [s, 0.0, c, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ])
    }
    /// Apply this transform, then `next`.
    pub fn then(self, next: Self) -> Result<Self, RenderError> {
        Self::from_columns(next.0.multiply(self.0)?.0)
    }
    /// Model matrix for CPU queries and custom providers.
    pub fn matrix(self) -> Matrix4 {
        self.0
    }
}

/// Validated right-handed perspective camera. Aspect derives from the target.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PerspectiveCamera {
    eye: [f32; 3],
    view: Matrix4,
    vertical_fov: f32,
    near: f32,
    far: f32,
}
impl PerspectiveCamera {
    /// Look from `eye` toward `target`. Up must not be parallel to the view axis.
    /// FOV is in radians, strictly between 0 and pi; 0 < near < far.
    pub fn look_at(
        eye: [f32; 3],
        target: [f32; 3],
        up: [f32; 3],
        vertical_fov: f32,
        near: f32,
        far: f32,
    ) -> Result<Self, RenderError> {
        if !eye
            .iter()
            .chain(target.iter())
            .chain(up.iter())
            .all(|v| v.is_finite())
            || !vertical_fov.is_finite()
            || vertical_fov <= 0.0
            || vertical_fov >= std::f32::consts::PI
            || !near.is_finite()
            || !far.is_finite()
            || near <= 0.0
            || far <= near
        {
            return Err(invalid("invalid perspective camera"));
        }
        fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
            (0..3).map(|i| a[i] * b[i]).sum()
        }
        fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
            [
                a[1] * b[2] - a[2] * b[1],
                a[2] * b[0] - a[0] * b[2],
                a[0] * b[1] - a[1] * b[0],
            ]
        }
        fn normalized(v: [f64; 3]) -> Result<[f64; 3], RenderError> {
            let length = dot(v, v).sqrt();
            if length <= 1e-12 {
                return Err(invalid("degenerate camera basis"));
            }
            Ok(v.map(|x| x / length))
        }
        let eye_world = eye;
        let eye = eye.map(f64::from);
        let z = normalized(std::array::from_fn(|i| eye[i] - f64::from(target[i])))?;
        let up = normalized(up.map(f64::from))?;
        let x = normalized(cross(up, z))?;
        let y = cross(z, x);
        let view = Matrix4::checked([
            [x[0] as f32, y[0] as f32, z[0] as f32, 0.0],
            [x[1] as f32, y[1] as f32, z[1] as f32, 0.0],
            [x[2] as f32, y[2] as f32, z[2] as f32, 0.0],
            [
                -dot(x, eye) as f32,
                -dot(y, eye) as f32,
                -dot(z, eye) as f32,
                1.0,
            ],
        ])?;
        let camera = Self {
            eye: eye_world,
            view,
            vertical_fov,
            near,
            far,
        };
        camera.view_projection(1.0)?;
        Ok(camera)
    }
    /// World-to-view matrix.
    pub fn view(self) -> Matrix4 {
        self.view
    }
    /// Camera position in world space.
    pub fn eye(self) -> [f32; 3] {
        self.eye
    }
    /// World-to-clip matrix for wgpu's 0..1 depth convention.
    pub fn view_projection(self, aspect: f32) -> Result<Matrix4, RenderError> {
        if !aspect.is_finite() || aspect <= 0.0 {
            return Err(invalid("invalid camera aspect"));
        }
        let f = 1.0 / (f64::from(self.vertical_fov) * 0.5).tan();
        let n = f64::from(self.near);
        let far = f64::from(self.far);
        Matrix4::checked([
            [(f / f64::from(aspect)) as f32, 0.0, 0.0, 0.0],
            [0.0, f as f32, 0.0, 0.0],
            [0.0, 0.0, (far / (n - far)) as f32, -1.0],
            [0.0, 0.0, (far * n / (n - far)) as f32, 0.0],
        ])?
        .multiply(self.view)
    }
}

/// Immutable indexed triangle mesh. Winding is retained; this profile is double-sided.
#[derive(Clone, Debug)]
pub struct Mesh3D {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    indices: Vec<u32>,
}
impl Mesh3D {
    /// Validate finite positions, complete triangles and all index references.
    pub fn new(positions: Vec<[f32; 3]>, indices: Vec<u32>) -> Result<Self, RenderError> {
        if positions.is_empty()
            || indices.is_empty()
            || !indices.len().is_multiple_of(3)
            || positions.len() > u32::MAX as usize
            || indices.len() > u32::MAX as usize
            || !positions.iter().flatten().all(|v| v.is_finite())
            || indices.iter().any(|&i| i as usize >= positions.len())
        {
            return Err(invalid("invalid triangle mesh"));
        }
        let normals = derive_vertex_normals(&positions, &indices);
        Ok(Self {
            positions,
            normals,
            indices,
        })
    }
    /// Local-space positions, in application world units.
    pub fn positions(&self) -> &[[f32; 3]] {
        &self.positions
    }
    /// Area-weighted vertex normals derived from the triangle faces.
    pub fn normals(&self) -> &[[f32; 3]] {
        &self.normals
    }
    /// Triangle-list indices.
    pub fn indices(&self) -> &[u32] {
        &self.indices
    }
}

fn derive_vertex_normals(positions: &[[f32; 3]], indices: &[u32]) -> Vec<[f32; 3]> {
    let mut accumulated = vec![[0.0f64; 3]; positions.len()];
    for triangle in indices.as_chunks::<3>().0 {
        let a = positions[triangle[0] as usize].map(f64::from);
        let b = positions[triangle[1] as usize].map(f64::from);
        let c = positions[triangle[2] as usize].map(f64::from);
        let ab = std::array::from_fn::<_, 3, _>(|i| b[i] - a[i]);
        let ac = std::array::from_fn::<_, 3, _>(|i| c[i] - a[i]);
        let face = [
            ab[1] * ac[2] - ab[2] * ac[1],
            ab[2] * ac[0] - ab[0] * ac[2],
            ab[0] * ac[1] - ab[1] * ac[0],
        ];
        for &index in triangle {
            for (component, value) in face.iter().enumerate() {
                accumulated[index as usize][component] += value;
            }
        }
    }
    accumulated
        .into_iter()
        .map(|normal| {
            let length = normal.iter().map(|value| value * value).sum::<f64>().sqrt();
            if !length.is_finite() || length <= 1.0e-30 {
                [0.0, 0.0, 1.0]
            } else {
                normal.map(|value| (value / length) as f32)
            }
        })
        .collect()
}

/// Finite directional-light source; direction points from the surface to the light.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DirectionalLight {
    direction: [f32; 3],
    color: [f32; 3],
    intensity: f32,
}
impl DirectionalLight {
    /// Construct a normalized light direction, sRGB light color, and linear intensity.
    pub fn new(
        direction_to_light: [f32; 3],
        color_srgb: [f32; 3],
        intensity: f32,
    ) -> Result<Self, RenderError> {
        if !direction_to_light.iter().all(|v| v.is_finite())
            || !color_srgb
                .iter()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
            || !intensity.is_finite()
            || !(0.0..=100_000.0).contains(&intensity)
        {
            return Err(invalid("invalid directional light"));
        }
        let length = direction_to_light
            .iter()
            .map(|v| f64::from(*v).powi(2))
            .sum::<f64>()
            .sqrt();
        if length <= 1.0e-12 {
            return Err(invalid("directional light direction is zero"));
        }
        Ok(Self {
            direction: direction_to_light.map(|v| (f64::from(v) / length) as f32),
            color: color_srgb,
            intensity,
        })
    }
    pub fn direction(self) -> [f32; 3] {
        self.direction
    }
    pub fn color_srgb(self) -> [f32; 3] {
        self.color
    }
    pub fn intensity(self) -> f32 {
        self.intensity
    }
    fn linear_radiance(self) -> [f32; 3] {
        srgb_to_linear(self.color).map(|v| v * self.intensity)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct MetallicRoughness {
    metallic: f32,
    roughness: f32,
    light: DirectionalLight,
}

/// Opaque sRGB scene material. `new` remains unlit; the optional
/// metallic/roughness profile uses one directional light and derived mesh normals.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BasicMaterial {
    color: [f32; 3],
    metallic_roughness: Option<MetallicRoughness>,
}
impl BasicMaterial {
    /// Color channels must be finite and in 0..=1.
    pub fn new(srgb: [f32; 3]) -> Result<Self, RenderError> {
        if srgb
            .iter()
            .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
        {
            Ok(Self {
                color: srgb,
                metallic_roughness: None,
            })
        } else {
            Err(invalid("invalid scene material"))
        }
    }
    /// Create a basic Cook-Torrance metallic/roughness material. Roughness is
    /// constrained to 0.04..=1 to avoid a singular zero-width microfacet lobe.
    pub fn metallic_roughness(
        base_color_srgb: [f32; 3],
        metallic: f32,
        roughness: f32,
        light: DirectionalLight,
    ) -> Result<Self, RenderError> {
        if !base_color_srgb
            .iter()
            .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
            || !metallic.is_finite()
            || !(0.0..=1.0).contains(&metallic)
            || !roughness.is_finite()
            || !(0.04..=1.0).contains(&roughness)
        {
            return Err(invalid("invalid metallic-roughness material"));
        }
        Ok(Self {
            color: base_color_srgb,
            metallic_roughness: Some(MetallicRoughness {
                metallic,
                roughness,
                light,
            }),
        })
    }
    /// Original sRGB color.
    pub fn srgb(self) -> [f32; 3] {
        self.color
    }
    fn linear(self) -> [f32; 4] {
        let rgb = srgb_to_linear(self.color);
        [rgb[0], rgb[1], rgb[2], 1.0]
    }
    fn params(self) -> ([f32; 4], [f32; 4], [f32; 4]) {
        match self.metallic_roughness {
            Some(profile) => (
                [profile.metallic, profile.roughness, 1.0, 0.03],
                [
                    profile.light.direction[0],
                    profile.light.direction[1],
                    profile.light.direction[2],
                    0.0,
                ],
                [
                    profile.light.linear_radiance()[0],
                    profile.light.linear_radiance()[1],
                    profile.light.linear_radiance()[2],
                    0.0,
                ],
            ),
            None => ([0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0; 4]),
        }
    }
}

fn srgb_to_linear(srgb: [f32; 3]) -> [f32; 3] {
    srgb.map(|v| {
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    })
}

/// App-owned object description; IDs have meaning only in the owning scene.
#[derive(Clone, Debug)]
pub struct SceneObject {
    /// Stable app-owned object identity, unique within a snapshot.
    pub id: u64,
    /// Immutable mesh; may be shared by objects and snapshots.
    pub mesh: Arc<Mesh3D>,
    /// Local-to-world affine transform.
    pub transform: Transform3D,
    /// Opaque unlit surface color.
    pub material: BasicMaterial,
}
impl SceneObject {
    /// Sample an application-owned translation track into this object's current
    /// transform, preserving its rotation and scale.
    pub fn apply_translation_animation(
        &mut self,
        animation: &TranslationAnimation,
        time_seconds: f64,
    ) -> Result<(), RenderError> {
        self.transform = self
            .transform
            .with_translation(animation.sample(time_seconds)?)?;
        Ok(())
    }
}

/// Immutable scene/camera description, independent of UI tree ownership.
#[derive(Clone, Debug)]
pub struct SceneSnapshot {
    revision: Revision,
    camera: PerspectiveCamera,
    objects: Vec<SceneObject>,
}
impl SceneSnapshot {
    /// IDs must be unique. Increment revision when camera or scene content changes.
    pub fn new(
        revision: Revision,
        camera: PerspectiveCamera,
        objects: Vec<SceneObject>,
    ) -> Result<Self, RenderError> {
        let mut ids = BTreeSet::new();
        if objects.iter().any(|object| !ids.insert(object.id)) {
            return Err(invalid("duplicate scene object ID"));
        }
        Ok(Self {
            revision,
            camera,
            objects,
        })
    }
    /// App-owned scene revision.
    pub fn revision(&self) -> Revision {
        self.revision
    }
    /// Camera used by this snapshot.
    pub fn camera(&self) -> PerspectiveCamera {
        self.camera
    }
    /// Immutable object descriptions.
    pub fn objects(&self) -> &[SceneObject] {
        &self.objects
    }
}

/// Explicit per-preparation ceilings; not a process-wide GPU memory quota.
#[derive(Clone, Copy, Debug)]
pub struct SceneBudgets {
    /// Maximum object/draw count.
    pub max_objects: usize,
    /// Position/normal vertex, index and uniform bytes (meshes counted per instance).
    pub upload_bytes: u64,
    /// Maximum color/depth extent in pixels; each depth pixel costs four bytes.
    pub target_pixels: u64,
}
impl Default for SceneBudgets {
    fn default() -> Self {
        Self {
            max_objects: 4096,
            upload_bytes: 64 << 20,
            target_pixels: 4096 * 4096,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniform {
    mvp: [[f32; 4]; 4],
    model: [[f32; 4]; 4],
    color: [f32; 4],
    params: [f32; 4],
    light_direction: [f32; 4],
    light_color: [f32; 4],
    camera_position: [f32; 4],
    normal_matrix: [[f32; 4]; 3],
}
const _: () = assert!(std::mem::size_of::<Uniform>() == 256);
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuVertex {
    position: [f32; 3],
    normal: [f32; 3],
}
struct GpuDraw {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    bind: wgpu::BindGroup,
    count: u32,
}

/// Minimal offscreen provider. Never submits work, creates a device, or owns a loop.
/// Buffers are immutable after preparation and retained by encoded wgpu commands.
pub struct SceneProvider {
    snapshot: SceneSnapshot,
    budgets: SceneBudgets,
    contract: Option<AttachmentContract>,
    generation: Option<DeviceGeneration>,
    pipeline: Option<wgpu::RenderPipeline>,
    draws: Vec<GpuDraw>,
    prepared: Option<Revision>,
}
impl SceneProvider {
    /// Construct without accessing the GPU, enforcing the scene allocation budget.
    pub fn new(snapshot: SceneSnapshot, budgets: SceneBudgets) -> Result<Self, RenderError> {
        Self::validate_budget(&snapshot, budgets)?;
        Ok(Self {
            snapshot,
            budgets,
            contract: None,
            generation: None,
            pipeline: None,
            draws: Vec::new(),
            prepared: None,
        })
    }
    fn validate_budget(snapshot: &SceneSnapshot, budgets: SceneBudgets) -> Result<(), RenderError> {
        let mut bytes = 0u64;
        for object in &snapshot.objects {
            let size = object.mesh.positions.len() as u64 * std::mem::size_of::<GpuVertex>() as u64
                + object.mesh.indices.len() as u64 * 4
                + std::mem::size_of::<Uniform>() as u64;
            bytes = bytes.checked_add(size).ok_or(RenderError::BudgetExceeded)?;
        }
        if snapshot.objects.len() > budgets.max_objects || bytes > budgets.upload_bytes {
            return Err(RenderError::BudgetExceeded);
        }
        Ok(())
    }
    /// Replace app-owned content atomically with a strictly newer scene revision.
    /// With ProviderSession, obtain mutable access only after retiring prior work.
    pub fn set_snapshot(&mut self, snapshot: SceneSnapshot) -> Result<(), RenderError> {
        if snapshot.revision <= self.snapshot.revision {
            return Err(invalid("stale scene revision"));
        }
        Self::validate_budget(&snapshot, self.budgets)?;
        self.snapshot = snapshot;
        self.prepared = None;
        self.draws.clear();
        Ok(())
    }
    /// Current app snapshot, available to CPU queries without GPU access.
    pub fn snapshot(&self) -> &SceneSnapshot {
        &self.snapshot
    }
    fn validate_extent(&self, size: [u32; 2]) -> Result<(), RenderError> {
        if size.contains(&0) {
            return Err(invalid("scene suspended at zero extent"));
        }
        if u64::from(size[0]) * u64::from(size[1]) > self.budgets.target_pixels {
            return Err(RenderError::BudgetExceeded);
        }
        Ok(())
    }
}

impl RenderProvider for SceneProvider {
    fn requires_depth(&self) -> bool {
        true
    }
    fn negotiate(&mut self, contract: AttachmentContract) -> Result<ContributionMode, RenderError> {
        contract.negotiate(ContributionMode::Offscreen)?;
        self.validate_extent(contract.size)?;
        // Re-negotiation requires recreation; never reuse a pipeline for another format.
        self.contract = Some(contract);
        self.pipeline = None;
        self.prepared = None;
        self.draws.clear();
        Ok(ContributionMode::Offscreen)
    }
    fn prepare(&mut self, context: &GpuContext, revision: Revision) -> Result<(), RenderError> {
        context.check(self.generation.ok_or(RenderError::StaleResource)?)?;
        if revision != self.snapshot.revision {
            return Err(invalid("scene preparation revision mismatch"));
        }
        if self.prepared == Some(revision) {
            return Ok(());
        }
        let contract = self
            .contract
            .ok_or_else(|| invalid("scene not negotiated"))?;
        self.validate_extent(contract.size)?;
        crate::context::validate_size(
            contract.size,
            context.device.limits().max_texture_dimension_2d,
        )?;
        let vp = self
            .snapshot
            .camera
            .view_projection(contract.size[0] as f32 / contract.size[1] as f32)?;
        let pipeline = self
            .pipeline
            .as_ref()
            .ok_or_else(|| invalid("scene pipeline needs recreation"))?;
        let layout = pipeline.get_bind_group_layout(0);
        // Validate all CPU math and per-buffer limits before creating any GPU data.
        let uniforms = self
            .snapshot
            .objects
            .iter()
            .map(|object| {
                if object.mesh.positions.len() as u64 * std::mem::size_of::<GpuVertex>() as u64
                    > context.device.limits().max_buffer_size
                    || object.mesh.indices.len() as u64 * 4
                        > context.device.limits().max_buffer_size
                {
                    return Err(RenderError::BudgetExceeded);
                }
                let mvp = vp.multiply(object.transform.matrix())?;
                let model = object.transform.matrix();
                // Prevent finite matrix/vertex combinations from overflowing in the shader.
                for p in &object.mesh.positions {
                    mvp.transform([p[0], p[1], p[2], 1.0])?;
                    model.transform([p[0], p[1], p[2], 1.0])?;
                }
                let (params, light_direction, light_color) = object.material.params();
                Ok(Uniform {
                    mvp: mvp.0,
                    model: model.0,
                    color: object.material.linear(),
                    params,
                    light_direction,
                    light_color,
                    camera_position: [
                        self.snapshot.camera.eye[0],
                        self.snapshot.camera.eye[1],
                        self.snapshot.camera.eye[2],
                        1.0,
                    ],
                    normal_matrix: if params[2] > 0.5 {
                        object.transform.normal_matrix()?
                    } else {
                        [
                            [1.0, 0.0, 0.0, 0.0],
                            [0.0, 1.0, 0.0, 0.0],
                            [0.0, 0.0, 1.0, 0.0],
                        ]
                    },
                })
            })
            .collect::<Result<Vec<_>, RenderError>>()?;
        let draws = self
            .snapshot
            .objects
            .iter()
            .zip(uniforms)
            .map(|(object, uniform)| {
                let gpu_vertices: Vec<_> = object
                    .mesh
                    .positions
                    .iter()
                    .zip(&object.mesh.normals)
                    .map(|(position, normal)| GpuVertex {
                        position: *position,
                        normal: *normal,
                    })
                    .collect();
                let vertices =
                    context
                        .device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("view scene vertices"),
                            contents: bytemuck::cast_slice(&gpu_vertices),
                            usage: wgpu::BufferUsages::VERTEX,
                        });
                let indices =
                    context
                        .device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("view scene indices"),
                            contents: bytemuck::cast_slice(&object.mesh.indices),
                            usage: wgpu::BufferUsages::INDEX,
                        });
                let uniform =
                    context
                        .device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("view scene uniform"),
                            contents: bytemuck::bytes_of(&uniform),
                            usage: wgpu::BufferUsages::UNIFORM,
                        });
                let bind = context
                    .device
                    .create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("view scene object"),
                        layout: &layout,
                        entries: &[wgpu::BindGroupEntry {
                            binding: 0,
                            resource: uniform.as_entire_binding(),
                        }],
                    });
                GpuDraw {
                    vertices,
                    indices,
                    bind,
                    count: object.mesh.indices.len() as u32,
                }
            })
            .collect();
        self.draws = draws;
        self.prepared = Some(revision);
        Ok(())
    }
    fn encode(&mut self, pass: &mut wgpu::RenderPass<'_>) -> Result<(), RenderError> {
        if self.prepared != Some(self.snapshot.revision) {
            return Err(invalid("scene not prepared"));
        }
        pass.set_pipeline(
            self.pipeline
                .as_ref()
                .ok_or_else(|| invalid("scene pipeline absent"))?,
        );
        for draw in &self.draws {
            pass.set_bind_group(0, &draw.bind, &[]);
            pass.set_vertex_buffer(0, draw.vertices.slice(..));
            pass.set_index_buffer(draw.indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..draw.count, 0, 0..1);
        }
        Ok(())
    }
    fn retire(&mut self, _revision: Revision) {}
    fn resize(&mut self, size: [u32; 2]) -> Result<(), RenderError> {
        self.validate_extent(size)?;
        let contract = self
            .contract
            .as_mut()
            .ok_or_else(|| invalid("scene not negotiated"))?;
        contract.size = size;
        self.prepared = None;
        self.draws.clear();
        Ok(())
    }
    fn recreate(&mut self, context: &GpuContext) -> Result<(), RenderError> {
        context.check(context.generation())?;
        let contract = self
            .contract
            .ok_or_else(|| invalid("scene not negotiated"))?;
        crate::context::validate_size(
            contract.size,
            context.device.limits().max_texture_dimension_2d,
        )?;
        let shader = context
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("view basic scene"),
                source: wgpu::ShaderSource::Wgsl(include_str!("scene.wgsl").into()),
            });
        let attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3];
        let pipeline = context
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("view opaque depth scene"),
                layout: None,
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<GpuVertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &attributes,
                    })],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: contract.format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::Less),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            });
        self.pipeline = Some(pipeline);
        self.generation = Some(context.generation());
        self.prepared = None;
        self.draws.clear();
        Ok(())
    }
    fn failed(&mut self, _error: &RenderError) {
        self.prepared = None;
        self.draws.clear();
    }
}
