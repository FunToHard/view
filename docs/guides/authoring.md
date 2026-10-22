# Authoring reference — view 0.1

Rust 1.98.1 / edition 2024. The public facade is `view`; no macro, global
model, executor, HTTP library or renderer is needed for composition. All packages
remain unpublished previews. This guide describes compiled APIs, unlike the
historical design sketches.

## Composition and ownership

`view::authoring::{Text, Column, ColumnProps, Element, IntoElement, Children,
keyed, reconcile}` supplies the canonical nested composition boundary.
`Text::new(key, text)` requires a stable key. `Column::new(key, children)` accepts
fixed tuples (one through six children), `()` or `Vec<Element<M, A>>`.
`Column::with_props(ColumnProps { width, row_height, gap })` is the uniform-row
layout profile; variable-height/custom layout uses `Description::layout` or the
existing public layout engines. This is not a complete responsive layout system.

`Description::new(key, state, handler).into_element()` is the extension point.
The owned handler is `FnMut(&mut State, &mut Model, Action) -> Dirty`. It borrows
the current application model only during dispatch. Actions are queued with
`Runtime::enqueue`, then executed by `flush`; describing UI does not execute them.
`Element` erases each description once and owns its descendants. Ordinary `match`
or `if` returns `Element<Model, Action>` via `.into_element()`.

`reconcile(runtime, declarative_parent, children)` validates duplicate keys and
geometry throughout the supplied tree before reconciling. Depth is limited to
64 and nodes to 10,000 per call. Allocation/revision exhaustion is still a runtime
error, not a general rollback guarantee. Stable parent/key/state-type/structure
preserves local state and replaces handlers/properties. Removed or incompatible
nodes lose local state and revoke their resources. Keys do not resurrect state.

Retained leaf elements preserve their independently mounted descendants.
`Runtime::mount/update/state/unmount` and generational `NodeId` are the supported
retained-handle API. An element with children owns them declaratively; it cannot
also give an immediate builder authority over those same children. Immediate
islands use `Structure::Immediate` and `Runtime::set_region_builder`. The builder
receives immutable model access and once-consumed response revisions. An idle
region is skipped without disposing state or replaying actions.

## Branch lifetime and resources

`view/resources` enables `view-resource`, which depends only on core/std.
Create a `Runtime::own(owner, ResourceKind::Subscription, cancel)` lease and pass
its `cancellation()` to `Resource::new`. Use an application root as owner when
data should outlive a view; a view-owned lease is revoked on unmount/window close.
The resource and lease must belong to the same owner; the app adapter constructs
that association. A cloned request cannot prolong its owner's lifetime.

`begin(key)` returns a worker-safe `Request`; the application launches the effect
outside build/layout. `complete`, `fail` and `set_progress` validate namespace,
owner generation, key, request revision and cancellation. A completion is accepted
at most once. Call them on the UI thread, then `notify(runtime)` to invalidate
subscribers once per resource revision. Unmounted subscribers are pruned.

`Policy` declares retention during same-key refresh, retry count/delay and opt-in
reconnect. Changing query clears the previous query's value. Successful empty
data is `Ready(empty)`, distinct from loading/failure. Failures may retain the
last good value. `retry(now)` and `reconnect()` return effects; neither starts a
timer or network client. Retry policy applies to idempotent retrieval, not remote
mutations. Deduplication and persistent storage remain application policies.

Keep content mounted while refreshing to retain edit/focus/scroll state. On actual
branch removal, resolve focus to an app-selected live fallback. The storefront
uses its retained search field. Document text/selection/undo that must survive a
window belongs to the app model, not an unmounted component.

`stream::Mailbox<T, P>` has a bounded required FIFO and one coalescing preview slot.
`send` returns the original event on saturation: retry or apply backpressure;
never discard required document events. The payload type/app adapter must impose
byte limits in addition to queue capacity. `Sequence` rejects duplicates and
requires an authoritative resynchronization after a gap or epoch mismatch.
The proof executor caps outstanding jobs at four, uses workers for fake retrieval,
and requests native loop wakeups only while work remains.

`assets::Cache<T>` has explicit item/byte limits, host-clock expiry, stale reads and
LRU eviction. Callers account actual payload bytes; no hidden global cache exists.
Raw `Image::rgba` validates decoded size. Optional `view/image-bmp` enables only
uncompressed 24-bit Windows BMP (40-byte DIB, row padding, top/down orientation).
No codec is in the minimal facade. Download bytes, decode budgets and upload queues
must all be bounded by the adapter; decode on workers, upload through the renderer
after completion. There is no implicit fetching from image/Markdown references.

`sections::Registry` compiles a typed versioned schema to local controls/actions.
The transport separately bounds raw input bytes. Registry validation bounds depth,
item count and text/key/action/asset bytes, rejects duplicate sibling keys,
unknown actions/assets and unsupported versions, and substitutes an inert fallback
for unknown optional controls. Unknown mandatory controls fail. Even fallback
subtrees are validated; a remote page cannot add capabilities or executable code.

## Compiler-checked pack

All eight examples live in `crates/view/examples/` and run without native input:

| Example | Exercise | Features |
| --- | --- | --- |
| `minimal` | Tuples, text and column | none |
| `form` | Shared text editor, validation and typed commit | `controls` |
| `keyed_list` | Reorder keeps identity | none |
| `async_branch` | Explicit retrieval then branch reconciliation | `resources` |
| `two_windows` | Shared app document, independent window lifetime | none |
| `immediate` | On-demand build and once-only action | none |
| `custom_control` | Public description/layout/semantics | none |
| `viewport` | Demand scheduling independent of UI | none |

Run `cargo run -p view --features controls,resources --example <name> --locked`.
The [application proof](../../examples/application-proofs/src/lib.rs) adds scene
selection, inspector, capture/undo, virtualized catalog, worker replies, offline
refresh/reconnect and detached Markdown preview. Run:

```text
cargo run -p view-application-proofs --locked
cargo run -p view-application-proofs --locked -- --gpu dx12
cargo run -p view-application-proofs --features native --locked -- --run-hidden
cargo run -p view-application-proofs --features native --locked -- --show
```

Only `--show` intentionally displays windows. Native proof is Windows/DX12 first;
the GPU-only command also accepts `vulkan`/`metal` for separately qualified hosts.
The fake service is not a live storefront backend. The proof's clipboard is
memory-only, not the user's OS clipboard. Physical input and native service
certification are outside these deterministic application checks.

In the native proof, select the object through the hierarchy or viewport, drag
the gizmo, use Right Arrow for a numeric transform step and Ctrl+Z for document
undo. Store/Library/Downloads are local navigation actions; enter a search and
press Enter to request the bounded fake catalog. The detached source editor uses
shared text editing and Ctrl+Enter for a worker preview update. Further edits
cancel the old preview request; closing the window preserves the app document.
Tab traverses declared focus targets and Enter activates focused links without
opening external programs. These interaction routes are implemented, but were
not physically exercised on the user's desktop.

See [community controls](community-controls.md), [embedding](embedding.md), and
[M4 evidence](../plan/evidence/m4-authoring-proofs.md) for contracts and limits.
