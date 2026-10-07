# Runtime add-ons and custom UI

Status: revision 0.4 selected P-22 baseline: optional trusted native child-process add-ons. VM/DLL/scripting backends are deferred; comparisons below explain their future tradeoffs.

## Runtime extensibility does not require JIT

A package can be compiled before distribution and loaded or started later. Rust can produce executables and dynamic libraries, including cdylib artifacts for a defined foreign interface. Windows supports runtime dynamic-library loading. The question is the extension boundary and lifecycle, not whether the source language compiles just in time. [Rust linkage](https://doc.rust-lang.org/reference/linkage.html), [Windows DLLs](https://learn.microsoft.com/en-us/windows/win32/dlls/using-dynamic-link-libraries)

## Four integration routes

| Route | Installed after app build? | UI integration | Main tradeoff |
| --- | --- | --- | --- |
| Cargo control/feature crate | Requires rebuilding host | Full public Rust composition/provider APIs | Simplest and efficient; compile-time compatibility |
| Separate native Rust process | Yes | Versioned protocol; host-rendered panels and commands | Process/IPC cost; app isolation and permissions need explicit design |
| WASM module/component | Yes | Explicit imported host functions and UI protocol | Optional runtime cost/limits; interface/runtime compatibility |
| Native DLL | Yes | Versioned C-compatible ABI and host handles | Low overhead; fully trusted in-process code and difficult unload safety |

Runtime add-ons are an optional subsystem; ordinary controls remain ordinary crates. Do not force all application developers to ship a WASM engine or process manager just to use a community widget.

Selection: a separate-process Rust add-on using a bounded, length-delimited JSON protocol over dedicated stdio, with version negotiation and host-rendered contributions. It permits AOT Rust without selecting a VM. The first release treats these executables as trusted code running with their actual OS privileges; host API scopes do not claim to sandbox the process. WASM and native DLL routes remain future options requiring a separate decision.

A separate process helps contain crashes but is not automatically an OS security sandbox. A host claiming restricted filesystem/network access must actually enforce that with its process/runtime policy, not only omit those methods from an SDK. The extension host must document its trust model.

## Word-style workflow

An example add-on contributes a review panel, a toolbar/menu command, a suggestion list, and an action that applies a document change:

```text
Load manifest -> negotiate versions/capabilities -> activate extension
    -> mount host-rendered panel and register scoped commands
    -> request permitted document snapshot/selection
    -> compute suggestions in extension
    -> publish panel update against its base revision
    -> user accepts suggestion
    -> host validates document revision and commits one undoable transaction
```

view supplies UI contributions, lifecycle, command routing, inspection, automation, and platform integration. The Word-like application supplies the document model, editing commands, access policy, persistence, and undo transactions. view cannot define every application's domain API.

The host does not give extensions raw mutable access to the document or runtime tree. Mutations use versioned document operations; a changed selection/document may cause rejection or an explicit rebase rather than editing an unrelated range.

## Custom UI model

For the selected process add-ons, use a versioned retained UI description/patch protocol rendered by the host. A future WASM adapter may reuse its logical schema. The guest can author that description using a Rust SDK with familiar builders; ordinary in-process View objects and borrowed closures do not cross the boundary.

Separate the logical property/action vocabulary from binary layout. A code generator or adapter may share schemas, but not every in-process Rust widget method can automatically be serialized. The supported subset and custom component registration are explicit.

The host owns layout, hit testing, caret/selection/IME, focus, scrolling, accessibility, theme, and final composition. A guest receives meaningful action/value events and sends batched revisions. Editing a field should remain responsive without a blocking round trip for each key. Guest validation is asynchronous, with a declared pending/commit/error policy.

Extension IDs namespace node keys, commands, resources, and automation selectors. A contribution can attach only to offered slots such as sidebars, toolbars, menus, or document overlays. It cannot impersonate application-owned chrome or bypass modal/shortcut policy just by naming a node.

Unknown required components or schemas cause a negotiated incompatibility; optional ones may have a declared fallback. UI patches include a base revision and are applied coherently or rejected with resynchronization instructions. Enforce limits for tree size, update rate, outstanding requests, resource memory, and task time as appropriate to the backend.

## Featureful host controls and third-party components

An add-on text field uses the host's standard text behavior profile. The add-on does not reimplement Delete, IME, or platform shortcuts just because it supplies a custom panel.

Hosts may compile community control crates into a component registry available to extensions. An add-on cannot assume that a new native control implementation is present in an already shipped host merely by sending its crate name. Truly new control behavior requires a supported module/runtime route or a contract for lower-level canvas content.

A sandboxed/external custom canvas can submit a bounded display list, semantic/hit regions, and host resource requests. That is a distinct capability, not an unrestricted wgpu Device handle. Advanced external GPU surfaces require the rendering interop contract and platform support; no universal zero-copy guarantee is implied. High-frequency gestures and basic control behavior remain local where possible.

## Lifecycle and failure

Lifecycle states include installed, negotiating, active, deactivating, failed, disabled, and removed. Deactivation revokes commands and subscriptions, cancels work, resolves focus/capture/IME, removes UI contributions, and retires resources before destroying execution state.

Tag callbacks and requests with extension generations. Late responses cannot update a newly loaded instance. A failed extension displays a host-owned error state and loses its command authority; application documents remain owned by the host. Persist extension settings/state under a versioned application policy.

Updates may require restart; runtime installation is not a promise of stateful hot reload. Safe native DLL unload is particularly difficult while callbacks, function pointers, objects, threads, or GPU work remain reachable. An initial native route should prefer keeping code loaded until process exit rather than claiming arbitrary unload is safe.

## ABI and WASM details

Rust's native ABI has no stability guarantee. A DLL SDK must define a versioned C-compatible interface, opaque/generational handles, explicit allocation/release ownership, callback lifetimes, threading rules, and error encoding. Do not pass Rust String, Vec, trait objects, or compiler-dependent layouts directly as the public binary contract. Do not permit panic unwinding across an ABI that forbids it. [Rust ABI](https://doc.rust-lang.org/reference/items/external-blocks.html#abi)

A C-compatible ABI describes the calling/data contract; the host and extension implementations can both remain Rust. Compatibility still requires negotiated API versions and a defined data layout, not merely an extern declaration.

WASM does not require web deployment or a browser renderer. A Rust guest can run in a native host's optional interpreter. Wasmi is one Rust interpreter candidate demonstrating that a no-JIT route exists; no runtime is selected yet. WIT can describe Component Model interfaces, but choosing WIT does not mean every core-WASM interpreter implements the Component Model. Validate guest ABI, component support, host bindings, resource limits, and actual runtime compatibility together. [Wasmi](https://github.com/wasmi-labs/wasmi), [WIT](https://component-model.bytecodealliance.org/design/wit.html)

Choose capabilities granted to each instance: document read/edit scopes, named app commands, allowed assets, optional network/filesystem access, and any automation surface. UI rendering rights alone do not grant global desktop-input control, arbitrary native calls, or app MCP authority. The application decides how install/enable/permission policy is presented; the framework enforces the declared contract it owns.

## Automation and conformance

Mounted add-on UI participates in the same semantic, hit-test, focus, and E2E paths as application UI. Namespaced extension commands can be exposed through MCP only if the host enables them. Protocol discovery reports actual backend capabilities and the extension's version, not merely its presence.

Acceptance examples include a review panel using host text controls, one undoable document edit, rejected stale edits, unsupported protocol negotiation, timeout/disconnect, reload with stale response, and full cleanup of focus/subscriptions/resources. Add independent limits tests and verify that undeclared operations are rejected by the actual boundary. A native trusted module must not be mislabeled as sandboxed.
