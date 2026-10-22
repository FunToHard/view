# Automation protocol 1 / view 0.1

`view-automation` is an optional package over `view-testing::Harness`. Core and
the minimal `view` facade have no serialization, transport, executor or native
input dependency. `view-tooling` supplies the local stdio adapter and an optional
official rmcp adapter. Both dispatch the same `Request` on the runtime owner
thread. The application retains ownership of its document and action reducer.

## Attachment and requests

No process, socket or desktop session starts by default. The sample binary owns
a small document fixture; applications call `view_tooling::serve` with their own
session to expose the same local protocol. A parent process attaches with private
stdin/stdout pipes. No network listener, general eval or shell execution tool is
provided. Session identifiers route requests; they are not authentication secrets.
The launching host grants attachment and scopes. Do not expose these pipes through
an unauthenticated network bridge.

```text
cargo run -p view-tooling --locked -- --enable-session unique-run-id --stdio
cargo run -p view-tooling --features mcp --locked -- --enable-session unique-run-id --mcp
```

One JSON request per line, up to 256 KiB including framing:

```json
{"version":1,"session":"unique-run-id","id":1,"operation":{"op":"discover"}}
```

Responses contain `id`, committed `revision`, and `result` (`Ok` or structured
`Err` with code/message). Stdout contains protocol messages only. MCP negotiates
through rmcp and exposes `view.request`; its arguments are this same envelope.
`discover` returns operation names, actual capabilities, domain command/task
descriptions, scopes, and input/result JSON schemas. An unknown capability is an
error, not an empty successful response.

`resolve` accepts a selector with optional window, role, name, explicit host test
ID and ancestor. Role spelling follows the versioned core role debug names
(`Button`, `TextInput`, etc.). More than one match is an ambiguity error. Resolve
returns an arena/slot/generation string and commit revision. `command` and
`action_point` require that exact target/revision; callers explicitly resolve
again after a stale result. No implicit fallback to a reused slot occurs.

## Commands, workers and synchronization

`Session::register<I>` takes a descriptor and typed deserializable arguments.
Exact schemas reject missing/unknown fields before calling the handler. The
handler reads the model and returns one ordinary action plus a validated result.
The harness dispatches the action on its owner thread. Inspect commands cannot
produce actions. There is no atomic multi-command transaction promise.

Requests are deduplicated within the live session. Reusing an ID with another
payload is rejected. Retention never evicts an ID and then silently executes it
again: full retention requires a new explicit session. A new session after an
ambiguous disconnect does not establish the outcome of a previous mutation.

`register_task<I>` starts bounded background work with owned inputs. The worker
receives `TaskContext` cancellation/progress, never the runtime. `start_task`
returns an operation ID; `poll` observes progress/result, and `cancel` revokes
publication. Host workers must cooperate with cancellation; threads are not
forcibly terminated. Disconnect cancels workers and marks unfinished outcomes
unknown. Completed results are schema-validated on the owner thread.

`wait` evaluates commit, exact presentation-boundary, selector-existence or finite
idle predicates for a bounded number of harness turns. It never sleeps or promises
to finish arbitrary live IO. Clients poll real workers with their own wall-clock
deadline. `advance` changes controlled harness time and requires test scope.
Continuous render producers are not part of the finite idle predicate.

## Geometry, inspection, capture and replay

`snapshot` returns committed tree/layout/semantics/resources and current owner-thread
focus/capture state. Inspection rejects pending runtime work instead of mixing an
old tree with new geometry. `inspector` returns the same snapshot, bounded
invalidation history, and standalone HTML with bounds, accumulated clips and
verified hit-point overlays. HTML labels are escaped and load no scripts or network resources.
Hit overlays sample at most 64 nodes per inspection; remaining nodes carry an
explicit sampling-limit error and can be queried individually with `action_point`.

`action_point` samples a bounded grid in a target's visible window bounds and
validates each candidate using actual transformed/clipped/paint-ordered hit
routing. Thin or irregular targets may conservatively fail. Disabled nodes remain
occluding but cannot receive focus or input; disabled ancestors disable children.
`realize` invokes an explicitly registered host virtual-content/reveal callback.
Its result is labeled semantic setup, never physical scrolling.

Hosts publish presentation and capture receipts explicitly. Supported boundaries
are `offscreen_readback`, `surface_present`, and `compositor_observed`; one is not
evidence of another. Capture records carry window/content revision, dimensions,
RGBA bytes, provenance and measured capture overhead. No provider means unsupported.
Failure bundles include history, snapshots, trace, operation states and capture
metadata. `replay` dispatches recorded requests and requires the caller to supply
recorded asset/worker/network/clock/seed/environment boundaries; omitted fixtures
fail. Diagnostic history can truncate and is not automatically a complete replay.
Generational target IDs remain attachment-local: cross-session replay fixtures must
resolve targets in the destination session and rebuild targeted requests. Replay
does not silently reinterpret stale identities or reproduce arbitrary live input.

Limits include 4,096 retained request/operation entries at most (host chooses fewer),
64 MiB request-result retention, eight active workers, 4,096 snapshot nodes, eight
captures of up to 4 MiB each, 8 KiB per diagnostic history record, and 1,024 turns
per wait. Large history records are explicitly omitted. Normal production hosts
should grant smaller scopes and omit sensitive diagnostics.

## Explicit Windows desktop testing

```text
pwsh -File scripts/verify-desktop.ps1 -SelectedDesktop -Scenario en-US
pwsh -File scripts/verify-desktop.ps1 -SelectedDesktop -Scenario ja-JP
```

These are separate from normal verification. The selected unlocked desktop is
temporarily controlled. F12 stops the driver. It checks foreground window/process,
visibility, existing held modifiers/buttons, current screen origin and DPI. A
cross-process semaphore and process owner flag serialize sessions. A watchdog
checks target/stop/deadline every 20 ms and releases only runner-held inputs on
failure, timeout or drop. Native insertion is isolated in `view-platform`'s
`desktop-input` feature. UIPI/integrity restrictions are reported, not bypassed.

Input insertion is not delivery. The application fixture checks actual routed
targets, document changes, focus, virtual scrolling and editing results. Before
wheel input it requires observed cursor positioning. Unicode insertion, en-US
physical keys and Japanese IME composition are separate paths. The IME scenario
requires an installed and selected Japanese IME; it never installs or activates
an input language. Native candidate geometry comes from shared text layout.

The reference fixture covers editor selection/gizmo/undo, retained search,
asynchronous catalog, wheel-driven virtualization, IDE editing and modal focus.
See [M5 evidence](../plan/evidence/m5-automation.md) for actual results and remaining
IME, accessibility, hardware and platform gaps. An implemented runner is not full
standard-control native conformance certification.
