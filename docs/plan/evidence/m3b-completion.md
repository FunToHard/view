# M3B canvas, scene and host integration — 2026-10-19

This report completes the implementation/proof sequence begun by
[M3B-01](m3b-01-canvas.md) and [M3B-02](m3b-02-scene.md). The canonical
[checklist](../implementation-checklist.md) remains the progress record.

| Item | Implementation and acceptance evidence |
| --- | --- |
| M3B-03 | [CPU picking](../../../crates/view-wgpu/src/scene/picking.rs), latest-request owner namespace and scene/camera/viewport stamps; nearest-hit, outside/behind projection, stale/duplicate/foreign/cancelled results, real worker completion and named command tests in [picking contracts](../../../crates/view-wgpu/tests/picking_contracts.rs) |
| M3B-04 | Projected gizmo painted above scene; runtime hit-order/capture test; [DragSession](../../../crates/view-core/src/viewport.rs) preview/commit/cancel and [typed app undo test](../../../crates/view-core/tests/viewport_contracts.rs), one history entry despite multiple moves and repeated release delivery |
| M3B-05 | Bounded host-clock ViewportScheduler, explicit cadence/demand, ancestor visibility and host occlusion; 120 CPU viewport ticks preserve retained/immediate state, build/layout/commit counters and committed geometry |
| M3B-06 | [Host service bridge](../../../crates/view-core/src/host.rs) with explicit unsupported services, coherent semantic publication/retry; real host-owned device/targets/encoder/queue in [embedded fixture](../../../crates/view-wgpu/examples/embedded.rs); one submission per composition |
| M3B-07 | [Compatibility/embedding guide](../../guides/embedding.md); host engine uses its own WGSL pipeline and output texture without SceneSnapshot/RenderProvider translation; same wgpu device shared with view, one host loop |
| M3B-08 | Embedded DX12 fixture: multiple clipped viewports, 1×/2× logical-to-physical changes, resize, occlusion/resume, cache budget and frame retirement assertions, explicit host device replacement, foreign target rejection, output after recreation; no CPU composition copy |
| M3B-09 | Bounded [app-owned tile/brush fixture](../../../crates/view-wgpu/examples/support/brush.rs); negative tile boundaries, preview interpolation, atomic budget failure, exact one-stroke undo tested in [brush contracts](../../../crates/view-wgpu/tests/brush_contracts.rs); GPU committed-stroke and transient-preview pixels; pressure/tilt unsupported |
| M3B-10 | Embedded fixture reports separate UI build/layout/commit deltas, per-viewport frames, host-engine frames, composition/submission counts and diagnostic readbacks; static retained/immediate inspectors beside animated scene stay unchanged |

Local environment: Windows x64, pinned Rust 1.98.1. GPU: NVIDIA GeForce RTX 3050
6GB Laptop GPU, DiscreteGpu, DX12, driver `32.0.16.1714`, PCI `0000:01:00.0`.
No software renderer is being labeled as hardware.

The 12-tick deterministic host fixture includes one occluded tick. Expected
executed work is 11 animated-scene frames, 4 on-demand-scene frames, 11 independent
host-engine passes, 11 composition frames and exactly 11 host submissions. UI
build/layout/commit deltas are all zero after initialization; semantic publication
occurs once. Three diagnostic readbacks validate analytic pixels with a 2/255
tolerance. One initial host device is replaced once explicitly during recovery;
the framework creates no additional device. This is a correctness/counter fixture,
not a measured 60/120 Hz workload or driver-reset simulation.

All commands below passed locally. Verification output is recorded in
`target/verification/m3b-completion-checks.log` (generated, not committed):

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
cargo test --manifest-path tests/compatibility/facade-consumer/Cargo.toml --locked --target-dir target/consumer
cargo test --manifest-path tests/compatibility/facade-consumer/Cargo.toml --features scene --locked --target-dir target/consumer
cargo run -p view-wgpu --features scene --example embedded --locked -- dx12
cargo run -p view-wgpu --features scene --example scene --locked -- dx12
cargo run -p view-wgpu --example offscreen --locked -- dx12
```

The final inspector-rendering refinement also passed formatting and the optional
scene all-target Clippy check. Local documentation links/fences, verification
script syntax and `git diff --check` passed. Added CPU suites contain four
viewport/host/drag tests, five picking/gizmo tests and two tiled-brush tests;
the earlier six scene tests and existing workspace regressions also passed.

No new registry dependency, crate or lockfile is required. The default facade
still has only core; optional scene code stays in the wgpu adapter. Both public
profiles are exercised by independent consumers. `scripts/verify.ps1` includes
new CPU tests through existing package profiles and runs embedded GPU checks
only with `-Gpu`. Fixture shader/mesh/brush content is original first-party code.

Coverage limits remain explicit: these new tests executed locally on Windows,
not Linux/macOS or remote CI. DPI evidence is scripted scale/extent changes,
not actual physical monitor movement. Native OS capture, IME/clipboard and screen
reader execution require the separately provisioned desktop gates. CPU routing
and a recording host semantic sink do not certify them. M0-12/M0-13/M0-15,
M3A-14 and M2B-11 remain open. T-06/T-09 have executable CPU/offscreen proofs;
T-07/T-08/T-11/T-12 have the layer-specific evidence above, with native/platform
certification still tracked in their owning gates. No missing coverage is a pass.
