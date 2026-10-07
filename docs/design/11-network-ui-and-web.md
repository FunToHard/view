# Network-driven UI, storefront proof, and web portability

Status: revision 0.4 selected P-13/P-14 baseline; P-15 web delivery deliberately deferred. Owner clarification: a native network-driven storefront, not a browser shell. Windows-first delivery remains unchanged.

## Network requests update state, then UI

Normal Rust branching should select UI from application state. A request is an application effect, initiated by an action, subscription, or explicit component lifecycle operation. Neither rendering nor repeated layout starts the request.

```text
User action / subscription
    -> request with owner identity and request generation
    -> asynchronous completion message
    -> UI-thread reducer validates and updates model revision
    -> invalidate subscribed regions
    -> rebuild/reconcile affected branches
    -> layout/paint/semantics commit
    -> requested presentation
```

The same flow updates retained properties or schedules an immediate region. Transport code never mutates runtime nodes from a worker. Network updates do not force the whole window or scene to rebuild.

## Rust branch example

This is an unimplemented API sketch; Element is a proposed deliberate type-erasure boundary for branches with different concrete widget types.

```rust
enum LoadState<T> {
    Idle,
    Loading,
    Ready(T),
    Failed(AppError),
}

fn results_view(model: &SearchModel) -> Element {
    match &model.results {
        LoadState::Idle => Text::new("Enter a query").into_element(),
        LoadState::Loading => Progress::new().into_element(),
        LoadState::Ready(items) => Results::new(items)
            .key("search-results")
            .into_element(),
        LoadState::Failed(error) => Column::new((
            Text::new(error.summary()),
            Button::new("Retry").on_press(Action::RetrySearch),
        ))
        .into_element(),
    }
}
```

Rust if/match/loops are the public starting point. An Either-style typed branch is another possible representation; no template language or reflection is required. Returning impl View alone does not permit arbitrary unrelated concrete return types from different branches. Q-02/Q-15 must choose the common conversion/branch abstraction rather than hiding this Rust constraint.

Requests run outside this function. Entering Ready does not initiate a second fetch. Retry is an action that changes request state and issues a new effect.

## State ownership across branches

Reconciliation retains a compatible keyed node in the same structural scope. Replacing a branch normally unmounts its old nodes and cancels view-owned subscriptions/effects. Model/document state can survive outside that view; cache-backed data requests may intentionally outlive it under a different owner.

A key alone does not resurrect a previously unmounted editor or preserve it across incompatible parents/types. For refresh, prefer keeping content mounted while showing a loading indicator, or use an explicit keep-alive facility with lifecycle and memory limits. Specify the focus destination when loading/error/content replaces a focused subtree.

Remote data normally updates typed application models. If an application needs server-described UI, it uses an explicit versioned schema mapped to an allowlisted local widget registry, bounded resources, validated values, and supported actions. Arbitrary remotely supplied Rust code is a separate execution system, outside this design.

## Async correctness contract

- A request carries its logical owner, owner generation, query/resource key, and request generation.
- Latest-request-wins is a useful policy for search/preview, not a universal rule for every stream or mutation.
- A completion is accepted only if its policy and owner are still current. Cancellation is best effort; validation remains required.
- For ordered streams, sequence/version checks and explicit resynchronization handle gaps and reconnects. Coalesce replaceable previews under backpressure without dropping required document events.
- Optimistic mutations track base revision and pending operation identity. Rollback, rebase, or conflict UI is application policy. Timeout/cancellation does not prove a server mutation failed or was undone; safe retry may need server idempotency.
- Batch related model changes into one coherent UI commit where possible. Authentication/error states and reconnect indicators are ordinary branches with explicit lifecycle behavior.
- Tests control delays, out-of-order completions, cancellation, errors, and reconnects through a fake service boundary. Real transport integration tests are separate.

view should expose effect completion, cancellation, and wakeup interfaces without forcing reqwest, Tokio, or any specific networking stack into its core. Native applications may use threads/executors; browser code uses the browser's asynchronous facilities. Portable APIs must not require blocking calls or Send futures when the platform only permits local tasks.

## S-01: native network-driven storefront proof

The owner clarified that Steam/Epic-like storefront behavior is the example: content and changes arrive over the network, while view supplies the application UI. This document makes no claim about how those products internally implement their interfaces. No browser or embedded web engine is required for this proof.

The fixture has a native shell, navigation, search, a virtualized catalog, product detail panels, asynchronously loaded images, simulated download progress, cached/offline content, and server-controlled page sections. A local fake service generates deterministic responses and delays. It complements the engine-editor, IDE, and image-editor proofs rather than replacing their graphics/input requirements.

There are three useful levels of remote behavior:

| Level | Server supplies | Client responsibility |
| --- | --- | --- |
| Remote data | Products, images, status, search results | Locally authored Rust components and ordinary branching |
| Remote configuration | Ordering, featured sections, feature flags, display parameters | Typed validated configuration selecting known local components |
| Server-described UI | Versioned page/component descriptions | Compile descriptions into a bounded, supported local component registry |

All three can use view's normal identity/layout/input/semantics/automation paths. Server-described UI is an optional adapter, not a mandatory alternate runtime or executable-code channel. The first proof should establish remote data/configuration; a small section-schema fixture can validate the advanced boundary without promising a general web replacement.

