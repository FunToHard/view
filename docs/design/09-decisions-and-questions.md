# Decision baseline and refinement log

Priority amendment — 2026-10-23: the owner authorized building UI controls before
finishing remaining framework milestones. Unfinished implementation/qualification
items are deferred from the active queue, not marked passed or removed from release
requirements. Work proceeds through the UI-* entries in the canonical implementation
checklist: extend the existing gallery and shared behaviors first, then dropdowns,
menus/dialogs and collection controls. Accessibility, IME, lifecycle and performance
acceptance remain attached to each applicable control. Routine framework fixes are
limited to concrete control requirements; advanced docking and unrelated capabilities
remain deferred.

Revision: 0.4 — 2026-10-07. Baseline selected under the owner's explicit request to finalize decisions and continue planning. Windows 11 x64, GitHub Actions, and MIT OR Apache-2.0 are delegated defaults, not explicit questionnaire answers. No implementation is authorized by this planning update.

## Accepted requirements

Memory-budget clarification (2026-10-24): the owner permits compromising on the
memory target if necessary but explicitly excludes software rendering. Use
hardware-GPU staged measurements to separate native/window, device/swapchain,
renderer and text/control costs before selecting a revised budget. Report both
working set and private commitment; a hidden minimal-form measurement does not
cover full application/IME/accessibility overhead. Reproduction:
[GPU memory guide](../guides/gpu-memory.md).

DX12 allocator follow-up (2026-10-24): wgpu 30.0.1's default performance
memory hint reserves 192 MiB of reported DX12 suballocation blocks at device
creation in the measured host. The memory-usage hint lowers that to 12 MiB and
manual 16–64 MiB to 24 MiB; after 1,000 edit/render iterations the measured
private commitment favored manual sizing, while idle commitment favored
MemoryUsage. These are probe-only variants on one NVIDIA host. No framework
default or public policy API changes until realistic churn and frame-latency
trade-offs are qualified. Evidence and reproduction:
[GPU memory guide](../guides/gpu-memory.md).

Profile-guided optimization (2026-10-24): `ShapedText` shares immutable layout
storage instead of cloning cosmic-text buffers and geometry on cache hits.
Sampled-image bindings live with renderer-owned textures and are shared by
prepared frames, avoiding per-draw texture-view/binding creation. Existing
font/device identity, eviction and prepared-frame lifetime contracts remain.
No dependency or unsafe-code changes. Reason and acceptance evidence:
[renderer evidence](../plan/evidence/m3a-renderer.md).

M3A cache follow-up (2026-10-23): bound retained GPU text draws by Budgets::max_text_entries as well as atlas accounting. This prevents edit/resize presentation keys from retaining draw resources indefinitely. Prepared frames own evicted draws independently. Evidence: the DX12 offscreen two-entry/100-key churn and retained-frame pixel comparison, plus scripts/verify.ps1, passed locally.

M3A resize correction (2026-10-23): also bound retained vertex/index buffer pairs
by `Budgets::max_buffer_entries` (default 256, zero disables retention). Resize
changes normalized geometry keys; thousands of tiny GPU resources have driver
overhead far beyond their payload-byte accounting. Prepared frames retain evicted
buffers. Ignore duplicate surface sizes while preserving initial/resume
configuration, and poll completion in the gallery. Acceptance and local memory
measurements: [M3A renderer evidence](../plan/evidence/m3a-renderer.md).

M3A resize latency follow-up (2026-10-23): cache mesh positions in physical pixels
and convert to clip coordinates in the shader with an immutable per-frame
viewport uniform. This removes size-only mesh allocation churn without mutating
in-flight geometry or projection. The gallery paints on native resize delivery
instead of deferring that paint to a separate redraw event. Semantic bounds still
update with the client size. Acceptance: [M3A renderer evidence](../plan/evidence/m3a-renderer.md).

M3A cache correction, 2026-10-22: immutable shaped results now carry clone-stable
identity. The renderer uses that identity rather than the address of a caller's
`Arc` wrapper. Reason: controls wrap cached layouts on every repaint, causing
duplicate retained text renderers during idle operation. Cache ownership and
font/device epochs are unchanged; new shaping results have new identities.
CPU identity and GPU fresh-wrapper regression evidence is recorded in
[M3A evidence](../plan/evidence/m3a-renderer.md).

