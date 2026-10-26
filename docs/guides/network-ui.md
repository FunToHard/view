# Network-driven UI — view 0.1

Enable `view/resources` for transport-independent resources. The framework does
not select an HTTP client, executor, credentials store or storefront backend.
Applications own IO and documents; the UI receives validated, revisioned results.
The [authoring reference](authoring.md) describes the compiled resource, mailbox,
cache and remote-section APIs and the [runtime guide](headless-runtime.md)
describes owner cancellation.

The executable starting points are:

```text
cargo run -p view --features resources --example async_branch --locked
cargo run -p view-application-proofs --locked
cargo test -p view-resource --all-features --locked
```

The first example starts retrieval outside build/measure. The application proofs
use a bounded deterministic fixture rather than a commerce service. They cover
editor/storefront/IDE workflows, reordered completions and owned state through
the public interfaces; [M4 evidence](../plan/evidence/m4-authoring-proofs.md)
separates native/GPU observations from CPU execution.

Create a runtime-owned subscription lease, then bind a `Resource` and each request
to its cancellation token and owner generation. Start effects from event handling
or an application scheduler. On completion validate request/key/revision ownership,
publish once on the UI thread and notify subscribers. Cancellation prevents late
results from acting on recycled nodes. Request creation in build/measure would
replay effects and violates the runtime contract.

Use the declared retention policy during same-query refresh. Keep the existing
content mounted to retain selection/focus/scroll. A different query invalidates
the old value; a successful empty result is a ready state. Resolve focus to an
application-selected live node when replacing a branch. Retry and reconnect
return effects; the application owns clocks, scheduling, deduplication and the
idempotency policy for remote mutations.

Required stream events use bounded FIFO/backpressure; replaceable preview updates
may coalesce. Account bytes as well as event counts. Detect sequence gaps and
resynchronize authoritatively. Bound input bytes before decoding remote sections,
then enforce schema version, depth, items, key/action/asset/text lengths and stable
sibling keys. Remote configuration selects compiled, locally authorized controls
and actions. Unknown optional controls become inert fallbacks; unsupported
mandatory controls fail. A response cannot grant privileges or execute code.

Fetching, decode and GPU upload have separate byte/queue budgets. Markdown/image
references do not fetch automatically. Optional `image-bmp` supports the documented
uncompressed BMP profile only. Preserve the last known good data only under an
explicit retention policy and expose failures/retry state. Secrets and user data
do not belong in diagnostic bundles or repository fixture recordings.
