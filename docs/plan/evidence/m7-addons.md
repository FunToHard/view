# M7 native-process add-on evidence

Date: 2026-10-22. Local Windows x64; canonical completion is in the
[implementation checklist](../implementation-checklist.md).

## Contract mapping

| Items | Concrete implementation / acceptance |
| --- | --- |
| M7-01–02 | Optional view-addons, validated manifest/protocol 1, actual native guest, bounded length-prefixed JSON pipes, separate capped stderr, negotiation/startup deadline; malformed/oversize/partial/unknown-field tests |
| M7-03 | Installed/negotiating/active/deactivating/failed/disabled/removed states, unique activation generations, process/worker receipts, cancellation, explicit restart and owner-unmount cleanup |
| M7-04 | Namespaced slot-owned runtime roots, flat bounded keyed snapshots/Put/Remove patches, coherent schema validation and base-revision Resync |
| M7-05–06 | Rust guest builders, retained normal Button/TextField engines, local editing and asynchronous value validation, registered typed component decoders, required-version rejection and optional fallback |
| M7-07–08 | Explicit host scopes and action schemas, monotonic document request IDs, application revision/range validation and undo; queue/node/property/byte/rate/pending/startup/event limits |
| M7-09 | Normal runtime IDs, semantic values/roles/actions, focus/modal/capture teardown and generation-qualified allowlisted automation descriptors; host CLI/MCP registration remains explicit |
| M7-10 | Actual child review panel, note editor and toolbar/menu actions; host-owned plain-document replacement and one undo; optional real host-rendered GPU image |
| M7-11 | Actual incompatible/malformed/oversized/crash/disconnect/flood/stale-generation children; restart/deactivation/disable/owner cancellation, atomic invalid patches, stale edits/validation and resource cleanup regressions |
| M7-12 | [Integration/trust/version/subset guide](../../guides/addons.md); no sandbox, JIT, DLL unload or native certification claim |

## Executed evidence

Fourteen all-feature contract tests passed, including actual subprocess exchanges. Local editing
performed grapheme deletion and synthetic preedit/commit before any guest round
trip; guest echo preserved both value and composition. This is shared-editor
behavior evidence, not physical Japanese IME evidence. Button key repeat emitted
one activation, while two distinct activations remained separate queued events.
Focus/capture, UI roots, pending events and command authority were removed during
deactivation; direct child exit and zero owned workers were observed.

The contributed field also ran the shared SingleLineV1 conformance runner with
no failed cases and with native certification correctly remaining false. Further
regressions preserve grouped typing undo, cancel composition on blur, retain
caret blink timing across paint, preserve accumulated clips and reject unsupported
nontranslation transforms. The default feature suite omits only the painter test.

The explicit review proof passed negotiation, normal host controls, one
revision-checked expected-range document edit, undo and cleanup. The GPU variant
also passed on NVIDIA GeForce RTX 3050 6GB Laptop GPU, DX12, driver
32.0.16.1714: 900 × 300 × 4 actual RGBA bytes, host-generated label/button/text
paint and no captured renderer validation errors. Artifact:
`target/verification/m7-review.bmp`; final execution log: `m7-gpu-final.log`.
This is an offscreen render, not a native window/compositor observation.

The first lifecycle test attempted to unmount a window root through the retained
child API and correctly received StructuralOwnership. The test now closes the
window through its public API and verifies owner cancellation. The first GPU
Clippy check requested the pinned toolchain's array-chunk API; that correction is
included. Failed attempts are not reported as passed gates.

The full Windows `scripts/verify.ps1` run passed (exit 0): required workspace
fmt/check/all-target Clippy/tests/docs, supported feature profiles, actual child
proof, independent consumer profiles, hidden Windows shell checks and dependency
inventory. Final focused formatting, all-target/all-feature add-on Clippy, tests
and documentation were rerun after the rendering/focus corrections. Logs live in
`target/verification/m7-tests-final.log`, `m7-consumer.log`, `m7-facade.log` and
`m7-full.log`. No M7 remote CI run is claimed.

## Scope and remaining platform coverage

No new registry package was introduced. The optional crate reuses existing
MIT OR Apache-2.0 framework/core/control/text/render packages and the already
qualified serde JSON stack. Default add-ons include the control/shaping graph;
GPU dependencies are behind render. The default facade remains core-only.
No packages, executables, diagnostics or infrastructure were published.

The shipped profile is flat translated rectangular label/button/plain-text rows,
host-compiled schema mappings and host-owned document services. It does not ship
rich-document layout, new native control bytecode, autonomous MCP registration,
OS process sandboxing or descendant containment. Explicit teardown while the
runtime remains available is required; dropping a process handle cannot promise
UI-tree teardown in a separately owned runtime. The app renders failure status
from the library's failed-state diagnostic in host-owned chrome.

Linux/macOS compile and subprocess tests are configured in normal CI, but no M7
remote jobs have been run in this change. Native AT, physical IME and broader
control certification retain their existing M2/M5 coverage gaps. The M7 tests
exercise shared behavior and boundaries without claiming those unrelated gates.
