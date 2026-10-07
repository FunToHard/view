# view project plan

Revision: 0.4 — 2026-10-07. Bootstrap update: the owner authorized project initialization. Git, the initial Cargo workspace/facade, pinned compiler, licenses and contributor guidance now exist. Runtime implementation, dependency qualification and CI remain backlog work.

M0 execution update: the owner requested direct completion without agents. Platform
and graphics/text qualification probes, independent consumer, fixtures and CI
workflows now exist. [Evidence](evidence/m0-qualification.md) and the canonical
checklist distinguish completed local validation from unexecuted remote/platform
gates. No full M0 or M1 completion is implied by the workflow files.

This plan turns the selected design into executable milestones. The [decision baseline](../design/09-decisions-and-questions.md) records resolved choices and their provenance. The [repository plan](repository-and-maintenance.md) defines the monorepo/workspace and release policy. The [webview contract](../design/15-webview-lifecycle.md) defines disposal and hot retention. The [initial backlog](initial-backlog.md) makes M0/M1 tasks reviewable before implementation begins.

## Scope and release boundary

The [ordered implementation checklist](implementation-checklist.md) expands these
milestones into sequential tasks and is the canonical progress record. The initial
backlog retains supporting BOOT/RUN descriptions and historical bootstrap evidence.
Architecture and release scope remain defined by this plan and the decision register.

The first release is a Windows 11 x64 native developer preview. GitHub Actions runs the defined Windows/Linux/macOS build and portability tests; Linux/macOS receive no claim of full product parity. MIT OR Apache-2.0 is the planned first-party framework license. These are delegated planning defaults. The preview demonstrates retained/declarative/immediate composition, standard text/control behavior, compatible 2D/3D viewports, network-driven state, automation including MCP, optional native webview panels with both lifetime policies, and one process-based add-on proof.

The examples prove contracts, not complete game engines, IDEs, storefront backends, or Word/image-editor products. Optional features remain separate packages/features so a simple app does not pay for webview/MCP/add-on stacks.

Deferred deliberately: web/mobile delivery, ARM64 product support until selected separately, rich document layout/pagination, full docking, advanced PBR/physics/asset engines, production color management/HDR, arbitrary native GPU resource sharing, VM/DLL plugin backends, stateful Rust hot reload, and transformed GPU composition of webviews. These are closed scope decisions for this release, not forgotten questions.

## Milestones and work packages

| Milestone | Work packages | Dependencies | Exit evidence |
| --- | --- | --- | --- |
| M0: engineering bootstrap | Initialize workspace/toolchain; qualify compatible dependencies/MSRV; establish CI jobs and owned fixtures; document registry/license metadata | Implementation phase; selected defaults and provisioned runners | Small Windows app and actual Linux/macOS test execution; dependency/features report; runnable contributor commands |
| M1: shared runtime and native shell | Generational IDs, lifecycle, actions/revisions, event ordering, constraints, focus/capture, native decorated window, semantic snapshots, controlled clock/harness | M0 | T-01 through T-05 at the applicable layer; native shell smoke; idle-work instrumentation |
| M2: text and standard controls | Text backend, shared editing session, core widgets, profiles, styling, IME/clipboard integration, package-level conformance runner | M1 | Headless portions of T-29 through T-31; independently expected edits; native/render portions complete with M3/M5 before certification |
| M3: 2D/3D and host rendering | Quad/path/text rendering, clips/layers, resource budgets, simple camera/mesh/depth/picking, custom/host-owned rendering contract | M1; text integration from M2 | T-06 through T-12; real offscreen GPU output; reference render/provider integration |
| M4: authoring and application proofs | Canonical Rust builders/Element branches, retained handles, immediate regions, Resource<T>, async assets, two-window editor proof, native storefront fixture, community Markdown package | M2 and M3 | T-16 through T-19 and T-27/T-28; no private framework patches; network reorder/cancellation tests |
| M5: external tooling and physical E2E | CLI, inspector, app command schemas, MCP stdio adapter, revision waits/captures, dedicated desktop input driver | Core inspection starts in M1; full flows need M4 | T-14/T-22 through T-26; Rust/CLI/MCP semantic parity; real mouse/keyboard/IME evidence with honest coverage |
| M6: optional webview panels | Windows WebView2 binding, native rectangular hosting, profile/ownership management, DisposeAfterUse and KeepHot, bridge limits, lifecycle instrumentation | M1; M5 for native E2E | T-35 through T-38; dedicated cleanup receipts, shared-session preservation, hot reuse and failure handling |
| M7: optional runtime add-on proof | Child-process host protocol, host-rendered UI patches, contribution slots, typed app document commands, lifecycle/version limits | M4 and M5 | T-32 through T-34; review panel and one undoable document transaction; no false sandbox claim |
| M8: hardening and release candidate | Optional diagnostics, device/process recovery tests, dependency/feature packaging, documentation, benchmark baselines, native behavior sweep | M2 through M7 | T-13/T-15/T-21 plus all applicable feature gates; profile/support matrix, migration notes and reproducible artifacts |

