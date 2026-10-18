# Viewports and host embedding

The host owns event pumping, time, device creation, queue submission and
presentation. There is no second event loop or hidden device. Use the existing
owner-thread `Runtime` to route normalized input and flush application actions.
`view::host` supplies explicit native-service handoff; `view::viewport` supplies
independent frame demand and document-neutral drag state.

The GPU compatibility boundary is **the qualified wgpu 30 API family and the same
actual device/queue pair**, not merely the same physical adapter. Use re-exported
`view::render::wgpu` types or align the host's Cargo dependency. Unrelated
D3D/Vulkan/Metal devices, arbitrary external-memory sharing and incompatible
wgpu crate versions are not automatically bridged. There is no stable Rust ABI.

The compiled [embedded example](../../crates/view-wgpu/examples/embedded.rs)
creates its own device with wgpu, supplies it through `GpuContext::from_device`
using `HostManaged` error callbacks, allocates targets, and imports them through
`import_target`. This import validates actual device ownership under a wgpu error
scope and requires a resolved, single-layer/mip, sampleable, copyable SDR target.
Validation records a pass but never submits it. Host-managed callbacks remain in
place; forward device loss with `mark_lost` and recreate device-bound state.

Each frame has one host encoder. Scene providers, an independent host-engine
shader, and UI composition record into that encoder. The host submits once and
acknowledges provider/prepared-frame submission. The independent shader does not
implement `SceneSnapshot` or `RenderProvider`: existing engines may retain their
own internal representation and import a compatible output texture. No mandatory
scene translation is required. Offscreen hosts have no presentation operation;
window hosts must designate exactly one surface presentation authority.

Normal composition samples GPU textures with no CPU readback. For diagnostics,
`PreparedFrame::encode_capture` records a copy into the same host encoder and
returns `EncodedCapture`. Call its consuming `submitted` method with the host's
submission index, then `readback`. This never submits on the host's behalf.
Discarding unsubmitted prepared/capture work also requires discarding its encoder.
Poll completion before rewriting imported content; new image IDs do not make
simultaneous reads/writes safe. The example waits per frame as a deterministic
qualification fixture, not as a recommended throughput strategy.

`HostServices` declares clipboard, IME and accessibility support independently.
Implement it using the host's native services or delegates to `view-platform`.
Unavailable services return `Unsupported`. `HostBridge` is attached to one
runtime stream: it publishes each committed semantic revision once, retries
publication failures, and includes removals through the next complete snapshot.
It never reruns actions or installs native callbacks. Host native input callbacks
normalize/enqueue events; effects stay in runtime dispatch. The offscreen example
uses a recording semantic consumer and reports clipboard/IME unavailable; this
does not certify a desktop accessibility adapter.

`ViewportScheduler` uses runtime `NodeId` ownership, a committed snapshot and
host-supplied monotonic time. `OnDemand` coalesces invalidations; `Continuous`
emits at most one frame per nonzero interval, skipping missed ticks. It reports
the next host deadline. Hidden, clipped, suspended or zero-size owners and hidden
ancestors suspend output; the host reports window minimization/occlusion and
marks geometrically clipped owners appropriately. Resume requests a fresh frame,
and unmounted/reused owner handles are pruned. A failed render should re-request
its owner. The scheduler neither rebuilds the UI nor advances simulation.
`Runtime::Counters::layouts` counts completed window geometry traversals, so UI
build, layout and commit work can be checked independently of viewport frames.

For a gizmo, use `SceneSnapshot::project_overlay` for a visible logical anchor,
paint the handle after the scene, and mount its hit region in the ordinary UI
tree. A routed primary Down acquires runtime pointer capture. `DragSession<T>`
holds original/preview values; routed Move updates preview and primary Up returns
at most one `DragCommit<T>`. Enqueue it as one typed application command, validate
its document revision again, and append one undo record. Cancel on Escape/native
capture loss and call `reconcile` after commits to handle owner removal/visibility.
On Escape, call runtime `cancel_pointer` and deliver its cancellations as well
as clearing preview; do not leave capture behind by merely dropping a session.
Cancel resets preview without document writes. The native host still owns OS
capture; this helper does not inject physical input or replace runtime routing.

`PickTracker` issues replaceable requests carrying scene/camera/viewport revisions
and a unique owner namespace. The caller advances camera and viewport revisions
for projection, geometry or DPI changes, and scene revision whenever snapshot
content changes (including camera changes). Run `SceneSnapshot::pick` on a worker
if useful; it performs double-sided CPU triangle queries with frustum-depth checks.
Accept only the latest result against the current stamp. Cancel picks on drag
start or owner removal. Replacing the tracker revokes its namespace. Picking is
linear in triangle count and is not a raster-coverage guarantee at triangle edges.
`SceneCatalog` separately resolves unique names and allowlisted Select/Frame
commands. These return app-owned commands, not focus changes or direct mutations.

The [brush fixture](../../crates/view-wgpu/examples/support/brush.rs) is deliberately
an application document. It bounds 16×16 RGBA tiles, interpolated preview samples
and undo history; commits validate tile capacity before mutation and retain one
delta per stroke. Cancel never changes tiles. Negative coordinates use Euclidean
tile division. Pressure/tilt are explicitly unavailable. This is a reproducible
proof, not a production painting engine or hardware-input certification.

```text
cargo test -p view-core --test viewport_contracts --locked
cargo test -p view-wgpu --features scene --locked
cargo run -p view-wgpu --features scene --example embedded --locked -- dx12
```

See [M3B evidence](../plan/evidence/m3b-completion.md) for actual counters,
platform coverage and the distinction between scripted scale changes and physical
monitor movement. There are no claimed frame-rate benchmarks.
