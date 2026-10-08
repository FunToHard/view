# M1A headless runtime evidence

Date: 2026-10-08. Executed locally on Windows x64 with pinned Rust 1.98.1.
The canonical [checklist](../implementation-checklist.md) remains the progress
record. M0 continuation was explicitly skipped by the owner; its open gates are
unchanged. This report does not close native T-01/T-04 or the complete M1 milestone.

## Implementation and acceptance mapping

| Items | Implemented contract | Evidence |
| --- | --- | --- |
| M1A-02 | Process-unique arena namespaces, generational slots, stale/foreign rejection and parent ownership | [Arena](../../../crates/view-core/src/arena.rs); reuse, invalid-parent and fixed-seed tests |
| M1A-03/04 | Retained ownership, visibility distinctions, owned typed state/handlers, subtree/window/drop cleanup and owner-thread boundary | Lifecycle tests; compile-fail lifetime/thread doctests |
| M1A-05 | Parent/key/type/structure reconciliation, duplicate rejection, retained descendant preservation | Keyed reorder/type/structure/duplicate tests |
| M1A-06/07 | Ordered typed actions, once-only effects, region responses, skipped-region preservation | Measurement/action-once, failed-build retry, child response and suspended-buffer tests |
| M1A-08/09 | Conservative dirty dependencies, coherent structural commits, separate presentation/viewport demand, bounded required/preview queues | Window isolation, presentation validation, viewport/idle and queue retry tests |
| M1A-10 | Owner/request generations, cancellation flags/hooks and dispatch-time revalidation | Supersession, duplicate completion, cancellation after enqueue, slot reuse and worker-thread token tests |
| M1A-11 | Virtual clock, deterministic input/completion queues, FIFO service fixtures and revision-tagged tree observation | [Driver tests](../../../crates/view-testing/tests/driver.rs) and headless example |
| M1A-12 | Optional bounded action/invalidation/lifecycle buffer and idle counters | Trace bound/drop tests; idle example |
| M1A-14 | API/dependency review and independent consumer | Review below; zero-registry consumer graphs and executable example |

The [runtime contract suite](../../../crates/view-core/tests/runtime_contracts.rs)
has 18 scenarios. The arena unit test exercises generation exhaustion by setting
a slot to its last incarnation, then verifying actual retirement. The separate
three foundation tests retain their original value-contract coverage. Three
harness scenarios validate time/order/backpressure, stale completion replay and
fixture/clock errors. Three compile-fail doctests cover window type separation,
runtime thread confinement and borrowed-state lifetime rejection.

The arena stress scenario uses seed `0x5eed_cafe`, 2,048 operations and a separate
live-value map. Other scenarios use fixed explicit action/order/state expectations,
not randomized scheduling or wall-clock sleeps. No test seizes native input.

## Executed commands and results

`pwsh -File scripts/verify.ps1` passed the required workspace formatting,
all-target locked check, strict Clippy, tests and rustdoc. It also passed:

- Isolated core and testing package suites with default features disabled.
- Facade minimal/all-current-feature checks (no production feature flags exist).
- Headless example: `model=5, builds=2, commits=2, idle_polls=100`.
- Independent facade consumer compile/run and its optional harness integration
  test in a separate workspace/lockfile; consumer formatting.
- Existing owned fixture hashes, dependency inventory and Windows binding probe.

The workspace executed 31 unit/integration tests (25 core/harness, six existing
qualification tests) and three compile-fail doctests. The independent harness
consumer passed one additional test. The 100 idle steps in the example caused
zero additional builds/commits; this is a behavior assertion, not a performance
benchmark or measured OS idle-wakeup result.

Both `cargo tree --manifest-path tests/compatibility/facade-consumer/Cargo.toml
--locked` and its `--features harness` variant were inspected. Default graph:
consumer → view → view-core. Harness graph additionally includes view-testing →
view-core. Zero registry dependencies in either; the workspace qualification
stack retains 124 registry packages. Registry versions were not changed.

Raw local logs/environment inventory are under ignored `target/verification/`.
Affected Markdown local links/anchors/fences and `git diff --check` were checked.

## M1A-14 API and dependency review

- No backend/platform/tool dependencies flow into core. It uses std only;
  facade and testing depend separately on core. All packages forbid unsafe code,
  inherit the pinned toolchain/edition/lints and remain unpublished.
- `Runtime<M, A>` borrows the model at dispatch and stores owned state/handlers.
  Its !Send/!Sync boundary is checked by compilation; completion tokens cross an
  actual worker thread in a test. No global application model/executor is added.
- Single structural authority and retained descendants are covered through the
  public API. Raw handle reconstruction does not bypass arena checks.
- Once-only effects are defined for accepted in-process actions. No rollback,
  panic recovery or network exactly-once guarantee is claimed. Required actions
  are returned on saturation; explicit previews return their replaced value.
- The host handles response backpressure explicitly. Suspended regions keep
  pending state; continuous viewports do not schedule UI builds. An idle flush
  has no build/commit work, with only explicit diagnostic idle polling counted.
- Current snapshots cover structural runtime state only. Native focus/capture,
  IME, geometry and semantics remain M1B/later work. Conservative window-wide
  layout propagation and simple keyed matching have no performance certification.

Contracts and runnable entry points are in the
[headless guide](../../guides/headless-runtime.md). This is a local implementation
review, not an independent third-party audit.

## Coverage remaining

M1A-13 remains open: these new tests have not executed on Linux/macOS. The existing
three-OS CPU workflow runs the updated verification script when this revision is
made available to CI; historical M0 runs cannot certify these changes. No new
push, remote CI result, native window/GPU or physical-input execution is claimed.
M0 remains explicitly skipped/open per the owner, including GPU/interactive
provisioning and its dependency on the Phase 2 native shell.
