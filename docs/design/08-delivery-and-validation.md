# Delivery and validation plan

Status: selected validation contracts. Revision: 0.4. Executable work ordering is maintained in the linked project plan; this document is not implementation authorization.

## Planning gate

The technical baseline selects the proof applications, Rust authoring model, 2D/3D ownership, automation contracts and dependency families. Windows 11 x64, GitHub Actions, and MIT OR Apache-2.0 are recorded delegated planning defaults. Exact dependency qualification and hardware provisioning are M0 tasks, not evidence already produced by design work.

No calendar estimates are assigned before scope, available engineering time, hardware, and CI budget are known. Milestone acceptance is behavioral rather than a count of widgets.

## Requirement traceability

| Requirement | Design location | Evidence / gate |
| --- | --- | --- |
| R-01 name and tagline | Root/index/charter | Documentation and later package metadata review |
| R-02 Rust and wgpu | Authoring/rendering/dependencies | Q-06/Q-10 resolved; M3 build and GPU evidence |
| R-03 dependency discipline | Feature discovery | Reviewed dependency inventory and feature graph |
| R-04 hybrid modes | Runtime/authoring | T-01 through T-06 |
| R-05 Flutter-like Rust | Authoring | Q-02 selected; compiler-tested API exercises and T-27 onboarding evidence |
| R-06 Windows plus Linux/macOS CI | Platform/delivery | T-11, T-13, T-15 and coverage manifests |
| R-07 native 2D/3D performance | Rendering/delivery | T-06 through T-09; named workload measurements |
| R-08 complex application integration | Charter/rendering/features/network/controls/add-ons | Workspace/storefront proofs, T-09/T-16/T-19/T-28 and optional-module T-32/T-34 gates |
| R-09 OS behavior | Platform | T-11 through T-13 with native E2E evidence |
| R-10 feature discovery | Feature inventory/decision baseline | C-01 through C-45 classified; native-process add-on backend selected |
| R-11 automation with/without MCP | Automation | T-14/T-22 through T-26; adapter parity and physical-input provenance |
| R-12 design before implementation | Index/decision register | Baseline finalized before owner-authorized bootstrap; runtime implementation remains in the backlog |
| R-13 developer-selected webview lifetime | Webview lifecycle / project plan | T-35 through T-38; disposal and hot-retention evidence |

Full MCP transport completion is separate from proving the shared API through a simpler external adapter. MCP is selected as an optional package in the preview and receives the same parity acceptance tests; protocol discovery alone cannot establish completion.

## Executable milestones

The authoritative work breakdown is the [project plan](../plan/README.md): M0 engineering bootstrap, M1 runtime/native shell, M2 text/controls, M3 graphics, M4 integrated proofs, M5 external tooling/E2E, M6 webviews, M7 process add-ons, and M8 hardening. Repository boundaries and release policy are in the [monorepo plan](../plan/repository-and-maintenance.md).

Shared editing behavior and profiles are implemented before claiming complete standard controls. Rendering/native portions of conformance finish when their infrastructure is ready; a missing portion cannot be counted as passed. Optional webview and process add-on modules have explicit proof/release gates rather than an unspecified future backend.

Automation begins in M1 and grows through every stage. Accessibility semantics and custom graphics are not postponed until a large widget library already exists. A second image-editing/IDE workflow is required before declaring the architecture broadly suitable, although its full product functionality is not a milestone deliverable. The native storefront fixture adds network/resource and optional server-described UI evidence; its scope can be exercised incrementally in M3/M4 without building a commerce backend or browser.

Web support remains deferred. T-20 is a future conditional feasibility check, not a current mandatory release job. A browser feature subset cannot silently lower native capabilities.

## CI platform matrix

| Job type | Windows | Linux | macOS |
| --- | --- | --- | --- |
| Format, compile, lint | Required | Required | Required |
| Runtime/layout/semantic tests | Required, executed | Required, executed | Required, executed |
| Platform adapter build/tests | Required | Required where implemented | Required where implemented |
| Offscreen GPU tests | Required on a provisioned compatible runner | Required on a provisioned compatible runner | Required on a provisioned compatible runner |
| Native window smoke | Required provisioned desktop session | Add suitable display/session fixture | Add suitable desktop session fixture |
| Full native integration/E2E | Windows release gate | Coverage tracked; not initial product parity claim | Coverage tracked; not initial product parity claim |
| Performance benchmark | Dedicated reference hardware | Expansion | Expansion |

