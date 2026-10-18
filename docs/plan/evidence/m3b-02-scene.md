# M3B-02 basic scene provider — 2026-10-18

Scope: optional camera/mesh/transform/material/depth rendering. Picking, overlays,
viewport scheduling and host-embedding proofs remain M3B-03 through M3B-10.
This report supports the canonical checklist rather than a separate progress board.

Implementation: [scene module](../../../crates/view-wgpu/src/scene.rs),
[WGSL](../../../crates/view-wgpu/src/scene.wgsl),
[provider session](../../../crates/view-wgpu/src/provider.rs),
[CPU contracts](../../../crates/view-wgpu/tests/scene_contracts.rs),
[GPU fixture](../../../crates/view-wgpu/examples/scene.rs), and
[independent facade consumer](../../../tests/compatibility/facade-consumer/tests/scene.rs).
Conventions, budgets and limitations: [scene guide](../../guides/scene.md).

Six analytic CPU tests cover near/far/aspect projection, translated/rotated camera
bases, affine transform order and handedness, invalid mesh/material/camera data,
overflow (including cancellation hiding GPU intermediates), duplicate object IDs,
atomic revision/budget failures and offscreen depth negotiation.

The explicit GPU fixture renders original indexed quads, composes their texture
through the existing 2D renderer, and compares independently expected pixels at
2/255 tolerance. It checks nearer geometry drawn before farther geometry, identical
pixels after reversed draw order, behind/near/far clipping, model scale/translation,
sRGB color, transparent clearing, changed aspect/extent, stale revision/attachment
rejection and recreation on a second device. It opens no window and injects no input.

Local GPU: NVIDIA GeForce RTX 3050 6GB Laptop GPU, discrete hardware, DX12,
PCI `0000:01:00.0`, driver `32.0.16.1714`. Local CPU checks use the pinned Rust
1.98.1 toolchain on Windows x64. Generated log:
`target/verification/m3b-02-checks.log`; image: `target/verification/m3b-scene.ppm`.

All verification commands below passed locally:

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo doc --workspace --no-deps --locked
cargo clippy -p view-wgpu --all-targets --features scene --locked -- -D warnings
cargo test -p view-wgpu --features scene --locked
cargo doc -p view-wgpu --features scene --no-deps --locked
cargo check -p view --no-default-features --locked
cargo check -p view --all-features --locked
cargo test --manifest-path tests/compatibility/facade-consumer/Cargo.toml --features scene --locked --target-dir target/consumer
cargo run -p view-wgpu --features scene --example scene --locked -- dx12
cargo run -p view-wgpu --example offscreen --locked -- dx12
```

The unchanged 2D GPU fixture also passed, covering the existing color-only/direct
provider path after the shared session extension. Documentation local links and
fences, consumer formatting, `git diff --check` and verification-script syntax
passed. The minimal facade dependency tree still contains only `view` and
`view-core`. These are local results; the updated CI script has not run remotely
for this increment.

`scripts/verify.ps1` now includes the optional scene CPU/consumer profiles in the
normal CI path and the scene readback fixture only under explicit `-Gpu`.
No registry dependency or lockfile change is required: wgpu 30 and bytemuck remain
the qualified family, and existing first-party licenses apply. The core has no
new GPU dependency; the scene module is feature-gated within the existing adapter.

No Linux/macOS execution, native window/physical-input certification, hardware
device-loss recovery or performance measurement is claimed for this increment.
Explicit second-device recreation does not simulate a driver reset. M0 runner,
M3A-14 and M2B-11 gaps remain open. T-07/T-12 receive only the scene-specific
offscreen evidence above; their broader native/multiple-viewport gates remain.
