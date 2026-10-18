# Basic 3D scenes

Enable `view/scene` for `view::scene`, or `view-wgpu/scene` for
`view_wgpu::scene`. The feature adds no dependencies beyond the already qualified
wgpu/bytemuck renderer family. Default facade and 2D renderer builds do not enable
it. The module consumes app-owned snapshots, with no ECS, scene graph, asset
loader, event loop or additional GPU device.

`Mesh3D` validates finite positions, complete indexed triangles and index bounds.
Meshes are immutable and can be shared with `Arc`. `SceneObject` combines an
app-owned ID, mesh, affine `Transform3D` and `BasicMaterial`. IDs must be unique
within a `SceneSnapshot`; they are not runtime node handles. The application owns
document edits and undo. `SceneSnapshot` includes a camera and revision; replace
it with a strictly newer revision when any scene or camera content changes.

Conventions:

- World coordinates are right-handed, with +Y up; a camera looks along local −Z.
  World units are application-defined. Coordinates/matrices are f32; rebase large
  worlds in the application before creating mesh data.
- Matrices are column-major and multiply column vectors. `a.multiply(b)` applies
  `b` first; `transform.then(next)` applies the receiver first. Models must be
  affine; zero or negative scale is allowed. No face culling is performed.
- `PerspectiveCamera::look_at` uses vertical field of view in radians, 0 < FOV < pi,
  and 0 < near < far. The eye and target must differ and up must not be parallel
  to the view axis. Basis normalization rejects lengths <= 1e-12. Aspect is target
  width/height, so resize preserves vertical FOV.
- Clip depth is wgpu's 0..1 range, near=0 and far=1. The private Depth32Float
  attachment clears to 1 each pass and uses `Less` with depth writes enabled.
  Coplanar ties keep the first fragment; the exact far plane fails `Less` against
  the clear value. Depth remains separate from UI composition order.
- `BasicMaterial` is opaque unlit RGB in sRGB 0..1. It converts to linear RGB for
  the shader and renders into the negotiated sRGB attachment. Background pixels
  remain transparent; covered pixels have alpha 1. Lighting, normals, textures,
  transparency sorting, PBR, shadows and MSAA are outside this initial profile.

Create `SceneProvider::new(snapshot, budgets)` and wrap it in `ProviderSession`
with a single-sample RGBA8/BGRA8 sRGB `AttachmentContract`. The provider always
requests offscreen mode. The session owns a private depth attachment and clears
color/depth for every contribution. Default existing providers request no depth;
direct contributions remain color-only.

Call `encode_offscreen` with the snapshot revision and a target matching the
negotiated extent/format. The provider prepares immutable vertex/index/uniform
buffers and encodes draws into the supplied pass. It never submits or presents.
Submit the encoder through the host's shared queue and then call `submitted` with
that revision. `discard_encoded` requires discarding the corresponding encoder.
Poll the device and call `retire_completed` to release session completion records.
Use `provider_mut()?.set_snapshot(...)` only after outstanding work is retired.
This intentionally serializes updates to an individual provider while work is
outstanding; [host-driven scheduling](embedding.md) independently selects which
viewports need frames without rebuilding retained/immediate inspectors.

Compose the output by importing its texture into `Renderer` with
`AlphaMode::Premultiplied`, sRGB transfer and top-left origin. Submit scene writes
before composition reads on the same queue, and finish readers before reusing
the target. No CPU copy is needed for composition. Readback in the example is
only for test assertions. The wgpu 30 version/device boundary is unchanged.

`SceneBudgets` limits objects, per-preparation upload bytes and target pixels.
Shared meshes are currently uploaded per object; unchanged preparations reuse
buffers, while scene updates, resize and recreation rebuild them. The upload
budget counts vertices, indices and 80 uniform bytes per object. GPU allocator
alignment/driver overhead is not measured. Depth costs four bytes per target
pixel; app-owned color targets and up to three session submissions have separate
lifetimes. These are bounded resource contracts, not a process-wide memory quota.
wgpu retains encoded buffer/attachment handles until their work is released.

Zero size returns a suspension error; the caller skips encoding until resized.
Resize invalidates projection/prepared buffers. A replacement context requires
session recreation and targets on the new device; old generations reject use.
Preparation validates revision, target limits and CPU data before recording draws.
Device-loss recovery remains host-controlled.

```text
cargo test -p view-wgpu --features scene --locked
cargo run -p view-wgpu --features scene --example scene --locked -- dx12
cargo test --manifest-path tests/compatibility/facade-consumer/Cargo.toml --features scene --locked --target-dir target/consumer
```

The explicit GPU example contains original analytic quad meshes, opens no
window, and checks depth/order, clipping, transforms, color, composition, resize
and recreation. It writes `target/verification/m3b-scene.ppm`. Use `vulkan` or
`metal` only on a provisioned compatible host. Picking, overlays, gesture undo,
continuous scheduling and host embedding are documented in the
[embedding guide](embedding.md), with [completion evidence](../plan/evidence/m3b-completion.md).
