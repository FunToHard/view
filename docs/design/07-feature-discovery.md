# Feature discovery and dependencies

Status: selected capability inventory for the project plan. Revision: 0.4. Deferred entries are explicit release exclusions.

## Classification

- **Foundation:** the contract is needed from the first runtime design.
- **First proof:** implement enough to exercise the selected demanding workflow.
- **Expansion:** supported direction after the first proof; design relevant boundaries now.
- **Deferred:** not in the selected first-release scope; requires an amended decision before implementation.

"First proof" is not synonymous with complete production functionality. The developer-preview scope is selected in the project plan; later stable-release scope depends on proof results and budget review.

## Capability inventory

| ID | Capability | Priority | Acceptance evidence / dependency |
| --- | --- | --- | --- |
| C-01 | Stable identity, lifecycle, keyed composition | Foundation | Reorder/delete preserve or cancel the correct state |
| C-02 | Retained, declarative, immediate regions | Foundation | Mixed tree; clean regions skip safely; action fires once |
| C-03 | Local/shared state, actions, async ownership | Foundation | Stale completion cannot mutate an unmounted owner |
| C-04 | Constraint layout and text measurement | Foundation | Nested resize, wrapping, DPI; no effect replay |
| C-05 | Focus, capture, shortcuts, command scopes | Foundation | Editing shortcuts coexist with scene controls |
| C-06 | Semantics and accessibility bridge | Foundation | Focus/value/action contracts exposed consistently |
| C-07 | Frame demand and instrumentation | Foundation | Continuous viewport beside idle inspector |
| C-08 | wgpu ownership and host embedding | Foundation | Host supplies device/target without competing loop |
| C-09 | Custom paint and render contributions | Foundation | Clipped viewport, overlays, resize, resource retirement |
| C-10 | Automation API and deterministic harness | Foundation | Same command semantics in Rust and external tool |
| C-11 | Native decorated windows and services | First proof | Windows integration checklist passes |
| C-12 | 2D primitives, text, images, paths | First proof | Canvas with transforms, strokes, clips, hit regions |
| C-13 | Basic first-party 3D path | First proof | Camera, mesh, depth, picking, overlay and inspector |
| C-14 | Split panes and editable properties | First proof | Interactive workspace without private patches |
| C-15 | Virtualized hierarchy/list | First proof | Large logical dataset realizes a bounded visible set |
| C-16 | Text editing, IME, clipboard | First proof | Unicode editing and native composition exercised |
| C-17 | Multiple windows and ownership | First proof | Move view/document focus across native windows |
| C-18 | Inspector, gallery, capture, failure bundles | First proof | Diagnose a failed interaction without reproducing manually |
| C-19 | Optional CLI/MCP transports | First proof | CLI can precede MCP during development; both pass parity gates for the preview |
| C-20 | Theming and resource reload | First proof | Theme change invalidates appropriate content only |
| C-21 | Application command/undo integration | First proof | Drag preview commits one reversible document command |
| C-22 | Docking, tabs, tear-off panels | Expansion | Layout serialization and focus preserved across windows |
| C-23 | Advanced text/editor facilities | Expansion | Large buffers, bidi selection, editor-service hooks |
| C-24 | Tiled images, brush/pen integration | Expansion | Bounded memory, pressure input, low-latency preview |
| C-25 | Advanced scene/material/asset modules | Expansion | Explicit quality/performance scope; no mandatory ECS |
| C-26 | Paths/effects/masks and export | Expansion | Effect bounds, color, and resource contracts defined |
| C-27 | Native title-bar content | Deferred | Native decorations ship first; custom content requires behavior certification |
| C-28 | Native/web child-view embedding | Optional module selected | Rectangular Windows WebView2 panel; arbitrary transformed/native views deferred |
| C-29 | Professional color/HDR workflows | Deferred | SDR baseline; preserve working/display/export metadata boundaries |
| C-30 | Native external GPU resource sharing | Deferred | Compatible same-device wgpu path ships; arbitrary APIs need separate evidence |
| C-31 | Runtime add-ons | Optional module selected | Trusted native child process, versioned UI/command protocol; VM/DLL/scripting deferred |
| C-32 | Arbitrary stateful Rust hot reload | Deferred | Use resource reload and application state restoration initially |
| C-33 | Mobile delivery | Deferred | Reassess platform contracts after desktop validation |
| C-34 | Network-driven state and Rust branches | Foundation | Out-of-order completion, cancellation, refresh, retry, and branch identity |
| C-35 | Native network-driven storefront proof | First proof | Remote data/configuration, resource bindings, bounded server-described sections, preserved focus/scroll |
| C-36 | Web/WASM capability profile | Deferred | No native feature ceiling; evaluate an explicit subset if a web use case is selected |
| C-37 | Optional diagnostics/crash integration | Foundation contract; OS collector selected | Host-owned diagnostics/panic hook and compatible WER setup; helper/watchdog deferred |
| C-38 | Application-integrated MCP/domain commands | First proof contract | Typed schema discovery, action dispatch, undo, async results, stale-target rejection |
| C-39 | Physical hit-test/scroll/keyboard E2E driver | First proof contract; Windows hardening gate | Explicit desktop session, real input, independent geometry and behavior evidence |
| C-40 | Learnable Rust API and versioned discovery/examples | Foundation | New readers/models can compile and validate examples without prior view-specific training |
| C-41 | Community control crates | Foundation | Separate Markdown control package works through public APIs and compatible versions |
| C-42 | Standard-control behavior engines and conformance profiles | Foundation | Text input includes Delete/selection/history/IME; styled variants pass shared suites |
| C-43 | Runtime add-on UI contributions and host commands | Optional module selected | Word-like process add-on uses host controls and revision-checked document edits |
| C-44 | Webview DisposeAfterUse / KeepHot policies | Optional module release gate | Owned-resource cleanup receipts, profile boundaries, hot leases, budgets and failure handling |
| C-45 | Monorepo/workspace maintenance and release | Foundation | Atomic cross-crate changes, independent feature/package tests, coordinated compatibility |

