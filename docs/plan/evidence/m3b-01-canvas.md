# M3B-01 Canvas2D — 2026-10-18

Scope: start Phase 6 with backend-independent Canvas2D; M3B-02 through M3B-10
remain unchecked. Existing infrastructure and native-control coverage gaps remain.

Implementation: [canvas module](../../../crates/view-core/src/canvas.rs),
[contracts](../../../crates/view-core/tests/canvas_contracts.rs),
[CPU example](../../../crates/view-core/examples/canvas.rs),
[independent facade consumer](../../../tests/compatibility/facade-consumer/tests/canvas.rs).
Public behavior and limitations: [guide](../../guides/canvas.md).

The five analytic contract tests cover large-coordinate rebasing, precision
rejection, anchored zoom, pan, invalid-change atomicity, camera-coherent hit and
semantic bounds, half-open/viewport clipping, paint ordering, zero-size suspension,
semantic-only records, skipped builds, duplicate IDs and record-count budgets.
The example uses an application-owned rectangle at document coordinates 1e12.
The independent consumer accesses the public facade without backend features.

All commands below passed locally on Windows using the pinned Rust toolchain:

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo doc --workspace --no-deps --locked
cargo check -p view --all-features --locked
cargo test --manifest-path tests/compatibility/facade-consumer/Cargo.toml --test canvas --locked --target-dir target/consumer
cargo run -p view-core --example canvas --locked
```

Execution log: `target/verification/m3b-01-checks.log` (generated, not committed).
The five canvas contract tests and one independent consumer test passed, as did
the workspace suite. Changed documentation passed local-link and fence checks;
consumer formatting and `git diff --check` also passed.
No registry dependencies, features, package boundaries or lockfiles changed.
Core remains platform-independent and unsafe-free. This increment does not claim
Linux/macOS execution, GPU canvas pixels, native accessibility publication,
physical input, capture/undo integration or T-08 completion. Runtime adapters
must map canvas IDs to owned nodes and validate revisions before action dispatch.
