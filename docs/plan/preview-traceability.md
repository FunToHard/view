# Preview requirement, capability and scenario audit

Audit date: 2026-10-25. This maps the accepted design to implementation, evidence
and remaining qualification. It does not check off gates: the
[implementation checklist](implementation-checklist.md) is the sole progress board.
Evidence links identify executed layers and their limitations. Changes after an
evidence report require requalification. No entire scenario is certified merely
because code or an example exists.

## Requirements

| ID | Implementation / scope | Evidence or remaining gate |
| --- | --- | --- |
| R-01 | Name/tagline and workspace metadata | Root README; registry names still unqualified |
| R-02 | Rust 2024, optional compatible wgpu adapter | [M0](evidence/m0-qualification.md), [M3A](evidence/m3a-renderer.md) |
| R-03 | Dependency boundaries/inventory and locked consumers | [M8 package audit](evidence/m8-packaging.md); vulnerability/maintenance release review open |
| R-04 | Shared retained/declarative/immediate runtime | [M1A](evidence/m1a-runtime.md), T-01–T-06 |
| R-05 | Canonical builders and compiled example pack | [M4](evidence/m4-authoring-proofs.md); independent onboarding T-27 open |
| R-06 | Windows product target; Linux/macOS CI portability | [platform matrix](preview-release.md); current-revision T-15 execution required |
| R-07 | Hardware 2D/3D and independently scheduled viewports | [M3B](evidence/m3b-completion.md); reference performance budgets open |
| R-08 | Editor/storefront/IDE, embedding and add-on proofs | [M4](evidence/m4-authoring-proofs.md), [M7](evidence/m7-addons.md); native integration gaps recorded |
| R-09 | Native decorated shell and service adapters | [M1B](evidence/m1b-native-shell.md), [M2B](evidence/m2b-controls.md); DPI/physical/AT sweep open |
| R-10 | C-01–C-45 classified below | Expansion/deferred boundaries preserved |
| R-11 | Rust/CLI/MCP shared automation and guarded driver | [M5](evidence/m5-automation.md); physical T-23–T-26 gaps remain |
| R-12 | Design baseline preceded authorized implementation | [decision register](../design/09-decisions-and-questions.md), bootstrap history |
| R-13 | Explicit DisposeAfterUse/KeepHot and receipts | [M6](evidence/m6-webview.md); native/platform gates remain as recorded |

## Capabilities

The original priorities and full contracts remain in the
[capability inventory](../design/07-feature-discovery.md). Partial expansion
examples do not activate the rest of their future roadmap.

