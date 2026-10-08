# Headless runtime API — 0.1.0

This is the implemented M1A surface. It does not open windows, route native input,
compute geometry, publish accessibility semantics or render. `WindowId` names a
logical runtime window; structural snapshots and adapter presentation receipts
are distinct from an observed displayed frame.

Run the [compiled example](../../crates/view-testing/examples/headless.rs):

```text
cargo run -p view-testing --example headless --locked
```

## Ownership and state

`Runtime<M, A>` owns UI state and owned handlers, with neither Send nor Sync.
`flush(&mut model)` borrows the application model only during an owner-thread
turn. Background work uses application-owned transport/execution; a sendable
completion token carries owner/request identity and a cancellation flag.

`Description::new(key, state, handler)` creates a direct-child description. The
handler receives `&mut state`, `&mut model` and the typed action by value, returning
dirty causes. State and handlers are `'static`; borrowed callbacks/state cannot
outlive their source. Distinct component kinds use distinct state newtypes.

`open_window` creates a root. Each node has one structural owner:

| Structure | Child operations |
| --- | --- |
| Retained | `mount` and `unmount` |
| Declarative | `reconcile` |
| Immediate | `set_region_builder`, invoked by `flush` only when needed |

Reconciliation matches parent, key, Rust state type and structure. It retains
compatible local state, replaces the handler and reorders the same identities.
New description state is an initializer, not an update to retained state. Use
`update` for explicit retained changes. Type/structure changes remount; duplicate
sibling keys fail before tree mutation. There is no implicit cross-parent move.
Descriptions specify one level; reused retained mounts preserve their children.

Handles contain an arena namespace, slot and generation. Every arena read/write
validates all three. IDs are process/session-local, not persistent document keys,
selectors or security tokens. The process-unique namespace issuer is an atomic
counter, not a global application model. Exhaustion never wraps; exhausted slots
retire permanently. Unmount removes the subtree iteratively and revokes tasks,
subscriptions and callback registrations. Queued actions to removed generations
are reported as rejected at dispatch. Closing a window also revokes its root.

Hidden, clipped and suspended nodes retain state and registrations. Suspension
also pauses descendant region builds. Resuming restores pending region demand.
`own` registers teardown hooks; `start_request` supersedes an owner/key request.
Hooks run on the owner thread and must be short and nonpanicking. Dropping the
runtime revokes outstanding tokens and runs outstanding cancellation hooks.

## Actions, responses and commits

`enqueue` accepts required typed actions in sequence order. A full queue returns
`Rejected { error, action }` for retry. `enqueue_preview` explicitly replaces a
pending same-owner/key preview, returns the old action and moves the replacement
to the newest sequence position. Required actions are never coalesced.

`complete` validates owner and request incarnation, accepts a result at most once
and leaves a saturated request active for retry. Dispatch revalidates the lease,
so cancellation/supersession after enqueue still prevents mutation. Successful
consumption revokes the token without calling its cancellation hook.

`flush` drains accepted input, executes handlers once, builds dirty regions and
publishes one coherent structural snapshot. It reports stale queued actions and
whether region response backpressure prevented draining the queue. Region
responses go to the nearest registered ancestor region, in input sequence order.
Each sequence is consumed before the builder runs. A failed description cannot
repeat an already executed action or redeliver its response on the next attempt.

Response buffers use the runtime queue capacity. A suspended region with a full
buffer can block ordered dispatch; the host must resume/build that region before
retrying the pending queue. Backpressure is explicit, not a promise that every
flush drains all pending work. Applications should not spin on a blocked receipt.

Builders receive immutable model access; measurement receives only the last
committed snapshot. Neither boundary receives a mutable runtime or dispatcher.
Rust cannot prohibit effects hidden in captured interior-mutable/external state;
applications must keep these callbacks pure. Effects belong in action handlers.
No panic catching, model rollback or transaction rollback is promised. A rejected
description leaves the old published snapshot visible; effects already dispatched
stay applied. Fix/retry the description without retrying accepted actions.

## Demand and observation

`invalidate` records build/layout/paint/composite/semantics/viewport causes. BUILD
schedules the nearest registered region; applications explicitly invalidate other
model-dependent regions. Layout currently propagates across the entire window to
cover ancestor and sibling dependencies. Composite changes also dirty semantics.
This conservative policy precedes actual layout/paint/semantic engines in M1B/M3.

`demand(window)` coalesces UI and viewport requests. `has_work` covers structural
changes/input/regions, including the closing of the last window. A viewport
request alone does not rebuild or commit UI; `take_viewport_request` consumes it.
There is no built-in loop, timer thread or automatic polling. An idle host can
sleep until external work arrives. `Flush::backpressure` requires host handling.

`snapshot` exposes the last committed tree, not partially mutated live nodes.
`state` inspects live typed component state and may differ from that snapshot
before commit. Snapshot contents include ownership, ordering, visibility and
resource categories; application state and closure contents are not serialized.
`acknowledge_presentation` rejects future/regressing revisions, but the adapter
must define what observation its acknowledgement actually represents.

Counters record actions, region builds, commits, idle polls and lifecycle events.
`observe(capacity)` enables a bounded action/invalidation/lifecycle trace; zero
disables collection. No global hook or external transport is installed. Counters
saturate and are diagnostic only; identity/revision arithmetic remains checked.

## Deterministic tests and current limits

`view-testing::Harness` owns a runtime and fixture model. `advance` changes virtual
time only; `schedule` queues bounded typed input/completions with stable equal-time
FIFO ordering; `step` delivers due inputs and flushes once. Full runtime queues
retain the due input for retry. Clock overflow fails without changing time.
`ServiceFixture<T>` provides bounded FIFO values (including Result values) instead
of live IO. Replay must explicitly supply external outcomes; input alone cannot
reproduce arbitrary real worker/network behavior.

See the [contract tests](../../crates/view-core/tests/runtime_contracts.rs),
[driver tests](../../crates/view-testing/tests/driver.rs), and
[independent consumer](../../tests/compatibility/facade-consumer/tests/harness.rs).
They use expected values/order, a recorded fixed seed for arena stress and actual
public APIs. Keyed editing state is covered; focus/IME/capture, geometry and native
input are M1B or later. No performance budget is claimed: keyed matching is linear
per child and layout invalidation is window-wide. The
[evidence report](../plan/evidence/m1a-runtime.md) records actual platform coverage.
