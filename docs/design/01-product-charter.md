# Product charter

Status: requirements and selected technical baseline. Revision: 0.4.

## Purpose

view should let a Rust developer build a complex native application without treating custom graphics, automation, or OS integration as exceptions. Controls, document canvases, scene viewports, and native windows participate in a coherent runtime.

The intended capability is broad enough for game-engine editors, IDEs, Photoshop-like tools, and other editing software. A first release will prove selected workflows rather than claim feature parity with those mature products.

## Owner requirements

| ID | Requirement |
| --- | --- |
| R-01 | Name: view. Tagline: The devil's framework. |
| R-02 | Framework implementation in Rust with a wgpu backend. |
| R-03 | Minimize third-party dependencies, favoring maintained and dependable components. |
| R-04 | Retained UI with on-demand immediate UI; both are supported authoring modes. |
| R-05 | Flutter-like syntax while remaining writable as Rust code; canonical builders are selected in Q-02. |
| R-06 | Windows is the first product scope; Linux/macOS receive CI/CD-based tests. |
| R-07 | First-class 2D and 3D capabilities with native performance ambitions. |
| R-08 | Support demanding creative/developer applications and planned integration paths. |
| R-09 | Respect native OS behavior, including window frames and title-bar integration. |
| R-10 | Discover framework features broadly; decide runtime plugin loading later. |
| R-11 | Include E2E automation with or without MCP as a major iteration capability. |
| R-12 | Develop and refine the design before starting implementation. |
| R-13 | Optional webview panels let the application choose cleanup after use or retained hot background loading. |

## What “native performance” means here

It is a target to demonstrate, not an existing result or an unconditional speed guarantee. The proposed interpretation is:

- Native Rust application/runtime execution and GPU work through wgpu.
- No mandatory browser, scripting VM, or IPC round trip on local widget/input/render paths.
- Demand-driven work at idle; independently scheduled continuous rendering where needed.
- Reuse of layout, geometry, glyphs, textures, and GPU resources when valid.
- No required CPU pixel readback to composite a compatible GPU viewport.
- Bounded queues, explicit memory budgets, and observable frame pacing under load.
- Measured CPU/GPU time, interaction latency, memory, and scaling on named hardware.

Rendering a 3D scene continuously must not require rebuilding a static inspector every frame. Moving the inspector must not require reconstructing the scene. An application may choose lower latency, quality, or energy use through explicit policies.

## Application proofs

| Proof | Workflow | Pressure on the architecture |
| --- | --- | --- |
| Engine editor | Scene viewport, selection, transform gizmo, hierarchy, inspector, detached window | Shared GPU composition, picking, action routing, state identity, multiple windows |
| IDE | Large project tree, text buffers, search results, command palette, async diagnostics | Virtualization, Unicode/IME, background work, cancellation, keyboard-first interaction |
| Image editor | Layer stack, pan/zoom, tiled image, brush preview, undo, color-aware display | Low-latency input, texture budgets, partial updates, effect passes, document/view separation |
| Network-driven storefront (S-01) | Native navigation, catalog/search, remote page sections, async images, progress and offline state | Resource subscriptions, branch identity, out-of-order data, bounded asset loading, server-described UI adapter |

These are acceptance fixtures, not commitments to build complete engines, language servers, image-processing suites, or a new web browser engine inside the core. The owner clarified that the storefront example means network-driven native application UI, not building a browser shell. See [network-driven UI and web portability](11-network-ui-and-web.md).

## Product boundary

The core provides composition, lifecycle, identity, input, layout, semantics, scheduling, and rendering integration. First-party modules may provide 2D drawing, a basic 3D scene path, docking, virtualization, command history, and tooling.

An engine may own its ECS, scene graph, physics, materials, and frame loop. An image editor may own its document graph and processing pipeline. view must provide supported interfaces for these systems rather than requiring translation into one universal scene or application model.

Basic first-party 2D and 3D demonstrations are required to prove that this support is real. Providing only an opaque “custom draw callback” would not satisfy the intended design.

## Scope boundaries

Initial supported delivery is Windows. Linux/macOS testing catches regressions and keeps platform boundaries honest; it does not by itself establish full product support there. CI coverage is described by actual tests and runner capabilities.

Mobile delivery, a full game engine, a full image-editing engine, in-process binary plugins, and arbitrary zero-copy interoperation with every native renderer are not initial promises. Web delivery remains deferred, with a possible capability profile documented for future evaluation; it must not constrain the native core to browser limits. Each future capability should remain possible without weakening the Windows product contract.

Optional crash diagnostics and application-integrated automation should respect host ownership. A native E2E profile can deliberately control desktop mouse/keyboard input; ordinary tests remain nonintrusive. These selected contracts are covered by the tooling and diagnostics documents.

Standard controls are required to be behaviorally complete for their declared profile, so a themed text field cannot omit deletion or selection. Community authors publish controls as normal crates. Independently installed Word-style add-ons use the selected native child-process protocol for the initial release. See [controls and conformance](13-controls-and-conformance.md) and [runtime add-ons](14-runtime-addons.md).

The authoring experience should be learnable from ordinary Rust and concise versioned examples by people and models without prior view-specific training. This is a discoverability/testability objective, not a promise of error-free AI generation.

The [decision baseline](09-decisions-and-questions.md) selects Rust plus WGSL, Windows 11 x64, SDR composition and a basic first-party 3D module. GitHub Actions and MIT OR Apache-2.0 complete the planning baseline. These platform/provider/license defaults were chosen under the owner's request to finalize decisions, not as explicit questionnaire answers. The [project plan](../plan/README.md) defines implementation work and validation.
