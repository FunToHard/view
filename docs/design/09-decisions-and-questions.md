# Decision baseline and refinement log

Revision: 0.4 — 2026-10-07. Baseline selected under the owner's explicit request to finalize decisions and continue planning. Windows 11 x64, GitHub Actions, and MIT OR Apache-2.0 are delegated defaults, not explicit questionnaire answers. No implementation is authorized by this planning update.

## Accepted requirements

R-01 through R-13 in the [product charter](01-product-charter.md) record explicit owner direction. They are requirements, not claims of implemented behavior.

The latest scope refinement establishes Windows as the first product target, Linux/macOS CI testing, complex 2D/3D applications, native OS integration, the name/tagline, and creation/refinement of design documents. It broadens the earlier desktop-controls framing substantially.

Revision 0.2: the owner confirmed that the documents fit the design criteria and requested deeper treatment of network-driven UI, web suitability, crash handling, application-integrated MCP, and physical hit-test/scroll/keyboard E2E. They clarified that the intended additional proof is a native network-driven storefront, not a browser shell. WASM/web remains a concern to evaluate without restricting native primitives. Positive design feedback does not close every open API/dependency choice or authorize implementation.

Revision 0.3: the owner asks for low-friction UI authoring by models without view-specific training, shareable community control crates, standard controls that do not omit expected editing behaviors, and runtime add-ons with custom UI in a Word-like application. These guide the new proposals below; no loader/runtime dependency or final syntax is selected by this update.

## Selected architecture

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
[M1A runtime report](../plan/evidence/m1a-runtime.md). Performance, native behavior
and new Linux/macOS execution are not inferred from Windows CPU checks.

### Revision procedure

1. Capture feedback as an existing requirement/decision change or a new numbered entry.
2. Record the decision, reason, alternatives, and affected contracts.
3. Update all affected documents in the same revision.
4. Add or revise the acceptance scenario that demonstrates the decision.
5. Check for contradictions, broken links, unlabelled assumptions, and unsupported claims.

Major decisions can graduate to individual ADR files when their alternatives need more detail. Keep this register as the index; do not create parallel contradictory “final plans.”

The owner subsequently authorized project initialization, Git setup, AGENTS.md and the remote. The initial workspace/facade and repository guidance are now present; see the backlog for actual evidence. Earlier planning-only statements are historical. Runtime implementation follows the M0/M1 plan within subsequent requested scope; routine work inside an authorized task does not require repeated approval.
