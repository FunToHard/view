# Ordered implementation checklist

Baseline: design revision 0.4. Updated: 2026-10-07. This is the progress checklist
for the complete selected developer preview, followed by the deferred roadmap.
Creating this checklist does not execute its tasks.

## How to use this checklist

Work through phases 0–12 in order. Within each phase, work from top to bottom;
smaller fixes and tests belong with the behavior they validate. Phase prerequisites
identify the minimum technical dependencies if work is split later. No parallel
team or fixed delivery dates are assumed.

- `[x]` means the specific deliverable has evidence; `[ ]` means it is not complete.
- Keep stable item IDs. Append new items without renumbering existing work.
- When checking an item, append an evidence link: code/commit, test report, fixture,
  or reviewed document. If blocked, leave it unchecked and record the blocker.
- Separate implementation from certification. A missing runner is uncovered work,
  not a passed native/GPU test. Optional packages still need their preview gates.
- The [milestone plan](README.md) owns scope and milestone dependencies; the
  [decision register](../design/09-decisions-and-questions.md) owns architecture.
  This checklist is the canonical progress record. The [initial backlog](initial-backlog.md)
  supplies detailed BOOT/RUN work-package descriptions, not a second status board.
- Review public API examples during their owning phase, before stabilizing those
  APIs. Do not wait until all features exist to start testing, semantics or tooling.

The current external gate is **M0-10** (execute hosted CPU CI). Local qualification
is recorded in [M0 evidence](evidence/m0-qualification.md). Windows GPU compute/readback
and hidden native create/close passed; these do not certify the UI runtime, physical
input, text rendering or other operating systems. M0 remains open at its required
infrastructure/native-shell gates.

## Sequence at a glance

| Order | Work | Milestone | Prerequisite |
| --- | --- | --- | --- |
| 0 | Finish repository/dependency/CI foundation | M0 | Existing scaffold |
| 1 | Runtime identity, lifecycle and test harness | M1 | Phase 0 local build/dependency foundation |
| 2 | Layout, input, semantics and native shell | M1 | Phase 1 |
| 3 | Shared text editing and behavior profiles | M2 | Phase 2; shaping dependencies qualified |
| 4 | wgpu 2D renderer and composition | M3 | Phase 2; Phase 3 text interfaces |
| 5 | Standard controls and native accessibility | M2 completion | Phases 3–4 |
| 6 | 3D, canvas interaction and host embedding | M3 completion | Phase 4; inspector controls from Phase 5 |
| 7 | Public authoring, async resources and application proofs | M4 | Phases 5–6 |
| 8 | CLI/MCP, inspector and physical desktop E2E | M5 | Phase 7; extends harness started in Phase 1 |
| 9 | Optional WebView2 panels and both lifetimes | M6 | Phase 2 host; Phase 8 native tests |
| 10 | Optional native-process add-ons | M7 | Phase 7 public contracts; Phase 8 automation |
| 11 | Failure recovery, performance and platform hardening | M8 | Phases 1–10, incremental instrumentation throughout |
| 12 | Packaging, documentation and release qualification | M8 | Phase 11 and all applicable acceptance evidence |
| Later | Expansion and deferred capabilities | Outside preview | Separate scope amendment and its own gates |

M2 is deliberately split: build the edit engine before graphics, then certify
rendered controls after graphics exists. Minimal builders and observation APIs
start in M1; M4/M5 complete their public and external forms. M0's small-window
evidence is supplied by Phase 2 rather than requiring a throwaway second shell.
Unavailable infrastructure may leave a gate open while independent local work
continues; it never becomes an implicit coverage waiver.

## 0. Finish the engineering foundation — M0

Sources: [repository plan](repository-and-maintenance.md), BOOT-01 through BOOT-04.
Capabilities: C-45. Acceptance: foundation for T-15 and T-39.