Ordinary hosted runners must not be assumed to provide a usable graphics adapter or interactive desktop. Hosted runner specifications and custom/self-hosted GPU arrangements need verification before a pipeline is selected. [Hosted runners](https://docs.github.com/en/actions/reference/runners/github-hosted-runners), [Self-hosted runners](https://docs.github.com/en/actions/reference/runners/self-hosted-runners)

Every GPU job logs adapter, backend, driver where available, features/limits, and hardware-versus-software classification. Software rendering can provide correctness evidence but not native GPU performance evidence. If a required capability is absent, the report is blocked/uncovered, not a green GPU test. Optional exploratory jobs may skip with a visible reason; required release gates may not.

Run core tests on actual OS runners rather than relying exclusively on cross-compilation. Track Linux display backend coverage separately when relevant. CI runner labels are not proof of exact OS/driver contents; record the environment manifest.

Desktop mouse/keyboard tests are opt-in jobs on a provisioned unlocked session, serialized per desktop with a supervised stop/cleanup path. Ordinary headless jobs cannot advertise that coverage. Native input limitations, interrupted foreground ownership, or missing IME fixtures are environment failures/uncovered cases, not green tests. Crash tests run in disposable child processes, separate from the test supervisor.

## Acceptance scenarios

| ID | Scenario | Observable result |
| --- | --- | --- |
| T-01 | Reorder a keyed list while editing | Focus/selection remain with the same logical item |
| T-02 | Skip a clean immediate region | Its nodes/state remain; no rebuilding work is reported |
| T-03 | Trigger activation and force repeated measurement | One application action/effect |
| T-04 | Remove a captured/focused widget | Capture is cancelled and focus/IME state resolves predictably |
| T-05 | Complete background work after unmount | Stale result cannot mutate a reused node |
| T-06 | Run continuous 3D beside static inspector | Scene advances; UI build/layout counters remain stable |
| T-07 | Clip/resize/overlay a 3D viewport | Correct output, depth isolation, resource accounting, no CPU copy requirement |
| T-08 | Pan/zoom canvas and drag an object | Input transforms match pixels/semantics; capture and undo are correct |
| T-09 | Integrate into a host-owned loop/device | One event/presentation authority; no second device required |
| T-10 | Evict glyph/texture caches | Existing references rebuild or fail safely; no wrong glyph/content reuse |
| T-11 | Move window between DPI scales | Text, hit geometry, popovers, IME positions, and viewport sizes agree |
| T-12 | Minimize/restore and recreate device-bound state | No invalid zero-size work; recovery or actionable error |
| T-13 | Native frame/system interaction | Caption, snap, resize, activation, system menu remain native |
| T-14 | Automate selected object, edit, undo, capture | Consistent IDs/revisions and useful failure artifacts |
| T-15 | Run Linux/macOS CI | Executed tests and missing capabilities are separately reported |
| T-16 | Build a module outside core using public interfaces | Layout/input/paint/semantics/automation need no private patch |
| T-17 | Network search with reordered completions and branch changes | Newer results win under the selected policy; focused state resolves; build never starts duplicate requests |
| T-18 | Progress/stream updates under load and reconnect | Bounded queues, responsive input, explicit gap/revision handling, no lost required actions |
| T-19 | Native storefront remote sections and assets | Stable catalog identity/scroll, bounded loading, declared schema fallback, no privileged remote commands |
| T-20 | Future WASM capability fixture, if selected | Shared model/compatible 3D render works; missing provider/platform capabilities are explicit |
| T-21 | Panic/abort/fault/hang in disposable processes | Opt-in reports and host policy; no corrupt UI continuation; no default upload |
| T-22 | Application-integrated MCP/domain command workflow | Schemas and preconditions validated; owner-thread action routing; undo/cancel/result revisions and retry semantics verified |
| T-23 | Physical click through transforms/clips/DPI/overlays | Actual routed target and independent geometry/pixel evidence match; stale/occluded target fails safely |
| T-24 | Physical scrolling of nested/virtual content | Input-driven movement and boundary/chaining behavior observed; no hidden direct offset assignment |
| T-25 | Keyboard-first complex workflow | Tab/shortcut/modal/text/IME scopes, focus restoration, and undo verified in a real test window |
| T-26 | Passive mode, focus loss, and interrupted desktop test | Default tests do not seize input; physical runner stops and releases only its own held inputs |
| T-27 | Authoring from versioned onboarding pack | Representative new readers/models produce compilable, behavior-checked form/branch/control examples; errors and invented APIs recorded |
| T-28 | Consume Markdown preview from separate Cargo package | Only public compatible APIs; documented features/events/semantics; packaging and examples validated |
| T-29 | Standard text-control behavior profile | Delete/Backspace, selection, clipboard, history, commands and applicable native IME tests pass; uncovered cases explicit |
| T-30 | Text composition with external value/branch updates | No stale echo resetting selection/composition; documented cancel/replace/undo policy |
| T-31 | Visually different controls reuse one editing engine | Same mandatory behavior/profile tests pass in both styles |
| T-32 | Optional runtime add-on review panel | Host controls behave normally; typed document transaction is revision-checked and undoable |
| T-33 | Optional add-on negotiation/failure/deactivation | Incompatible schemas rejected; stale callbacks revoked; focus/subscriptions/resources cleaned up; limits verified |
| T-34 | Selected native-process extension boundary | Actual protocol/resource/trust guarantees verified; no false sandbox or safe-unload claim |
| T-35 | Dispose dedicated ephemeral webview after work | Host/native resources released, owned process exit and safe profile cleanup observed or explicit failure |
| T-36 | Dispose one webview in a shared session | Siblings remain functional; shared/persistent resources retained and reported accurately |
| T-37 | KeepHot hide/show and budget policy | State retained without intentional reload/suspension; focus restored correctly; retained resources counted |
| T-38 | Webview failure and shutdown races | Stale callbacks rejected; late exits do not delete a new session's profile; timeout/recovery visible |
| T-39 | Workspace/package/feature consumers | Each supported package/feature profile builds independently; no hidden workspace-only dependencies or accidental core tool stack |

Fault injection can test recovery state machines, but actual device loss and native lifecycle behavior also need supported-environment validation. Not every driver failure is recoverable.

Control conformance reports are versioned by package/profile/platform/options and distinguish pass/fail/uncovered/not-applicable. Add missing expected features to the profile and regression suite. Public examples should be compile-tested when implemented; current design sketches remain uncompiled proposals. Packaging/dry-run validation does not publish a crate, and an add-on proof does not require a marketplace or arbitrary scripting engine.

## Proposed workload fixtures

These sizes are initial test candidates, not performance promises:

- A hierarchy with 100,000 logical items and bounded realized rows.
- An animated scene with a fixed mesh/material workload and an idle property inspector.
- A canvas with a large logical image represented by tiles under a fixed memory budget.
- Two windows with different scales viewing the same document.
- An input burst and background-import workload competing with interactive editing.

Define actual scene complexity, tile sizes, text corpus, hardware, and quality settings before publishing numbers. Measure warm/cold behavior separately. Baselines include p50/p95/p99 CPU/GPU time, interaction latency, resident/peak memory, allocations, bytes uploaded, and idle wakeups. Establish regression tolerances from stable measurements.

## Delivery/CD policy

Initially publish only CI artifacts: reports, captures, traces, environment manifests, and example binaries where useful. Release packaging, signing, distribution, update mechanisms, and crate publication require an explicit release policy. CI green status does not by itself authorize publishing.

Dependency upgrades run the same proof workflows and backend matrix. Keep a tested lockfile for repository applications/tools and declared compatibility ranges for libraries. Archive enough failure evidence to reproduce a regression without permanently recording sensitive application content.