M6 implementation refinement, 2026-10-22: `view-webview` separates portable
ownership/bridge contracts from a Windows-only, UI-thread WebView2 adapter.
Dedicated ephemeral profiles are the default; shared/persistent data is never
deleted. Controller Close is followed by matching session/PID browser-group exit
and worker-thread profile removal. Failed/pending receipts remain observable;
startup discovery never assumes a prior process exited. KeepHot preserves the
same controller with host accounting budgets and explicit eviction. Exact
document grants are deliberately narrower than origin-wide authority, and
session/epoch wire values are decimal strings to avoid JavaScript precision loss.
Reason: make both lifetimes executable without adding a browser dependency to
core or promising synchronous process cleanup. The qualified Windows binding is
webview2-com 0.39.1 with windows 0.62.2; its sys/macros plus base64 0.22.1 add four
registry packages. Runtime installation remains host-owned. Contracts and
maintenance/licensing rationale: [webview guide](../guides/webview.md).
Acceptance T-35–T-38 and remaining native/platform gates:
[M6 evidence](../plan/evidence/m6-webview.md).

R-01 through R-13 in the [product charter](01-product-charter.md) record explicit owner direction. They are requirements, not claims of implemented behavior.

The latest scope refinement establishes Windows as the first product target, Linux/macOS CI testing, complex 2D/3D applications, native OS integration, the name/tagline, and creation/refinement of design documents. It broadens the earlier desktop-controls framing substantially.

Revision 0.2: the owner confirmed that the documents fit the design criteria and requested deeper treatment of network-driven UI, web suitability, crash handling, application-integrated MCP, and physical hit-test/scroll/keyboard E2E. They clarified that the intended additional proof is a native network-driven storefront, not a browser shell. WASM/web remains a concern to evaluate without restricting native primitives. Positive design feedback does not close every open API/dependency choice or authorize implementation.

Revision 0.3: the owner asks for low-friction UI authoring by models without view-specific training, shareable community control crates, standard controls that do not omit expected editing behaviors, and runtime add-ons with custom UI in a Word-like application. These guide the new proposals below; no loader/runtime dependency or final syntax is selected by this update.

## Selected architecture

M7 refinement, 2026-10-22: P-22/Q-13 uses optional `view-addons`, four-byte
length-prefixed JSON on dedicated stdio and trusted AOT child executables.
Versioned manifests select offered host slots and compiled component schemas;
flat retained label/button/text nodes share core identities and control engines.
Reason: executable extension UI and document-command boundaries without a new
runtime, ABI, VM, mandatory networking or browser dependency. Host application
traits retain document revision validation and undo authority. The host renders
failure diagnostics in its own chrome; guests lose all contributed UI/commands.
Explicit teardown requires the owning runtime, and process receipts distinguish
child exit from worker completion. OS sandboxing/descendant containment are not
claimed. There are no new registry dependencies; optional rendering reuses the
qualified renderer. Contracts/subset: [add-ons](../guides/addons.md). Acceptance:
[M7 evidence](../plan/evidence/m7-addons.md).

M5 refinement, 2026-10-21: P-08/P-09/P-17/P-18 use optional `view-automation`
over the existing harness and `view-tooling` for explicit local stdio / rmcp 3.6.0
attachment. One versioned request model, typed schemas and owner-thread action
dispatch serve Rust/CLI/MCP. Background jobs receive owned data plus cancellation;
bounded session deduplication never promises cross-restart exactly-once delivery.
Reason: external tooling must not add a second runtime or mandatory networking.
The desktop driver is a separate opt-in platform feature with foreground/process
checks and watchdog cleanup. Input insertion and observed routing remain separate
receipts. Core exposes capture/clip diagnostics and effective enabled state so
disabled controls remain occluding while excluded from input/focus. Contracts:
[automation](../guides/automation.md). Acceptance and unexecuted native gates:
[M5 evidence](../plan/evidence/m5-automation.md).

M4 refinement, 2026-10-19: P-02/P-13/P-14/P-19/P-20 now use a bounded nested
Element adapter over existing Description/Runtime identity, an optional
transport-independent `view-resource` package and a separate `view-markdown`
public consumer. Reason: prove application authoring without a second lifecycle,
mandatory executor/network stack or add-on loader. Resources use owner leases,
namespace/request generations and explicit UI-thread completion; refresh, retry,
cache, stream and reconnect policy are caller-visible. The optional codec profile
is raw RGBA plus bounded uncompressed 24-bit BMP; no new registry dependency.
The Markdown profile deliberately supports bounded block syntax and app-owned
navigation rather than claiming full CommonMark/rich-text behavior.
Contracts: [authoring](../guides/authoring.md),
[community controls](../guides/community-controls.md).
Evidence and platform limits: [M4 report](../plan/evidence/m4-authoring-proofs.md).