- [x] **M0-01** Initialize Git on main and configure the requested origin. Evidence: bootstrap record; no commit or push was performed.
- [x] **M0-02** Create the initial Cargo workspace and unpublished facade; edition 2024, resolver 3 and shared lints. Evidence: [workspace manifest](../../Cargo.toml).
- [x] **M0-03** Pin Rust 1.98.1, declare the bootstrap minimum and generate the repository lockfile. Evidence: [toolchain](../../rust-toolchain.toml), [lockfile](../../Cargo.lock).
- [x] **M0-04** Add both licenses, ignore/line-ending/editor configuration, contributor instructions and agent guidance. Evidence: [contributor guide](../../CONTRIBUTING.md), [agent guide](../../AGENTS.md), repository root files.
- [x] **M0-05** Run local scaffold format/check/Clippy/test/rustdoc checks. Evidence: bootstrap record; zero behavior tests is explicitly recorded.
- [x] **M0-06** Qualify the initial core/platform dependencies, feature flags and compiler on Windows; record why each dependency is needed, its maintenance/license/target risks and transitive cost. Evidence: [reviewed platform probe](evidence/m0-06-platform.md) and [local rerun](evidence/m0-qualification.md).
- [x] **M0-07** Probe compatible wgpu/cosmic-text/glyphon/windows/winit/AccessKit versions plus lyon, Unicode segmentation and bytemuck; pin the coherent family centrally without adding unused dependencies to core. Evidence: [qualified versions and checks](evidence/m0-qualification.md), [inventory](evidence/dependencies-windows.json).
- [x] **M0-08** Define supported package/feature profiles and independent consumer-check commands; retain publish=false until release qualification. Evidence: [profiles](feature-profiles.md), separate consumer/lockfile and successful local build.
- [x] **M0-09** Add GitHub Actions format, lint, build, test and documentation jobs with locked dependencies and named Windows/Linux/macOS environments; preserve reports on failure. Evidence: [CPU workflow](../../.github/workflows/ci.yml), shared verification script passed locally; remote execution is M0-10.
- [ ] **M0-10** Execute CPU jobs on all three OSes and record environment/toolchain manifests. Add meaningful runtime tests as Phase 1 lands; an empty suite is bootstrap evidence only. Pending: publish reviewed workflow to the configured remote and execute CI.
- [x] **M0-11** Inventory owned fonts, images, text, protocol and network fixtures with licenses, versions/seeds and deterministic loading rules. Evidence: [current inventory](../../tests/fixtures/README.md), hash manifest and verification; absent asset families explicitly recorded.
- [ ] **M0-12** Specify and provision compatible offscreen GPU runners on all three OSes; record actual adapters/backends and software/hardware classification. Partial: local Windows DX12 passed; [runner contract](runner-contract.md) written, no Linux/macOS GPU machines registered.
- [ ] **M0-13** Specify and provision the Windows 11 x64 interactive test session, display/DPI configurations, en-US keyboard and Japanese IME; define isolation, serialization and interruption cleanup. Partial: [session contract](runner-contract.md) written; dedicated session/IME and physical-input evidence unavailable.
- [x] **M0-14** Establish a clean-checkout reproduction record once a baseline commit exists; verify lockfile tracking, ignored artifacts and contributor commands. Evidence: baseline 9db5751 passed the complete CPU verification in a separate clean checkout with build caches reused; [reproduction record](evidence/m0-qualification.md#clean-checkout-reproduction). Both lockfiles tracked; generated files ignored.
- [ ] **M0-15** Close the engineering gate with dependency/feature inventory, CPU execution reports, provisioning status and Phase 2 native-shell evidence. Keep unavailable coverage explicitly open. Blocked by M0-10/M0-12/M0-13 and the Phase 2 shell; hidden native lifecycle smoke is only partial evidence.

## 1. Build runtime ownership and the harness — M1

Sources: [runtime](../design/02-runtime-architecture.md), RUN-01/RUN-02/RUN-05.
Capabilities: C-01, C-02, C-03, C-07, C-10, C-37 foundations.
Acceptance: headless portions of T-01 through T-05.

- [ ] **M1A-01** Introduce view-core and view-testing when their first implementations land; define errors, revisions, window IDs and arena handles without backend/platform types.
- [ ] **M1A-02** Implement generational arena identity, ownership links and stale-handle rejection; test slot reuse and invalid parent references.
- [ ] **M1A-03** Implement mount/unmount and hidden/clipped/suspended distinctions; revoke owned subscriptions, tasks and callbacks on removal.
- [ ] **M1A-04** Implement component registry ownership, state storage and owned callbacks with dispatch-time model access; reject invalid lifetime/thread use.
- [ ] **M1A-05** Implement minimal descriptions, keyed reconciliation and retained mutation handles; match parent/key/type, reject duplicate keys and reset incompatible state.
- [ ] **M1A-06** Implement typed actions and ordered owner-thread transactions; effect execution occurs once, independently of pure build/measurement retries.
- [ ] **M1A-07** Implement immediate-region scheduling, retained mounts within regions and consumed-response sequences; skipped regions keep their previous nodes/state.
- [ ] **M1A-08** Implement build/layout/paint/composite/semantic/viewport dirty categories and dependency propagation; publish coherent commit revisions.
- [ ] **M1A-09** Implement demand scheduling, per-window coalescing and bounded queues; preserve required actions while replacing obsolete previews explicitly.
- [ ] **M1A-10** Add owner/request-generation checks and cancellation hooks for worker completions; test late completion after unmount/reuse.
- [ ] **M1A-11** Implement controlled clock, service fixtures, deterministic input queue and revision-tagged runtime snapshots.
- [ ] **M1A-12** Add bounded action/invalidation/lifecycle breadcrumbs and idle counters through an optional observer interface; no global telemetry or panic hook.
- [ ] **M1A-13** Run identity, reconciliation, action-once, skipped-region and stale-completion tests on all CPU platforms; record seeds and independent expected outcomes.
- [ ] **M1A-14** Review the small runtime API and dependency direction before exposing more facade types; demonstrate no continuous rebuild when idle.

## 2. Add geometry, input and the native shell — M1

Sources: [runtime](../design/02-runtime-architecture.md), [platform](../design/05-native-platform.md), RUN-03/RUN-04/RUN-06.
Capabilities: C-04, C-05, C-06, C-11, C-17 foundations. Acceptance: applicable T-01–T-05, T-11–T-13.

- [ ] **M1B-01** Define logical/physical/document geometry, units, finite-value validation and transform/precision rules; keep screen coordinates in the platform boundary.
- [ ] **M1B-02** Implement constraints, measure/arrange and row/column/stack/padding/alignment; cache only with valid dependencies and pure measurement.
- [ ] **M1B-03** Implement clip/transform chains, paint-order-aware hit geometry and custom layout/hit contracts with analytic fixtures.
- [ ] **M1B-04** Implement ordered pointer/keyboard events, hover/press/cancel, capture and routing against committed geometry; flush required commits before later geometry-dependent input.
- [ ] **M1B-05** Implement focus traversal, command/shortcut scopes, modal precedence and focus restoration; distinguish focus from scene selection.
- [ ] **M1B-06** Implement scroll state, nested boundary/chaining policy, keyboard scrolling, reveal-target and virtual-content realization contracts.
- [ ] **M1B-07** Implement semantic nodes, stable roles/names/values/actions and incremental snapshots; virtual/custom content declares its capabilities.
- [ ] **M1B-08** Introduce view-platform with winit and targeted Windows bindings; contain COM/FFI/thread affinity and handle ownership explicitly.
- [ ] **M1B-09** Create native decorated Windows windows and event pumping; implement open/resize/close/activation/minimize lifecycle without custom frame imitation.
- [ ] **M1B-10** Implement DPI/text-scale changes and client/logical/physical/screen conversion; cover monitor movement and negative virtual-desktop coordinates.
- [ ] **M1B-11** Connect native pointer/keyboard, capture-loss, cursor and text/IME service interfaces; register capability failures explicitly.
- [ ] **M1B-12** Implement multiple-window ownership, modal windows and close requests; preserve application documents while remounting view state, with explicit restoration snapshots.
- [ ] **M1B-13** Resolve focus/capture/composition on removal, deactivation and window destruction; route late events safely.
- [ ] **M1B-14** Run the minimal inspectable native app: queued action, keyed identity, coherent snapshot, unmount and idle behavior. Keep rendered-text/IME certification open for later phases.
- [ ] **M1B-15** Review M1 contracts and execute analytic layout/input/semantic tests plus native frame smoke; record Linux/macOS adapter limits separately.

## 3. Implement text services and shared editing — M2

Sources: [control profiles](../design/13-controls-and-conformance.md).
Capabilities: C-16, C-42; C-04 text measurement. Acceptance: headless T-29–T-31, T-01 editing extension.

- [ ] **M2A-01** Introduce view-text; define text indexing, revisions, shaping/measurement and editor-session interfaces before exposing backend types.
- [ ] **M2A-02** Integrate font discovery/owned fixtures, fallback, shaping, line layout and caches; define font/DPI invalidation and glyph lifetime rules.
- [ ] **M2A-03** Implement grapheme/word boundaries, bidi-aware logical/visual caret policy and shaped-run hit mapping; test mixed scripts, ligatures and emoji.
- [ ] **M2A-04** Define versioned SingleLineText v1 and plain-multiline behavior matrices with mandatory/optional/not-applicable cases and independent expected edits.
- [ ] **M2A-05** Implement insertion, selected-range replacement, Backspace/Delete, word deletion and empty/boundary/length-limit behavior.
- [ ] **M2A-06** Implement caret movement, Home/End/word movement, selection anchor/direction, Shift extension, select-all and pointer/word selection.
- [ ] **M2A-07** Implement field undo/redo grouping for typing, paste and composition; define coordination with application document undo scopes.
- [ ] **M2A-08** Implement revision-aware controlled values, validation and external replacement policies without stale echoes overwriting selection/composition.
- [ ] **M2A-09** Implement read-only/disabled/placeholder behavior, caret blink through the test clock, horizontal scrolling and drag-selection caret reveal.
- [ ] **M2A-10** Integrate clipboard copy/cut/paste and single-line newline policy; use deterministic service fixtures and actual native clipboard checks.
- [ ] **M2A-11** Integrate IME preedit/commit/cancel, composition replacement and candidate-position reporting; defer rendered-position certification to Phases 5/8.
- [ ] **M2A-12** Extend plain multiline behavior with line navigation, vertical movement, wrapping, newline/Tab policy and scrolling.
- [ ] **M2A-13** Build reusable profile adapters/reports; property-test valid selections and undo/edit sequences alongside explicit behavior fixtures.
- [ ] **M2A-14** Pass headless editing/profile tests and record native/render cases still uncovered; do not certify a standard text control yet.

## 4. Implement the wgpu 2D renderer — M3

Sources: [rendering](../design/04-rendering-and-interop.md).
Capabilities: C-08, C-09, C-12 foundations. Acceptance: T-07, T-10–T-12 at applicable layers.

- [ ] **M3A-01** Introduce view-wgpu; negotiate adapter/device features, limits, formats and error reporting without leaking wgpu into core controls.
- [ ] **M3A-02** Implement framework-owned surfaces and offscreen targets, acquisition/presentation lifecycle, resize coalescing and zero-size/occlusion handling.
- [ ] **M3A-03** Define paint/layer records and prepared snapshots with content revisions; preserve draw order and separate commit/submission/presentation observations.
- [ ] **M3A-04** Implement WGSL pipelines and checked host data layouts for quads, images and selected primitives.
- [ ] **M3A-05** Implement paths/fills/strokes through lyon, transforms, clipping and hit/paint coordinate agreement.
- [ ] **M3A-06** Integrate glyphon text drawing with cosmic-text; render caret/selection/preedit while preserving ordering and clipping.
- [ ] **M3A-07** Implement explicit SDR transfer/alpha conventions, premultiplied composition and isolated group opacity; preserve color metadata.
- [ ] **M3A-08** Implement texture/buffer/glyph/geometry caches, budget accounting, eviction invalidation and submission-aware retirement.
- [ ] **M3A-09** Implement provider negotiate/prepare/encode/compose/retire/resize/recreate phases and pass dependency validation; diagnose stale handles, cycles and undeclared writers.
- [ ] **M3A-10** Implement offscreen and compatible existing-texture contributions with device/usage/format/sample/alpha/origin checks and resolve requirements.
- [ ] **M3A-11** Implement the scoped direct-contribution path for compatible attachments/order; reject unsupported use or select the documented offscreen fallback.
- [ ] **M3A-12** Implement device-generation invalidation, surface failure handling and provider recreation/error callbacks; avoid unconditional recovery promises.
- [ ] **M3A-13** Add real offscreen GPU fixtures for shapes/text/clips/blending/layers/cache eviction and revision-tagged readback; record comparison tolerances.
- [ ] **M3A-14** Execute graphics fixtures on provisioned Windows/Linux/macOS backends; classify software rendering separately and keep missing required coverage open.

## 5. Complete standard controls and accessibility — M2 completion

Sources: [control profiles](../design/13-controls-and-conformance.md), [platform](../design/05-native-platform.md).
Capabilities: C-06, C-14, C-15, C-16, C-20, C-42. Acceptance: T-29–T-31 and applicable T-01/T-11/T-16.

- [ ] **M2B-01** Introduce view-controls and reusable pressable/focusable/range/selection/popup/scroll behaviors, separate from decoration.
- [ ] **M2B-02** Implement typed themes, inherited/local style resolution, interaction states and versioned resource reload with precise invalidation.
- [ ] **M2B-03** Implement text labels, images and buttons; include keyboard activation, press cancellation, disabled state and semantics.
- [ ] **M2B-04** Implement toggle/checkbox/radio controls with explicit mixed/group/exclusivity and keyboard policies.
- [ ] **M2B-05** Compose TextInput and plain TextArea from the shared engine; implement selection/caret/preedit visuals, focus indication and native command/context-menu routing.
- [ ] **M2B-06** Implement numeric fields/sliders with bounds, precision, intermediate-invalid text, keyboard steps and commit/cancel behavior.
- [ ] **M2B-07** Implement scrollbars/views and virtualized list/tree selection/navigation with stable identity, bounded realization and virtual focus.
- [ ] **M2B-08** Implement split panes, popup/menu/dialog primitives and command-palette composition needed by proofs; cover screen edges, nesting, Escape and focus return.
- [ ] **M2B-09** Integrate AccessKit semantic updates and action routing; expose native focus/value/selection/text services where supported.
- [ ] **M2B-10** Complete clipboard, drag-and-drop, scoped file-dialog and shell-activation service adapters required by the proofs; application actions own external effects.
- [ ] **M2B-11** Test actual IME candidate/preedit placement across clipping, scrolling and DPI; test composition with external value updates and unmount.
- [ ] **M2B-12** Build a visually distinct text field using the same editor session; run identical mandatory profiles for both presentations.
- [ ] **M2B-13** Build the control gallery with states and behavior matrices; include lifecycle, read-only-plus-copy, selection-plus-IME and virtualization-plus-focus cases.
- [ ] **M2B-14** Publish preliminary per-control/profile/platform reports; close render/native checks now available and leave physical E2E certification to Phase 8.

This is the preview catalogue needed by the selected proofs. General tables,
password fields and rich document editors are separate profiles to scope in the
expansion roadmap, not undeclared promises attached to TextInput.

## 6. Prove canvas, 3D and host integration — M3 completion

Sources: [rendering](../design/04-rendering-and-interop.md).
Capabilities: C-08, C-09, C-12, C-13, C-21; initial C-24 boundaries.
Acceptance: T-06–T-12.

- [ ] **M3B-01** Implement Canvas2D document/view transforms, pan/zoom, hit/semantic regions and retained/on-demand draw records with explicit precision conversion.
- [ ] **M3B-02** Implement the minimal camera/mesh/transform/basic-material/depth scene provider; document projection, handedness, depth and unit conventions.
- [ ] **M3B-03** Implement picking with scene/camera/viewport revisions; discard stale asynchronous results and expose named object actions separately from pointer picking.
- [ ] **M3B-04** Implement UI/gizmo overlays, pointer capture and drag preview/commit/cancel; route one completed gesture into one app-owned undo transaction.
- [ ] **M3B-05** Schedule independent continuous viewport frames beside idle retained/immediate inspectors; implement visibility and frame-demand policies.
- [ ] **M3B-06** Implement host-owned loop/device/target integration with one event/submission/presentation authority and explicit native/accessibility service handoff.
- [ ] **M3B-07** Publish the wgpu compatibility boundary and implement a host-embedding example without a second loop/device or mandatory scene translation.
- [ ] **M3B-08** Exercise multiple clipped/resized viewports, cache/resource retirement, DPI movement and device recreation; demonstrate no required CPU composition copy.
- [ ] **M3B-09** Build a bounded tiled-canvas/brush-preview fixture with app-owned document coordinates and undo; pressure/tilt is capability-reported and needs its own hardware evidence if advertised.
- [ ] **M3B-10** Record UI/scene/composition counters separately; pass the mixed static-UI/animated-scene and host-owned rendering scenarios.

## 7. Finish authoring, network UI and application proofs — M4

Sources: [authoring](../design/03-rust-authoring-api.md), [network UI](../design/11-network-ui-and-web.md), [community controls](../design/13-controls-and-conformance.md).
Capabilities: C-02, C-03, C-14–C-17, C-20, C-21, C-34, C-35, C-40, C-41.
Acceptance: T-16–T-19, T-27/T-28; integrated T-01–T-09.

- [ ] **M4-01** Stabilize canonical builders, property structs, fixed-child tuples, keyed iterators and Element branches; review ownership, diagnostics, code size and compile ergonomics.
- [ ] **M4-02** Expose supported retained handles/immediate regions and mixed-mode composition through the facade; test structural ownership and skipped-region state.
- [ ] **M4-03** Implement Resource<T> state/subscriptions, versioned completions and application/executor adapters without starting effects in build/layout.
- [ ] **M4-04** Implement request identity, cancellation/stale-response checks, refresh-over-existing-content, retry/cache policies and explicit reconnect behavior.
- [ ] **M4-05** Implement bounded progress/stream handling, image/asset loading, cache eviction and opt-in codec features; preserve required events under backpressure.
- [ ] **M4-06** Implement loading/ready/error/empty branches with documented local-state preservation, focus fallback and application-owned persistent state.
- [ ] **M4-07** Implement bounded server-described sections using a local versioned control/action registry, stable keys, schema fallback and resource limits; no remote executable code.
- [ ] **M4-08** Build the engine-editor proof: hierarchy, inspector, scene selection/gizmo, undo and detached window using public interfaces.
- [ ] **M4-09** Build the native storefront fixture: navigation/search/catalog/remote sections/images/progress/offline state; inject reordered replies, refresh and reconnect.
- [ ] **M4-10** Complete a second image-editor or IDE workflow using shared documents, keyboard navigation and async work; reuse the canvas fixture or editor services without building a full product.
- [ ] **M4-11** Publish public control composition/layout/paint/semantics/event contracts and version/feature requirements; keep ordinary controls independent of runtime add-ons.
- [ ] **M4-12** Build a separate Markdown preview control package with explicit syntax, selection/link/image/update/semantic policies and app-owned link actions.
- [ ] **M4-13** Consume that package in an independent Cargo project using public APIs only; test package/feature isolation and required conformance adapters.
- [ ] **M4-14** Create compiler-checked minimal/form/keyed-list/async-branch/two-window/immediate/custom-control/viewport examples and a versioned API quick reference.
- [ ] **M4-15** Evaluate onboarding with the reference pack and record compile/behavior failures, invented API calls and correction needs; improve docs without claiming universal AI reliability.
- [ ] **M4-16** Pass integrated proof and network race scenarios; document missing capability contracts instead of patching private framework internals.

## 8. Complete external automation and physical E2E — M5

Sources: [automation](../design/06-automation-and-tooling.md).
Capabilities: C-10, C-18, C-19, C-38–C-40.
Acceptance: T-14, T-22–T-26; physical/native portions of T-29–T-31.

- [ ] **M5-01** Introduce view-automation over the existing harness; version command/query schemas, capabilities, session/window IDs and structured unsupported errors.
- [ ] **M5-02** Implement role/name/test-ID/ancestor selectors with ambiguity errors, target-generation checks and explicit stale-revision policies.
- [ ] **M5-03** Expose coherent tree/layout/hit/focus/capture/semantic/resource snapshots, predicate waits, controlled time and commit/presentation waits with bounded diagnostics.
- [ ] **M5-04** Register typed application commands with argument/result schemas, read/mutate scopes, revision preconditions and owner-thread action dispatch.
- [ ] **M5-05** Implement long-operation IDs/progress/cancellation and session-local request deduplication; report unknown outcomes after ambiguous disconnects rather than promising exactly-once effects.
- [ ] **M5-06** Implement the CLI/local attachment adapter with explicitly enabled session access; keep protocol messages separate from diagnostic output.
- [ ] **M5-07** Qualify rmcp and implement the optional stdio MCP adapter; expose framework/domain discovery and command parity without a default eval tool.
- [ ] **M5-08** Build the tree/layout inspector, clip/hit overlays, why-updated view and resource counters through the same public tool model.
- [ ] **M5-09** Implement revision-tagged image capture, bounded failure bundles and replay of controlled time/assets/worker/network boundaries; label capture provenance and overhead.
- [ ] **M5-10** Implement the opt-in Windows desktop input session: foreground/process checks, one owner, stop mechanism, timeouts and cleanup of only runner-held inputs.
- [ ] **M5-11** Implement target realization and actionable-point selection through clips/transforms/occlusion/DPI/screen conversion; revalidate before input and fail on non-actionable targets.
- [ ] **M5-12** Implement actual mouse/key/wheel input and observed delivery/results; distinguish semantic ensure-visible from physical scrolling and wheel tests from touchpad coverage.
- [ ] **M5-13** Run independent hit/scroll fixtures for nested/virtual content, overlays, disabled/offscreen targets, mixed DPI, window movement and layout races.
- [ ] **M5-14** Run keyboard-first flows: Tab/reverse Tab, arrows/pages, modifiers/shortcuts, modal containment/return, Escape, text deletion/selection and undo.
- [ ] **M5-15** Execute named en-US keyboard and Japanese IME scenarios; distinguish physical keys, Unicode insertion and native composition coverage.
- [ ] **M5-16** Compare Rust/CLI/MCP discovery, semantic actions, errors, async results and undo; separately run pointer-select/gizmo-drag/inspector-edit/capture workflows.
- [ ] **M5-17** Test focus loss, target closure, interrupted runs, inaccessible targets and passive default mode; publish artifacts for failed/uncovered cases.
- [ ] **M5-18** Close standard-control native conformance gaps and publish actual platform/profile coverage, including accessibility observations and independent geometry/pixel checks.

## 9. Implement optional webview panels — M6

Source: [webview lifecycle](../design/15-webview-lifecycle.md).
Capabilities: C-28, C-44. Acceptance: T-35–T-38.

- [ ] **M6-01** Qualify webview2-com/runtime compatibility and introduce view-webview as an optional Windows adapter; detect runtime availability with a documented Evergreen recovery route.
- [ ] **M6-02** Implement rectangular native hosting, placement/z-order, DPI, resize, focus/keyboard/IME and accessibility handoff; reject unsupported transformed composition.
- [ ] **M6-03** Implement explicit panel/controller/session/process-group/profile ownership; default to a dedicated ephemeral profile and require opt-in sharing/persistence.
- [ ] **M6-04** Implement typed origin/frame/session-checked bridge permissions and app-owned navigation/download/devtools policy; revoke grants on relevant navigation or disposal.
- [ ] **M6-05** Implement app-declared finish and DisposeAfterUse: stop new work, revoke callbacks, cancel tasks, resolve focus/IME, remove subscriptions, Close controller and release references on the owning thread.
- [ ] **M6-06** Implement asynchronous dedicated-session exit observation and safe ephemeral-profile removal with session-generation checks; never delete persistent/shared data or kill a shared group.
- [ ] **M6-07** Return cleanup receipts for released/exited/removed/shared-retained/pending/failed scopes; bound pending sessions and make timeout/retry/startup reconciliation observable.
- [ ] **M6-08** Implement KeepHot hide/detach/reuse with retained state, no deliberate reload/suspension and exclusion from visible focus/hit targets.
- [ ] **M6-09** Implement app-owned hot leases, count/memory/time budgets and explicit eviction decisions; report browser throttling, crashes and recreated instances honestly.
- [ ] **M6-10** Test repeated disposal, shared-session siblings, profile locks, late callbacks, recreate-during-cleanup and shutdown; verify measured owned-resource release.
- [ ] **M6-11** Test hot state reuse/budgets and native keyboard/IME/scroll/DPI reattachment; run lifecycle mocks on Linux/macOS without claiming native WebView2 coverage.
- [ ] **M6-12** Ship both policies in the panel example with cleanup/retention instrumentation and a documented native-composition limitation matrix.

## 10. Implement optional runtime add-ons — M7

Source: [runtime add-ons](../design/14-runtime-addons.md).
Capabilities: C-31, C-43. Acceptance: T-32–T-34.

- [ ] **M7-01** Introduce view-addons and a native Rust child-process example; define manifests, IDs, protocol versions and trusted-code/OS-privilege boundaries.
- [ ] **M7-02** Implement bounded length-delimited JSON on dedicated stdio, separate diagnostics, negotiation, startup timeout and malformed/oversized message rejection.
- [ ] **M7-03** Implement install/activate/deactivate/fail/disable lifecycle, instance generations, process exit handling and owned-request/resource cleanup.
- [ ] **M7-04** Implement namespaced side-panel/menu/toolbar slots and bounded host-rendered UI snapshots/patches with base revisions and resynchronization.
- [ ] **M7-05** Implement guest builders/SDK and standard-control event/value mapping; keep local editing responsive without per-keystroke blocking IPC.
- [ ] **M7-06** Implement host-registered component schemas, required/optional capability fallback and explicit rejection of unknown native control implementations.
- [ ] **M7-07** Implement application-granted document snapshots/actions with validation, preconditions and undo transactions; never expose unrestricted mutable tree/document access.
- [ ] **M7-08** Enforce host API scopes, resource/message/rate limits, cancellation and timeout behavior; distinguish API restrictions from OS sandboxing.
- [ ] **M7-09** Integrate contributed semantics, focus, standard-control conformance and host-authorized automation command discovery.
- [ ] **M7-10** Build the Word-like review-panel proof with one revision-checked, undoable document edit; the application owns its document model.
- [ ] **M7-11** Test incompatibility, malformed traffic, stale edits, disconnect/crash/restart, deactivation and late replies; verify no orphaned UI, commands or focus ownership.
- [ ] **M7-12** Document installation/update/restart/trust/version support and the shipped component subset; no JIT, DLL unload or sandbox claims beyond actual implementation.

## 11. Harden diagnostics, recovery and performance — M8

Sources: [diagnostics](../design/12-diagnostics-and-crash-policy.md), [validation](../design/08-delivery-and-validation.md).
Capabilities: C-07, C-11, C-17, C-37, C-45; all feature lifecycles.
Acceptance: T-13/T-15/T-21 plus regression suites from every preceding phase.

- [ ] **M8A-01** Complete the bounded diagnostic envelope: versions/capabilities/revisions/breadcrumbs with host-controlled redaction, storage and retention.
- [ ] **M8A-02** Implement opt-in panic/reporting hooks that respect host ownership and avoid resuming inconsistent UI state; no automatic upload or implicit global replacement.
- [ ] **M8A-03** Add compatible Windows WER collection integration/guidance, symbol/build identification and report handling; state unsupported fault/hang cases explicitly.
- [ ] **M8A-04** Demonstrate healthy-operation application checkpoint/journal recovery independently of crash handling; the framework does not invent a universal document format.
- [ ] **M8A-05** Test panic/abort/native fault/hang in disposable processes under an independent supervisor, preserving host reporting policy and recovery artifacts.
- [ ] **M8A-06** Sweep Windows caption/snap/system menu/move/resize/activation/modal/DPI/text-scale/clipboard/IME/accessibility behavior on the supported product environment.
- [ ] **M8A-07** Exercise surface/device/process failure, minimize/restore, sleep/resume, display changes and resource exhaustion; verify bounded cleanup or actionable failure.
- [ ] **M8A-08** Define named workloads, hardware/drivers, font/assets, resolutions, scene sizes and quality settings; separate correctness and performance environments.
- [ ] **M8A-09** Measure cold/warm p50/p95/p99 UI/scene/composition time, input latency, idle wakeups, memory/allocations, upload volume and webview hot/cold cost.
- [ ] **M8A-10** Measure virtualized large hierarchies, sustained resizing, multiple windows/viewports, tiled canvas and background-import pressure; enforce bounded queues/resource growth.
- [ ] **M8A-11** Establish repeatable numeric regression budgets from evidence and optimize actual bottlenecks; do not require speculative render threads or partial swapchain redraw.
- [ ] **M8A-12** Run lifecycle stress/property/fuzz scenarios at native/protocol/editor boundaries; review unsafe code and dependency vulnerabilities/maintenance at release qualification.
- [ ] **M8A-13** Execute the final Windows/Linux/macOS CPU and provisioned GPU matrix plus Windows physical E2E; publish passed/failed/uncovered/not-applicable manifests.

## 12. Qualify packages and the developer preview — M8

Sources: [repository/release policy](repository-and-maintenance.md), [validation](../design/08-delivery-and-validation.md).
Capabilities: C-40, C-41, C-45 and the whole selected preview. Acceptance: T-39 and all applicable T-* gates.

- [ ] **M8B-01** Audit the crate graph: no reverse dependency from core into platform/GPU/tools; no mandatory webview/MCP/add-on stack for a minimal app.
- [ ] **M8B-02** Verify minimal/default/supported-full profiles, each public package and independent consumers; test the declared compiler minimum and pinned repository build.
- [ ] **M8B-03** Verify public crate-name availability, dependency-order package contents, path-plus-version declarations, licenses/attributions and reproducible artifacts before enabling publication.
- [ ] **M8B-04** Finalize coordinated pre-1.0 versions, protocol compatibility, changelog, migration/deprecation notes and supported-platform/feature matrices.
- [ ] **M8B-05** Complete setup, architecture, authoring, control SDK, embedding, network UI, automation, webview and add-on guides with compile-tested versioned examples.
- [ ] **M8B-06** Produce the gallery/editor/storefront/Markdown/host-embedding/add-on/webview proof artifacts with reproducible commands and declared limitations.
- [ ] **M8B-07** Audit every R-01–R-13 requirement and C-01–C-45 capability against implementation or an explicit deferred entry; close all applicable T-01–T-39 gates except deferred T-20.
- [ ] **M8B-08** Review remaining failures and uncovered required coverage; fix or explicitly amend release scope rather than relabeling a skipped gate as passed.
- [ ] **M8B-09** Produce a release candidate from a clean reviewed revision with package dry runs, reports, environment manifests and regression baselines.
- [ ] **M8B-10** Execute separately requested publication/distribution operations in dependency order; verify delivered artifacts, symbols and versions. This checklist alone does not trigger pushing, package publication or installer signing.

## Later: expansion roadmap, excluded from preview completion

These unchecked items are deliberate scope exclusions, not prerequisites for the
developer preview. Ordering below expresses technical dependencies, not a promise
to implement every option. Each needs a decision amendment, bounded scope and its
own acceptance plan before becoming active work.

- [ ] **F-01** Expand control profiles/catalogue: general tables, password entry, richer selectable documents and specialized controls after the shared behavior/conformance SDK is proven.
- [ ] **F-02** Add full docking/tabs/tear-off persistence after multi-window/focus/restoration contracts are proven (C-22).
- [ ] **F-03** Add advanced large-buffer editor services, rich documents, pagination and collaboration profiles; preserve baseline bidi correctness already required by text controls (C-23).
- [ ] **F-04** Expand tiled-image processing, brush/pen tooling, formats and document effects beyond the proof fixture, with hardware evidence for advanced input (C-24).
- [ ] **F-05** Expand scene/material/asset modules, PBR/shadows/animation or integrations according to actual application needs; keep ECS/physics optional (C-25).
- [ ] **F-06** Expand advanced masks/effects/export and professional color/HDR pipelines after resource/color metadata and output requirements are validated (C-26, C-29).
- [ ] **F-07** Add native title-bar content only with complete native behavior certification (C-27).
- [ ] **F-08** Add arbitrary native GPU resource sharing and transformed/native webview composition through backend-specific ownership/synchronization tests (C-28 expansion, C-30).
- [ ] **F-09** Evaluate WASM interpreter or C-compatible DLL add-on backends separately; qualify ABI/runtime/trust/unload behavior before selecting either (C-31 expansion).
- [ ] **F-10** Evaluate stateful Rust hot reload only with explicit migration/lifetime/ABI contracts; resource reload and fast restart remain the baseline (C-32).
- [ ] **F-11** Evaluate web/WASM delivery using a capability subset and T-20 without capping native features; browser 3D support alone is insufficient (C-36).
- [ ] **F-12** Evaluate mobile, ARM64, Windows 10 and full Linux/macOS product support separately with native input/accessibility/distribution evidence (C-33 and platform expansions).
- [ ] **F-13** Evaluate custom crash dump helpers/watchdogs beyond selected WER/reporting integration and separately define their process ownership and recovery limits (C-37 expansion).
- [ ] **F-14** Evaluate optional macro syntax, typed branch optimizations, CSS/grid layout, render threading and damage redraw only when evidence justifies additional maintenance cost.

## Completion ledger

For each finished item append evidence next to its checkbox. For longer evidence,
add a row here rather than duplicating status in another plan.

| Item(s) | Evidence | Coverage / limitation |
| --- | --- | --- |
| M0-01–M0-05 | [Bootstrap record](initial-backlog.md#bootstrap-evidence--2026-10-07) and linked repository files | Local Windows scaffold only; zero behavior tests; no commits/pushes or remote CI execution during setup |
| M0-06–M0-09, M0-11, M0-14 | [M0 qualification](evidence/m0-qualification.md) | Six local tests, hardware GPU readback, hidden native lifecycle and committed clean-checkout CPU verification; no remote OS or physical-input certification |

All later entries remain unchecked until their concrete deliverables and applicable
verification exist. A milestone is complete only when its required implementation
and evidence items are complete; a percentage of checked boxes is not a release gate.
