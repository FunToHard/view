# F-05 scene assets and animation evidence

The scene feature now includes a bounded positions-only OBJ importer and a
validated linear translation track. OBJ parsing accepts finite `v` and indexed
`f` records, handles relative negative position indices, fan-triangulates
polygons, and rejects unsupported material/normal/texture-coordinate profiles.
Source bytes, positions, face count, and generated triangles have configurable
caps. Animation keyframes are finite, nonnegative, strictly ordered, and capped;
sampling clamps endpoints and interpolates in between. Applying a track to a
`SceneObject` changes translation while preserving scale/rotation.

Behavior checks live in `crates/view-wgpu/tests/scene_assets.rs`. The explicit
offscreen scene example imports the quad through the OBJ parser and samples an
animation track before sending it through the scene provider/shader.

## Windows execution

- `cargo test -p view-wgpu --features scene --locked`: passed, including 5 new
  scene asset/animation cases and the existing scene/picking/renderer contracts.
- `cargo clippy -p view-wgpu --all-targets --features scene --locked -- -D warnings`:
  passed.
- `cargo run -p view-wgpu --features scene --example scene --locked -- dx12`:
  passed on the NVIDIA GeForce RTX 3050 6GB Laptop GPU (DiscreteGpu, DX12).
  The existing offscreen assertions passed for depth/order, clipping, color,
  composition, resize and device recreation with the imported animated quad.
- Direct rustfmt check passed for the changed scene module, importer, tests and
  scene example. No Linux/macOS GPU run is claimed.

## Coverage

Final 2026-10-26 hardware execution also passed the opposing-light case: an odd
129×65 target's center pixel must match analytic ambient-only RGB within 2/255
with opaque alpha. The shader guards zero halfway vectors and scales directions
before normalization; inverse-transpose normal matrices share a positive scale
to keep vertex products bounded. This prevents undefined normalization from
turning valid back lighting into non-finite output. Receipt:
`target/verification/m8-scene-final.log`; final renderer/platform Clippy/tests:
`target/verification/m8-render-platform-final.log`.

The integration adds validated directional lighting and a basic metallic-roughness
shader with derived mesh normals; the GPU example checks its pixels separately
from the unchanged unlit profile. Shadows, skeletal animation, material-file
loading and broader asset formats remain unimplemented. This is partial F-05 evidence. The renderer
remains the existing hardware-wgpu path; no software renderer was introduced.