M4 integration correction: captured pointer coordinates are now transformed to
the capturing node's local space, including ancestor scroll/scale, just like
ordinary hit results. Previously the layout-free PointerRouter emitted window
coordinates through Runtime during capture. A translated editor gizmo exposed
the discrepancy. `Runtime::window_to_local` is the explicit public mapping;
capture-coordinate and integrated drag/undo regressions establish the correction.

M3B completion refinement, 2026-10-19: P-04/P-05/P-07 use CPU triangle picking
with owner-qualified latest requests and scene/camera/viewport revisions, normal
runtime capture plus document-neutral drag commits, and host-clock per-owner
viewport scheduling. P-01/P-03 are preserved: app scene selection/undo remain
outside UI identity, and previews do not execute document effects. Core now
exposes explicit host-service capability/publication contracts and counts layout
traversals separately from scene work. The adapter accepts validated host targets
and records optional readback into a host encoder without taking submission
authority. Reason: prove complete embedding and mixed UI/scene cadence without
adding an executor, native dependency, ECS or scene-translation requirement.
Contracts and compatibility: [embedding guide](../guides/embedding.md).
Acceptance and uncovered native/platform layers: [M3B report](../plan/evidence/m3b-completion.md).

M3B-02 implementation refinement, 2026-10-18: P-04/Q-05 now has the opt-in
`view-wgpu/scene` module and `view/scene` facade feature, without new dependencies
or a separate crate. The minimal profile is right-handed, perspective 0..1 depth,
affine models, indexed triangle meshes and opaque unlit sRGB materials. Reason:
prove scene rendering through the qualified provider/device boundary before
picking or application tools. ProviderSession now supports an opt-in private
Depth32Float offscreen attachment; direct contributions remain color-only.
Immutable snapshots/buffers and idle-only provider mutation preserve submission
ownership and bounded resources. Contracts: [scene guide](../guides/scene.md).
Acceptance/coverage: [M3B-02 evidence](../plan/evidence/m3b-02-scene.md).

M3B-01 implementation refinement, 2026-10-18: P-04/Q-15 now has a
backend-independent `view_core::canvas` module, re-exported as `view::canvas`.
It uses f64 canvas document coordinates and rebases before checked f32 conversion,
preserving existing f32 geometry compatibility. One bounded, revisioned record
snapshot supplies drawing descriptions, rectangular hits and semantic regions.
The application owns document IDs/payloads; runtime node identity, focus, capture,
semantic publication and effects retain their existing ownership. Reason: begin
the canvas proof without introducing GPU dependencies or a parallel UI lifecycle.
No new dependency or crate is needed. Contracts: [canvas guide](../guides/canvas.md).
Acceptance and coverage limits: [M3B-01 evidence](../plan/evidence/m3b-01-canvas.md).

P-01 through P-24 are adopted as the planning baseline. Historical proposal labels in earlier revision notes explain how the design evolved; they are not unresolved implementation alternatives. The resolution table below is authoritative. Backend qualification and measurements are implementation evidence tasks; failing a qualification requires a documented decision amendment rather than quietly changing architecture.

| ID | Proposal | Rationale | Status |
| --- | --- | --- | --- |
| P-01 | One runtime identity/lifecycle/interaction model with derived structures | Prevent retained/immediate input and state disagreement | Selected |
| P-02 | Ordinary Rust builders, property structs for configuration, Element branches | Rust-native tools and understandable API boundary | Selected |
| P-03 | Explicit actions/revisions first; pure build/measure | Predictable scheduling and effect ownership | Selected |
| P-04 | Canvas/viewport providers and basic first-party 3D | Prove complex graphics are supported | Selected |
| P-05 | view-owned and host-owned GPU/loop modes | Support integration without competing runtimes | Selected |
| P-06 | Native decorations and targeted platform services | Preserve OS behavior and contain platform complexity | Selected |
| P-07 | Demand-driven UI with independent viewport frame requests | Idle efficiency and real-time graphics coexist | Selected |
| P-08 | One automation command model; optional CLI/MCP adapters | Tests and tools share semantics | Selected |
| P-09 | Inspection, semantics, controlled time, traces from the foundation | Tooling accelerates framework development | Selected |
| P-10 | Ordinary crate controls independent of runtime add-ons | Keep source-level reuse simple | Selected |
| P-11 | Core/GPU/native CI tiers with honest coverage | Portable evidence without false support claims | Selected |
| P-12 | Named workloads and measured regression budgets | Testable performance claims | Selected |
| P-13 | Resource subscriptions and ordinary Rust branches | Async updates with explicit lifetime/revisions | Selected |
| P-14 | Native storefront fixture and bounded remote-section adapter | Exercise remote data/configuration without a browser | Selected |
| P-15 | Defer web delivery without capping native capabilities | Explicit release boundary | Deferred for this release |
| P-16 | Host-owned diagnostics, optional panic reporting and OS dump setup | Respect existing process-level ownership | Selected |
| P-17 | App-registered typed domain automation commands | Complex app workflows with proper document authority | Selected |
| P-18 | Optional desktop driver for physical input | Real hit/scroll/keyboard evidence | Selected |
| P-19 | Canonical API, versioned examples and discovery | Lower onboarding friction; no perfect-AI claim | Selected |
| P-20 | Community controls use public composition/behavior contracts | Normal Cargo distribution | Selected |
| P-21 | Shared behavior engines and versioned conformance profiles | Standard controls cannot omit mandatory behaviors silently | Selected |
| P-22 | Trusted child-process Rust add-ons over versioned UI/command protocol | Runtime installation without JIT/ABI coupling | Selected |
| P-23 | Git monorepo with modular Cargo workspace; no first-party submodules | Atomic API/test changes and coherent maintenance | Selected |
| P-24 | Optional WebView2 panels with DisposeAfterUse and KeepHot | App-controlled lifetime with explicit cleanup ownership | Selected |