M2 and M3 are independent work streams once shared interfaces settle, but no team size or delegated execution is assumed. For one maintainer, take short vertical slices rather than leave two large branches unmerged. Automation, accessibility, diagnostics breadcrumbs, and conformance are incremental from their first dependencies, not postponed until the named final integration stage.

## First executable increment

Project initialization is authorized and underway. The next implementation increment follows M0 and the smallest M1 slice:

1. Create the workspace with core/platform/facade/testing boundaries and a qualified stable Rust toolchain.
2. Establish format/check/lint/test jobs and run a real trivial Rust test on each OS runner.
3. Open a native decorated Windows window; maintain a small runtime tree with keyed identity and focus.
4. Inspect that tree through a headless test interface and observe a coherent revision after a queued action.
5. Verify unmount cancels ownership and an idle runtime does not continuously rebuild.

Review those interfaces before expanding controls or graphics. No full widget catalogue, browser runtime, or plugin loader is needed to prove this increment.

## Issue template and completion rules

Each work item records requirement/capability IDs, public behavior, affected contracts, dependencies, expected tests, platform coverage, dependency cost, and a bounded deliverable. Distinguish implementation, evidence, and infrastructure tasks. A missing runner blocks its coverage gate; it is not silently converted to a pass.

Definition of done: public behavior documented, selected interfaces used, tests pass at the relevant layer, known limitations and unsupported capabilities are explicit, resource/lifecycle behavior reviewed, no unwanted feature dependencies, and examples/tool schemas updated. Any public API change includes a compile-tested consumer when implementation exists.

## Validation and infrastructure

Baseline jobs run formatting, Clippy, compilation, runtime/text/semantic tests, and package/feature consumer tests on Windows/Linux/macOS. Native Windows E2E needs a dedicated interactive session; GPU correctness needs verified compatible adapters or explicitly labeled software backends. Graphics/performance evidence uses named hardware and recorded driver/backend/fixture manifests.

GitHub Actions is the selected provider; job contracts remain portable. Hosted CPU runners alone do not satisfy GPU or desktop gates, and a Windows hosted build runner does not certify Windows 11 native behavior. Runner provisioning, labels, credentials, capacity, and cost are setup work, not resources assumed to exist. Do not run untrusted contribution code on a privileged personal/interactive runner without an isolated execution policy.

Measure p50/p95/p99 UI/scene/composition times separately, idle counters, memory after repeated lifecycle cycles, uploads, input latency, and hot/cold webview costs. Capture reference measurements when executable fixtures exist, then record numeric regression limits with repeatability evidence. Do not invent benchmark results or freeze hardware-independent timing promises before measurement. No unnecessary continuous work and no unbounded lifecycle growth are testable requirements from the first increment.

## Release deliverables

The developer preview includes the selected library packages, minimal/gallery examples, editor/storefront/embedding proofs, tool adapters, control profile reports, native webview lifetime tests, add-on protocol example, environment manifests, and known limitations. Experimental modules are labeled and individually disableable.

The release excludes claims of full Linux/macOS product support, universal native interop, safe in-process plugin sandboxing, or perfect crash recovery. Documentation is explicit about which tests actually ran. Crate names, license metadata, dependency licenses and package contents must pass release checks. Code signing and installer publication are separate distribution operations.

No dates are invented without staffing and infrastructure information. Review progress by milestone evidence and remaining blocking tasks. The initialization request covers repository setup; it is not a commitment to complete the whole framework in one change. Continue subsequent work within the owner's requested scope.