## Low-friction resource API proposal

Provide a transport-independent Resource<T>/subscription abstraction, or equivalently documented adapters, carrying a request key, request generation, loading/error state, optional last-good value, progress, and explicit refresh/cancel behavior. Developers can bind it to a component without hand-writing UI-thread dispatch for every response. Cache ownership, deduplication, retry, stale-while-refresh, and persistence are explicit policies rather than surprising globals.

The mechanism does not dictate HTTP, a particular executor, application authentication, or the document data model. Native application adapters can implement real networking while deterministic fake adapters use exactly the same interface. Subscribe by resource/field revision so one progress update does not rebuild the entire storefront.

Images and other assets decode off the UI path when supported and upload through the resource scheduler. Bound download/decode/upload queues and memory, prioritize visible content, and release obsolete work. Do not replace input responsiveness with synchronous downloading or texture creation during build.

Server-described sections use stable server/application IDs, schema versions, capability negotiation, bounded depth/item counts, and validated asset references. Unknown optional components use a declared fallback; unsupported mandatory features fail visibly. Remote commands map to locally authorized typed actions; a page response cannot enable desktop automation, execute arbitrary code, or bypass application policy.

An atomic page-model update can reconcile insertions/reordering without losing the state of compatible keyed items. Preserve independent local state such as search text, focus, scroll anchors, and pending edits. Remote removal of a focused item has a defined fallback. Schema refresh and asset completion are independent revisions so a late image does not revert the page.

## Is web delivery suitable?

Recommendation after the owner's clarification: keep web delivery deferred and record a possible capability profile now. Do not constrain the native core to a browser's lowest common feature set, and do not promise one-click Flutter-like platform parity. A future feasibility experiment needs a concrete web use case and separate authorization within the implementation plan.

wgpu provides WebGPU and WebGL2 backends on WASM. This makes graphics portability plausible but does not supply the complete application platform. Native-only GPU features and a WebGL2 fallback need separate profiles rather than an assumption of equivalent features. [wgpu backends](https://docs.rs/wgpu/latest/wgpu/)

3D is not by itself an obstacle: compatible mesh/depth/material rendering can run through WebGPU. The deciding question is which GPU features, platform services, memory/latency characteristics, and integrations each application actually requires. Native-only providers should declare requirements rather than accidentally compile to incomplete behavior on web. A future web target can support a documented subset; an application that requires an absent primitive reports incompatibility or supplies its own tested alternative.

Winit can target wasm32-unknown-unknown and represents a web window with a canvas. Its current documentation lists CSS transforms/border/padding caveats affecting coordinates. The web adapter must specify supported canvas styling and test pointer conversion rather than assuming arbitrary CSS works. [Winit web platform](https://docs.rs/winit/latest/winit/platform/web/index.html)

WebGPU is available in secure contexts and must be capability-detected. Browser/device/driver support and requested limits are runtime conditions. A missing adapter is an explicit unsupported result or a tested lower-feature fallback. [MDN WebGPU](https://developer.mozilla.org/en-US/docs/Web/API/WebGPU_API)

## Platform differences to design for

| Concern | Web contract |
| --- | --- |
| Scheduling | Browser-controlled frame/event loop; no blocking main thread; handle hidden-tab throttling |
| Graphics | Negotiate web features/limits; WebGL fallback is optional and tested independently |
| Input | CSS pixels, canvas backing scale, focus, text composition, clipboard and pointer-lock restrictions |
| Semantics | Implement a browser accessibility bridge; drawing canvas pixels does not expose widget semantics |
| Network | Browser requests and origin policies; no assumption of unrestricted native sockets |
| Files/assets | Explicit browser storage/import/export adapters and font resources |
| Threads | Do not make native threads a baseline prerequisite; workers/shared-memory configurations need separate validation |
| Windows | Views within a page; no HWND, native title bar, unrestricted process access, or desktop input injection |
| Automation | External browser driver and app protocol bridge; a page cannot take over desktop mouse/keyboard |
| Diagnostics | Browser console/error and app breadcrumbs; no native process dump inside ordinary WASM |

Cross-origin fetch uses the browser's CORS policy; changing the UI framework cannot bypass it. Cancellation can be exposed through browser request facilities while preserving the owner/revision rules above. [Fetch API](https://developer.mozilla.org/en-US/docs/Web/API/Fetch_API/Using_Fetch)

The Rust application can remain Rust-authored, but browser deployment needs web bindings and bootstrap/HTML assets. Web delivery does not automatically meet native latency/memory targets. Measure download/startup cost, interop overhead, frame pacing, memory, and browser-specific behavior. Ordinary document/content websites are not the primary justification for a canvas-based framework; graphics-heavy tools, previews, and app distribution are stronger candidates.

## Proposed proof checks

Out-of-order search replies cannot replace newer results. Navigating away cancels a view subscription without deleting application-owned cached data. Catalog updates preserve compatible card identity and scroll anchors; slow images and progress streams do not stall keyboard interaction. Unknown server section types follow the declared fallback and cannot invoke privileged commands.

If web feasibility is later selected, a WASM fixture handles resize, network completion, text focus, semantic activation, and a minimal 3D viewport under declared capabilities. It reports unsupported providers explicitly. The delivery plan tracks this as deferred feasibility, not a current v0.1 commitment.