## Decision resolutions

| ID | Topic | Final baseline | State |
| --- | --- | --- | --- |
| Q-01 | Windows baseline | Windows 11 x64 product target; Windows 10 and ARM64 product certification deferred | Closed; delegated default |
| Q-02 | Syntax | Canonical builders and typed actions; property structs for complex configuration; tuples for fixed children, keyed iterators for lists, Element conversion for heterogeneous branches; no initial DSL macro | Closed |
| Q-03 | State/callbacks | Application-owned models, explicit subscriptions/revisions and typed action queues; component registry holds owned state/callbacks; current model context borrowed only during dispatch; pure build/measure; no required reactive library | Closed |
| Q-04 | Proof priority | Engine-editor vertical slice first, incremental native storefront and external Markdown-control consumer next | Closed |
| Q-05 | 3D scope | Optional camera/mesh/basic material/depth/picking/overlay module and custom providers; no mandatory ECS, PBR or physics engine | Closed |
| Q-06 | Source languages | Rust framework/host code with WGSL shaders; native bindings permitted; no Rust-to-GPU compiler requirement | Closed |
| Q-07 | Native platform | winit plus targeted Windows bindings; native decorations first; title-bar content deferred until native behavior can be certified | Closed |
| Q-08 | Color | Explicit SDR composition and color metadata; professional HDR/color/export pipeline deferred | Closed |
| Q-09 | CI | GitHub Actions with three-OS job contracts plus separately provisioned graphics/interactive gates; runner hardware/credentials/budget never assumed | Closed; delegated default |
| Q-10 | Dependencies | Selected stack below; compatible versions/MSRV and lockfile qualified in M0, not claimed tested by documents | Closed architecture; qualification task |
| Q-11 | Repository | One Git monorepo, one Cargo workspace, staged crates by dependency/platform boundary; no first-party submodules | Closed |
| Q-12 | Cross-window state | Preserve application documents; remount view-local state by default; typed explicit snapshots can restore selection/scroll; focus/capture/IME resolve through platform lifecycle | Closed |
| Q-13 | Runtime add-ons | Optional trusted native child process, bounded length-delimited JSON over dedicated stdio, version negotiation and host-rendered UI; VM/DLL/script backends deferred | Closed |
| Q-14 | Interop | Windows WebView2 rectangular panels with explicit lifetimes; compatible same-device/version wgpu interop; arbitrary external API sharing/transformed webviews deferred | Closed |
| Q-15 | Internal representation | Arena runtime and deliberate erased descriptors at dynamic boundaries; normal typed public builders; allocations/compile size measured and optimized without changing semantics | Closed |
| Q-16 | Network UI | Transport-independent Resource<T>, explicit caching/retry policies, native data/configuration first; bounded allowlisted section-schema adapter; no remote executable code | Closed |
| Q-17 | Web delivery | Deferred for this release; future capability subset must not limit native providers | Deferred scope decision |
| Q-18 | Diagnostics | Bounded diagnostic envelope and opt-in host panic hook; Windows WER collection guidance/integration where compatible; custom dump helper and watchdog deferred; healthy-operation document recovery | Closed |
| Q-19 | Physical E2E | Explicit desktop session; target/foreground checks, stop/cleanup; initial en-US keyboard and Japanese IME fixture, with additional locales later | Closed; provisioning task |
| Q-20 | Controls | Shared edit engine; SingleLineText v1 mandatory profile; separate plain-multiline profile extends it; primitives cannot masquerade as complete controls | Closed |
| Q-21 | Control SDK | Public crate interfaces, coordinated pre-1.0 compatibility train, runnable examples and reusable versioned conformance suites | Closed |
| Q-22 | Add-on UI | Namespaced side panel/menu/toolbar slots, standard host controls, transactional UI patches; app owns document commands/undo; scoped automation publication | Closed |
| Q-23 | Learnability | Version-matched quick reference/examples, read-only discovery and compiler/behavior tests; measure onboarding corrections without claiming universal model reliability | Closed |
| Q-24 | License | MIT OR Apache-2.0 for first-party framework code; license texts, package metadata and fixture/dependency attribution are M0/release tasks | Closed; delegated default |