## Extension contracts

First-party modules and third-party crates should use the same supported widget/layout/render/service contracts. A module may declare:

- Identifier, version, compatible view contract versions, and enabled capabilities.
- Typed configuration, public actions, and inspector properties.
- Identity/lifecycle, state ownership, scheduling, and invalidation behavior.
- Measurement, clipping, input/focus/capture, and paint/render integration as applicable.
- Semantic roles/actions and automation support, including explicit absence of optional capabilities.
- Resource requirements, budget accounting, cancellation, and recovery behavior.
- Test fixtures and meaningful failure diagnostics.

Avoid a universal registration object that forces every extension to implement irrelevant concerns. Composition widgets need much less machinery than a native service or render provider. Keep contracts modular and document which responsibilities belong to the host.

Runtime add-ons use the selected trusted child-process protocol. Compile-time control crates remain ordinary Cargo dependencies. VM/native-DLL options are deferred, and arbitrary independently built Rust trait objects are not a binary compatibility contract. See [controls](13-controls-and-conformance.md), [runtime add-ons](14-runtime-addons.md), and the [Rust ABI reference](https://doc.rust-lang.org/reference/items/external-blocks.html#abi).

Standard controls declare a versioned behavior profile. Sharing a paint implementation does not establish conformance; tests verify mandatory behavior and interactions. Basic expected features such as text selection/deletion cannot be silently omitted by optional feature combinations while keeping the standard-control claim.

## Dependency policy

Minimize total maintenance and correctness burden, not just direct crate count. No dependency is certified bug-free. Record maintenance evidence, supported targets, feature flags, transitive cost, release compatibility, licensing, and failure modes. The choices below are selected architecturally; exact compatible versions and features require the M0 qualification.

| Area | Selected strategy | Qualification task |
| --- | --- | --- |
| GPU | wgpu required | Pin compatible version; validate selected backends/features and upgrades |
| Windowing | winit plus targeted platform bindings | Verify native integration and embedding gaps before adoption |
| Windows services | Official Rust Windows bindings, narrowly featured | Contain FFI and lifecycle logic in platform adapter |
| Text | cosmic-text plus unicode-segmentation for edit boundaries | Complex scripts, fallback, IME integration, fonts and caches |
| Layout | Built-in constraint layout and custom layout trait; Taffy/CSS deferred | Test rows/columns/stacks/scroll/virtual layout without claiming CSS semantics |
| Accessibility | AccessKit | Actual roles/actions and native text/focus behavior |
| Text GPU backend | glyphon with compatible wgpu/cosmic-text versions | Qualify mixed paint order/clipping and resource ownership |
| Paths | lyon tessellation | Path correctness and selected feature cost |
| GPU serialization | bytemuck with checked data layouts | Avoid ad-hoc unsafe casts and validate shader/host layout |
| Diagnostics | Optional tracing/log adapter | Core events remain useful without a specific subscriber stack |
| Protocol | serde/serde_json in protocol packages | No mandatory serialization/network runtime in core |
| MCP | rmcp SDK in optional stdio adapter | Align protocol versions without coupling base UI to transports |
| Asset formats | Feature-gated codecs/loaders | Only requested formats included; resource limits defined |
| Network | Application/service adapter, no mandatory HTTP/executor stack | Native and browser-local completions share lifecycle/revision contracts |
| Server-described UI | Optional schema/registry adapter over ordinary view components | Versioning, limits, stable IDs and local action policies; no browser engine needed |
| Web delivery | Deferred WASM/browser binding and bootstrap adapters | Evaluate a capability subset without restricting native features |
| Crash collection | Optional OS/reporter adapter | Host ownership, fault safety, symbol pipeline, privacy and dependency cost |
| Shared control behavior | Framework editing/focus/selection engines plus selected text dependencies | Reuse correctness work; shaping alone is not a complete editing control |
| Runtime add-ons | Trusted child process and bounded stdio protocol | No runtime-loader cost for ordinary control crates; no JIT requirement |
| Webview | webview2-com adapter with WebView2 runtime | Both lifetimes, owned profiles, native hosting and distribution detection |
| WASM add-on interpreter | Deferred, independent of web delivery | No VM dependency in this release |

COSMIC Text provides shaping, layout, fallback, and rasterization in Rust; it does not remove the framework's platform IME responsibility. Glyphon packages text rendering integration. Their actual compatibility must be qualified with the selected wgpu version. Taffy remains a documented future option rather than an initial dependency. [COSMIC Text](https://github.com/pop-os/cosmic-text), [Taffy](https://docs.rs/taffy/latest/taffy/), [Glyphon](https://github.com/grovesNL/glyphon)

AccessKit uses semantic tree updates and platform adapters; custom drawing still needs meaningful semantic information from view/providers. [AccessKit](https://github.com/AccessKit/accesskit)

## Discovery procedure

For each proposed feature, write a workflow, API sketch, ownership/invalidation model, accessibility/automation story, failure behavior, dependency impact, and acceptance criterion. Give it an ID, dependency links, and priority rationale.

New capabilities that require private internals indicate a missing public contract. Extend that contract deliberately and test it using an external-style example. Do not respond by adding unbounded mutable access to the entire runtime.