| ID | Implementation or explicit deferred boundary | Evidence / qualification |
| --- | --- | --- |
| C-01 | Generational identity, keyed ownership | [M1A](evidence/m1a-runtime.md) |
| C-02 | Retained/declarative/immediate composition | [M1A](evidence/m1a-runtime.md), [M4](evidence/m4-authoring-proofs.md) |
| C-03 | Actions, cancellation and async resource ownership | [M1A](evidence/m1a-runtime.md), [M4](evidence/m4-authoring-proofs.md) |
| C-04 | Constraints/layout and shaping measurement | [M1B](evidence/m1b-review-fixes.md), [M2A](evidence/m2a-text-editing.md); multi-DPI sweep open |
| C-05 | Shared focus/capture/keyboard/modal engines | [controls](../guides/controls.md); physical scope/IME gaps remain |
| C-06 | Committed semantics and optional AccessKit translation | [M2B](evidence/m2b-controls.md); native AT certification open |
| C-07 | Demand scheduling, scene/UI counters | [M1A](evidence/m1a-runtime.md), [M3B](evidence/m3b-completion.md) |
| C-08 | Optional wgpu and same-device host embedding | [embedding](../guides/embedding.md) |
| C-09 | Paint/provider contracts and resource retirement | [M3A](evidence/m3a-renderer.md) |
| C-10 | Harness and shared automation dispatcher | [M5](evidence/m5-automation.md) |
| C-11 | Decorated Windows shell/services | [M1B](evidence/m1b-native-shell.md); native sweep open |
| C-12 | SDR quads/paths/text/images/clips/layers | [rendering](../guides/rendering.md); HDR/export deferred |
| C-13 | Basic camera/mesh/depth/picking | [scene](../guides/scene.md); advanced engine features deferred |
| C-14 | Splitter/property controls and editor proof | [controls](../guides/controls.md), [M4](evidence/m4-authoring-proofs.md) |
| C-15 | Virtual list/tree/table behavior | [controls](../guides/controls.md); physical virtualization/AT gates open |
| C-16 | Shared editor, IME adapter, clipboard | [text](../guides/text-editing.md); native composition matrix open |
| C-17 | Window ownership and shared app document proof | [M4](evidence/m4-authoring-proofs.md); physical cross-window workflow open |
| C-18 | Gallery/inspector/capture/failure bundle | [M5](evidence/m5-automation.md) |
| C-19 | Optional CLI and rmcp stdio package | [M5](evidence/m5-automation.md) |
| C-20 | Theme/revision invalidation; app-owned resource reload | [controls](../guides/controls.md); arbitrary stateful code reload excluded |
| C-21 | Document-owned commands/undo and drag transaction | [M3B](evidence/m3b-completion.md), [M7](evidence/m7-addons.md) |
| C-22 | Basic tabs only; docking/tear-off/persistence deferred F-02 | [checklist](implementation-checklist.md) |
| C-23 | Shared plain-text engine; advanced editor services deferred F-03 | [text](../guides/text-editing.md) |
| C-24 | Tiled brush proof only; complete pen/image engine deferred F-04 | [M3B](evidence/m3b-completion.md) |
| C-25 | Basic scene proof; advanced materials/assets/physics deferred F-05 | [scene](../guides/scene.md) |
| C-26 | Initial SDR paths/layers; advanced effects/export deferred F-06 | [rendering](../guides/rendering.md) |
| C-27 | Custom native title-bar content deferred F-07 | Native decorations preserved |
| C-28 | Optional rectangular WebView2; transformed/native universal embedding deferred | [webview](../guides/webview.md) |
| C-29 | Professional color/HDR deferred F-06 | Initial SDR profile explicit |
| C-30 | Arbitrary external native GPU sharing deferred F-08 | Same-device wgpu contract only |
| C-31 | Trusted native-process add-ons; VM/DLL backends deferred F-09 | [add-ons](../guides/addons.md) |
| C-32 | Arbitrary stateful Rust hot reload deferred F-10 | Application resource reload only |
| C-33 | Mobile and ARM64 product support deferred F-12 | [release scope](README.md) |
| C-34 | Explicit request/revision/retry/branch resource contracts | [network UI](../guides/network-ui.md) |
| C-35 | Deterministic native-storefront proof, bounded remote sections | [M4](evidence/m4-authoring-proofs.md); no production commerce backend |
| C-36 | Web/WASM delivery deferred F-11 / T-20 | No web capability ceiling imposed on native |
| C-37 | Host-owned diagnostics contract and optional implementation | [crash policy](../design/12-diagnostics-and-crash-policy.md); T-21 evidence separate |
| C-38 | Typed app commands, schema/precondition/revision/undo dispatch | [M5](evidence/m5-automation.md) |
| C-39 | Guarded selected-desktop driver | [automation](../guides/automation.md); actual physical sweep open |
| C-40 | Versioned authoring pack and example checks | [authoring](../guides/authoring.md); independent onboarding evidence open |
| C-41 | Separate Markdown control and independent consumer | [community SDK](../guides/community-controls.md) |
| C-42 | Shared behavior engines and declared control profiles | [controls](../guides/controls.md); native profile certification open |
| C-43 | Add-on review panel and revision-checked document transaction | [M7](evidence/m7-addons.md) |
| C-44 | Webview lifetime/ownership policies and cleanup receipts | [M6](evidence/m6-webview.md) |
| C-45 | Workspace, coherent versions, isolated consumers/package audit | [packaging](evidence/m8-packaging.md); publication disabled |

## Acceptance scenarios

The canonical scenario definitions are in the
[validation plan](../design/08-delivery-and-validation.md). This table records the
evidence location and the additional layer needed; it is not a pass/fail substitute.