All baseline decisions are selected or expressly deferred. The owner requested finalization and then continuation without supplying questionnaire choices; Q-01, Q-09 and Q-24 therefore use the stated recommended defaults under that delegation. They remain amendable before dependent implementation. Provisioning actual runners, checking registry names, selecting compatible versions, and measuring hardware are concrete project tasks rather than unanswered architectural choices. No passing tests or available infrastructure are inferred from selecting a design.

## Selected dependency and layout stack

| Area | Selection | Boundary |
| --- | --- | --- |
| Rendering/windowing | wgpu; winit; targeted windows bindings | Native/backend adapters; feature-scoped |
| Text | cosmic-text; glyphon for wgpu text integration; unicode-segmentation for editing boundaries | Compatible text/backend version family; framework-owned public interfaces |
| Paths | lyon tessellation | 2D renderer; do not implement a general tessellator from scratch |
| Layout | Built-in constrained row/column/stack/scroll/virtual layout; custom layout trait | Deliberately limited native layout, no claim of CSS conformance; Taffy/CSS grid deferred |
| Accessibility | AccessKit and winit integration | Native adapter and semantic bridge |
| GPU data layout | bytemuck where needed | Checked POD/derive usage at GPU upload boundary |
| Protocols | serde/serde_json in protocol packages; rmcp for optional MCP adapter | No serialization/network executor required in core |
| Webview | WebView2 through webview2-com Rust bindings | Optional Windows package; managed runtime detection; both lifetime policies |
| Add-ons | std process/IO plus bounded protocol codecs | Trusted process model; no VM dependency initially |
| Diagnostics | std/platform facilities and optional reporting adapter | No mandatory telemetry service or automatic global hook |

Exact versions are pinned only after a compatible set builds and passes the M0 qualification. Prefer a coherent maintained backend family over mixing incompatible newest versions. A necessary replacement requires evidence and a recorded baseline amendment. Public package names/MSRV are verified in bootstrap/release tasks, not fabricated during planning.

The dependency choices rely on the existing [source ledger](10-sources.md) plus primary projects for [WebView2 Rust bindings](https://github.com/wravery/webview2-rs), [lyon](https://github.com/nical/lyon), and [Unicode segmentation](https://github.com/unicode-rs/unicode-segmentation). Selection is not certification that any library is bug-free.

## Internal design refinement in revision 0.1

| Issue found during drafting | Refinement |
| --- | --- |
| A widget-only core would make 3D an exception | Added render providers, first-party proof, and host-owned embedding |
| A raw draw callback omits ownership and scheduling | Added negotiation/prepare/encode/compose/retirement/recovery phases |
| Immediate callbacks might repeat effects during layout | Split update consumption from pure build/measure |
| Shared tree could imply every scene object is a widget | Separated UI ownership from application scene/document models |
| “Native performance” could be an unverified claim | Defined measurable properties and workload-based validation |
| Linux/macOS CI could imply full support | Distinguished executed tests, GPU evidence, native sessions, and support status |
| MCP could become a mandatory runtime dependency | Made it an optional adapter over a shared automation API |
| Direct actions could be misreported as E2E input | Added explicit semantic, injected-input, and native-OS test tiers |
| Title-bar customization could regress OS behavior | Native defaults and behavior-based acceptance criteria |
| “No workarounds” could imply impossible universal interop | Added supported integration contracts and explicit capability negotiation |
| Same-device sharing could hide Rust dependency incompatibility | Added an explicit wgpu API/version compatibility boundary |
| Image-editor coordinates could inherit viewport precision limits | Kept document/world coordinates separate from GPU pixel representation |
| Feature inventory could drift away from owner requirements | Added requirement-to-document-to-acceptance traceability |

## Refinement in revision 0.2

| Issue / clarification | Refinement |
| --- | --- |
| Browser example meant network-driven storefront UI | Replaced temporary browser-shell interpretation with native storefront proof |
| Network responses need practical developer branching | Added Resource/effect flow, Rust match example, type-erasure option and branch lifetime |
| Remote data, configuration and executable code could be conflated | Separated data/configuration/schema-driven UI with a bounded local registry |
| 3D might be taken to prohibit web entirely | Documented compatible WebGPU 3D and separate native-only capability requirements |
| Portability could cap native features | Kept web deferred and native features unrestricted by a web subset |
| Crash handler could seize process-global behavior | Added optional host-owned diagnostics, native collection choices and recovery boundary |
| MCP integration might stop at generic widget clicks | Added typed domain commands, long operations, preconditions, undo and retry semantics |
| Locator and framework could agree on the same wrong bounds | Added OS input observations, analytic fixtures and independent visual checks |
| “Scroll test” could merely assign a scroll offset | Split ensure-visible setup from physical scroll/keyboard validation |
| Native input testing could disrupt unrelated work | Added explicit desktop session mode, foreground checks, stop and held-input cleanup |

## Refinement in revision 0.3

| Issue / clarification | Refinement |
| --- | --- |
| AI models may not know view's API | Added consistent Rust conventions, versioned discovery/examples and onboarding evaluation |
| A community control might require plugin machinery | Separated normal Cargo controls from runtime-installed add-ons |
| Text fields repeatedly omit expected features | Added shared editing behavior and mandatory profile/conformance gates |
| A checklist alone might certify an incomplete implementation | Require actual behavior tests and explicit uncovered cases across variants |
| Lack of Rust JIT might be mistaken for no runtime extensibility | Documented AOT process/DLL and interpreted WASM routes |
| Native plugin loading could assume stable Rust objects | Defined separate ABI/protocol requirements and ownership/trust limits |
| A Word add-on could imply framework-owned document logic | App owns document commands/undo; view owns UI/lifecycle/automation integration |
| WASM add-ons could imply web deployment | Kept native-host WASM execution independent of deferred web rendering |

## Finalization in revision 0.4

The owner requested final decisions and project planning. Technical proposals are now selected or explicitly deferred. The monorepo/workspace and staged release plan replace open-ended repository options. Webview panels have two developer-selected policies with independently tracked execution/profile ownership. Windows baseline, license and CI provider were asked as optional preferences. After the owner requested continuation, the stated recommended defaults were selected under their delegation; no questionnaire response is fabricated.

Changes to this baseline now require a short decision amendment with the reason, affected APIs/dependencies, and updated evidence. Exact dependency versions, benchmarks and runner availability remain qualification tasks because they cannot truthfully be established before implementation/provisioning.

## How to refine this set

### M1A-01 implementation refinement — 2026-10-08

P-01/P-03/Q-15 now have foundation value types in dependency-free `view-core`.
Arena-qualified generational handles distinguish slots across arenas; a distinct
window wrapper prevents accidental interchange with generic handles. Raw
construction is explicitly unchecked, and future arenas remain responsible for
liveness checks. Nonzero generations and checked u64 advancement avoid aliasing
old identities/revisions on overflow; exhausted slots must be retired. Arena
namespaces are session-local, issued without reuse by their eventual owner.

The facade re-exports these contracts without platform/backend/tool dependencies.
No third-party dependency was added: the first-party MIT OR Apache-2.0 core is
maintained in this workspace, uses std only and has no target-specific branches.
`view-testing` is intentionally staged with its first harness implementation
(M1A-11), rather than introduced empty. This refines the existing staged-crate
rule and does not close RUN-01/RUN-02/RUN-05. Contract and acceptance evidence:
[runtime design](02-runtime-architecture.md#identity-and-lifetime) and
[foundation verification](../plan/evidence/m1a-01-foundations.md).

### M1A runtime implementation refinement — 2026-10-08

P-01/P-03/P-07/P-09 and Q-03/Q-15 now have a headless implementation. Runtime
state/callbacks stay on their owner thread through a !Send/!Sync boundary; model
access is borrowed only during dispatch. A checked process-local atomic issues
unique arena namespaces. This supersedes the unimplemented issuer in M1A-01
without introducing global application state. Arena exhaustion retires identities.

Direct-child authority is explicit (retained, declarative or immediate). Keys and
Rust state types identify compatible components; structure changes also remount.
Compatible descriptions replace handlers but retain local state and descendants.
Root/window teardown revokes owned work. Tokens carry both owner and request
generations; successful enqueue does not replace dispatch-time validation.

Required actions return ownership on queue saturation. Replaceable previews use
an explicit key, return the replaced action and take the newest sequence position.
Region responses are FIFO and consumed before a builder call; bounded response
buffers report backpressure rather than dropping input. Suspended regions preserve
their state/responses. There is no promise of rollback for application effects or
recovery from panicking callbacks. Duplicate description keys are rejected before
tree mutation; a failed build leaves the previous published snapshot visible.

Layout invalidation conservatively covers the whole window. Structural commits
remain distinct from adapter-acknowledged presentation and from independent
viewport requests. The harness uses explicit virtual time and FIFO fixture
services; it adds only a first-party core dependency. No registry dependency,
executor, native input or GPU requirement is added to either production package.

Reason: establish testable lifecycle/effect boundaries before native geometry,
semantics and rendering. Contracts and acceptance evidence are in the
[headless guide](../guides/headless-runtime.md) and
[M1A runtime report](../plan/evidence/m1a-runtime.md). Windows/Linux/macOS CPU
execution is recorded there; performance and native behavior are not inferred
from headless checks.

### Revision procedure

M2B implementation refinement, 2026-10-18: P-21/Q-20 now has `view-controls`
with persistent shared behavior sessions, typed resolved styles, and optional
render/accessibility features. Runtime node IDs remain the sole control identity;
logical pointer capture is exposed with liveness, visibility and modal checks.
AccessKit IDs are monotonic adapter mappings, not a second ownership model.
Native callbacks read complete committed snapshots and queue revision-tagged
actions for UI-thread validation. Full semantic updates express removal through
parent child lists. Desktop effects are explicit application actions; Win32/COM
ownership and thread checks stay in view-platform. Reason: compose the existing
editor/runtime with multiple decorations and native services without adding
platform dependencies to core or applying editing effects during rendering.
Acceptance, dependency costs and uncovered native requirements are recorded in
[M2B evidence](../plan/evidence/m2b-controls.md); public contracts are in the
[controls guide](../guides/controls.md). This does not certify physical native
input, Linux AT-SPI, or native text-range geometry. M2B-11 remains open.

M3A implementation refinement, 2026-10-08: P-04/P-05/P-07 and Q-06/Q-08/Q-14
now use optional `view-wgpu`, keeping core backend-independent. The initial paint
profile is single-sample RGBA/BGRA sRGB with linear premultiplied blending,
alpha-correct image filtering, rectangular committed-window clips, lyon geometry,
and isolated opacity layers. Text shares the exact cosmic buffer used by editing;
font collection identity scopes glyph atlas epochs. Whole-epoch eviction and
immutable GPU buffers avoid live slot reassignment; submission callbacks retain
bounded frame leases. Glyph reservations conservatively use the device atlas
limit rather than invent residency measurements. Captures copy in the paint
submission, preserving the captured revision across later target reuse.

Reason: establish executable 2D/provider lifetime contracts before controls and
3D proofs. Providers have explicit direct/offscreen attachment profiles, pass
dependency validation and host submission acknowledgement; errors/recreation are
observable. Host device callbacks remain a deliberate ownership choice. No new
registry family is introduced; the qualified stack is now a production optional
adapter. API/limitations: [rendering guide](../guides/rendering.md). Acceptance and
platform gaps: [M3A evidence](../plan/evidence/m3a-renderer.md). M3A-14 remains open
without Linux/macOS graphics execution; neither M0 infrastructure nor M3B is closed.

M2A implementation refinement, 2026-10-08: P-21/Q-20 uses grapheme-safe committed
editing, scalar-indexed transient preedit, owner-local revision preconditions and
explicit logical/visual caret commands. External replacement clears field history;
application undo retains document authority. Qualified cosmic-text shaping is
optional, owns font lifetimes, and uses a bounded request cache. The public facade
keeps editing/shaping opt-in; core remains backend-free. Unicode segmentation and
the already qualified cosmic-text family are reused with no new root registry
package. OFL-owned fixtures remove host-font dependence. Windows clipboard testing
uses a private noninteractive station; native rendered IME/control certification
remains later work. Reason and acceptance: [M2A evidence](../plan/evidence/m2a-text-editing.md)
and [contracts](../guides/text-editing.md).

M2A-01 foundation refinement, 2026-10-08: P-21/Q-20 now has a `view-text` boundary
and optional facade `text` feature. Public contracts use UTF-8 scalar-boundary byte
indices with checked UTF-16 conversion, owner-local text revisions, directional
selection and separate transient composition. Layout/session traits expose no
backend types. Font/DPI inputs and retained source lifetimes are explicit so later
cosmic-text integration cannot leak cache or GPU lifetime assumptions into controls.
This first-party MIT OR Apache-2.0 crate depends only on std and view-core, adds no
registry/transitive backend cost, and has no platform-specific branches. Reason:
settle the shared text boundary before shaping and editor behavior. Acceptance:
[M2A-01 evidence](../plan/evidence/m2a-01-text-foundations.md).

M1B review corrections, 2026-10-08: Q-15 geometry composition now explicitly
applies child-local transforms before ancestors and includes arranged origins
and scroll displacement. P-03 observations publish semantic state at commit.
Modal keyboard routing validates targets and excludes hidden ancestor subtrees.
The native adapter uses the client origin rather than the decorated frame origin;
callback errors terminate the loop and propagate to its caller. Linux adapter
backend features are declared directly to avoid workspace feature-unification
leaks. Hidden native smoke requires a desktop, so the standard verification script
runs it on Windows only; other hosts run CPU tests and compilation. No new registry
dependency is selected. Acceptance: [regressions and verification](../plan/evidence/m1b-review-fixes.md).

1. Capture feedback as an existing requirement/decision change or a new numbered entry.
2. Record the decision, reason, alternatives, and affected contracts.
3. Update all affected documents in the same revision.
4. Add or revise the acceptance scenario that demonstrates the decision.
5. Check for contradictions, broken links, unlabelled assumptions, and unsupported claims.

Major decisions can graduate to individual ADR files when their alternatives need more detail. Keep this register as the index; do not create parallel contradictory “final plans.”

2026-10-25 scope amendment: the owner requested feasible Later roadmap work in
addition to remaining preview implementation. Bounded password/table/docking,
large-document editing, SDR tiled document effects/export, and scene asset/light
profiles are active increments, not claims that their entire expansion gates
are complete. Documents remain application-owned; core has no new dependency.
Diagnostics are a separate opt-in host-owned package, with Windows failure FFI
behind the explicit `failure-fixture` feature. Existing hardware-wgpu rendering
remains mandatory for GPU evidence. See [expansion evaluation](../plan/evidence/expansion-feasibility.md),
[controls](../plan/evidence/controls-expansion.md),
[large documents](../plan/evidence/f03-large-document.md),
[image documents](../guides/image-documents.md),
[scene assets](../plan/evidence/f05-scene-assets.md), and
[diagnostics](../plan/evidence/m8-diagnostics.md) for affected contracts and limits.

2026-10-26 follow-up: bounded rich-document transactions, measured-line pagination
and conservative disjoint remote rebasing are application-owned services. Remote
coordinates must validate against retained base text before transformation;
bounded missing history or overlap produces an explicit conflict. These services
do not silently become a rich visual editor or network CRDT. SDR image masks and
box blur use linear premultiplied processing with explicit source/scratch/work
budgets and straight-alpha sRGB storage. Native docking restoration persists
physical outer-window bounds in the same space as monitor work areas. A narrow
live runtime node-owner query prevalidates docking handles before focus/capture
effects; Windows monitor FFI remains in the platform adapter. Evidence:
[rich documents](../plan/evidence/f03-rich-documents.md),
[masked effects](../plan/evidence/f06-masked-image-effects.md), and
[native docking](../plan/evidence/controls-expansion.md).

The owner subsequently authorized project initialization, Git setup, AGENTS.md and the remote. The initial workspace/facade and repository guidance are now present; see the backlog for actual evidence. Earlier planning-only statements are historical. Runtime implementation follows the M0/M1 plan within subsequent requested scope; routine work inside an authorized task does not require repeated approval.

2026-10-26 text lifecycle refinement: native activation and retained logical focus
are separate. Blur cancels transient preedit/press state and rejects late editing
events; repaint cannot reactivate a field. Reasserting unchanged editor focus
preserves typing-history groups and caret-blink phase. Hosts use the editor's
absolute next-blink deadline, waiting without a timer when no caret can blink.
Page movement uses measured content height and the same preferred horizontal
position as vertical arrows. Geometry-sensitive input refreshes CPU text layout
before hit-testing/navigation when queued edits have not yet painted. Reason:
prevent stale-revision failures, unintended focus restoration and idle busy loops
without adding a background task or GPU presentation to input handling. Contracts:
[text editing](../guides/text-editing.md); acceptance:
[integration](../plan/evidence/m8-integration.md) and
[selected desktop](../plan/evidence/m5-automation.md).

Caret reveal is requested by explicit editing/navigation/focus activity and
password presentation-mode changes. Ordinary preparation clamps saved offsets
without pulling a manually scrolled view back to the caret. The request is a
bounded control-local stamp, including conservative mutable-editor access; masked
scrolling uses the same grapheme coordinate adapter as masked hit-testing. This
preserves host wheel intent while keeping current caret geometry available after
an edit. Regression evidence is in `text_reveal_scheduling.rs` and the integration
report linked above.