| ID | Existing evidence / implementation | Additional boundary or remaining gate |
| --- | --- | --- |
| T-01 | M1A runtime and M4 keyed authoring tests | Native edit/focus interaction certification separate |
| T-02 | M1A immediate clean-region tests | No native inference |
| T-03 | M1A pure phase/action tests | Controls must retain once-only effects |
| T-04 | M1A focus/capture/unmount tests | Native IME cancellation and physical sweep |
| T-05 | M1A ownership and M4 request-race tests | No late native callback claim beyond owning evidence |
| T-06 | M3B scene scheduling and embedding proof | Named reference performance baseline |
| T-07 | M3B scene GPU readback | Linux/macOS GPU execution |
| T-08 | M3B canvas transform/drag/undo contracts | Physical transform/DPI/overlay interaction |
| T-09 | M3B host device/encoder/submission proof | Supported host/platform matrix |
| T-10 | M3A cache eviction/epoch/readback tests | Long-running native stress and performance limits |
| T-11 | M1B geometry, M2 text and surface scale contracts | Real multi-monitor DPI/text/IME/overlay sweep |
| T-12 | M3 surface/device recreation contracts | Real native recovery and platform matrix |
| T-13 | M1B decorated shell | Caption/snap/system-menu/activation physical sweep |
| T-14 | M5 selectors, revision waits/captures and selected-desktop editor/gizmo/modal workflows | Broader native control/profile coverage |
| T-15 | Earlier OS CPU manifests; CI workflow | Execute latest revision on Linux/macOS |
| T-16 | M4 public control/proof packages | Native behavior layers as documented |
| T-17 | M4 resource race/branch/retry tests | Native focus continuity coverage |
| T-18 | M4 stream/backpressure/gap tests | Measured native responsiveness under load |
| T-19 | M4 deterministic storefront/sections/assets | Native physical scroll/focus and hardware limits |
| T-20 | Explicitly deferred F-11 | No preview web/WASM gate |
| T-21 | Optional diagnostics and actual supervised panic/abort/native-fault/hang recovery fixtures | Production WER collection and supported-OS/host-policy qualification |
| T-22 | M5 CLI/MCP schema/dispatch subprocess parity | Supported transport/platform execution |
| T-23 | Guarded desktop fixture and geometry receipts | Actual physical transform/clip/DPI/overlay matrix |
| T-24 | Virtual/nested scroll engines and desktop fixture | Actual physical scrolling/chaining evidence |
| T-25 | Physical en-US keyboard/modal/editor/Page flows; separate IME scenario | Installed Japanese IME and wider control certification |
| T-26 | Actual owned foreground-loss, closure/hidden-target rejection and OS-observed held-Shift cleanup on cancellation/owner drop | Forced termination and elevated/secure-desktop coverage |
| T-27 | Compiled versioned eight-example onboarding pack | Independent new-reader/model trials with errors recorded |
| T-28 | Separate Markdown package/consumer/profile adapter | Native render/input/AT limits declared |
| T-29 | M2 text profiles and control engine suites | Native clipboard/IME/AT profile matrix |
| T-30 | M2 composition/external-update/revision tests | Physical composition and branch-update sweep |
| T-31 | Shared engine styled variants and conformance adapters | Native/render portions of mandatory profile |
| T-32 | M7 review panel/document undo proof | Native physical host-control coverage |
| T-33 | M7 negotiation/stale/deactivation/resource tests | Native failure/cleanup stress matrix |
| T-34 | M7 actual AOT subprocess framing/limits/trust | No sandbox or safe DLL unload guarantee |
| T-35 | M6 owned-profile disposal/receipt proof | Native results only as recorded, OS runtime-specific |
| T-36 | M6 shared-session lifecycle tests | Sibling native behavior and retained resources evidence |
| T-37 | M6 KeepHot lease/budget/state proof | Native focus/IME and long-running retained cost |
| T-38 | M6 stale callbacks/startup discovery/shutdown tests | OS/runtime failure races and process cleanup evidence |
| T-39 | Local metadata/content/profile audit and isolated consumers | Clean reviewed revision, registry/archive verification and latest OS runs |

M1A/M1B, M2, M3A/M3B, M4, M5, M6 and M7 above refer to their linked evidence in
the requirement/capability tables. Remaining required native/platform/release
coverage keeps the preview release gate open; future F-* exclusions remain
explicit rather than being counted as implemented capabilities.
