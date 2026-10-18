# Canvas2D snapshots

`view::canvas` (or `view_core::canvas`) supplies backend-independent document/view
mapping and bounded ordered records. It introduces no package or dependency.
The app keeps its document, undo history and stable `CanvasId` values; the runtime
owns the canvas node, focus, capture and action execution as before.

`CanvasPoint` and `CanvasRect` use f64 document units, positive X right and Y down.
The older f32 `DocumentPoint` API remains compatible. `CanvasView` maps document
positions to viewport-local logical units by subtracting its document origin in
f64, multiplying by positive zoom, then narrowing to f32. `project` rejects
overflow or rounding error greater than 1/256 logical unit per axis. This is a
conversion bound, not unlimited document precision: f64 itself eventually loses
small increments. Clip or tile distant drawing data before conversion. DPI scale
and window placement are separate downstream transforms; do not apply DPI twice.

`pan` moves visible content by a logical delta. `zoom_at` preserves the document
point under a logical anchor, subject to f64 arithmetic. Resize preserves origin
and zoom. Zero extent disables hits and visible semantic regions. Invalid camera
changes fail atomically. Use `set_view` to publish a camera change without
rebuilding records; an unchanged view does not advance the canvas revision.

`Canvas2D<D>` owns back-to-front `CanvasRecord<D>` descriptions. The generic draw
payload belongs to the application/renderer and is not interpreted by core.
`update(Some(records))` atomically replaces the snapshot after validating finite
ordered bounds, unique IDs and the caller's record-count budget. `update(None)`
skips an on-demand build, retaining all records and the revision; an empty vector
explicitly clears it. The record budget does not bound allocations inside an
application payload or semantic string. This API never runs draw callbacks or
application effects. Do not mutate interior-mutable payloads behind a published
snapshot if coherent observations are required.

Route input through the owning runtime node, subtract window placement, and call
`hit_test` with viewport-local logical coordinates. It clips to the viewport and
chooses the last painted enabled half-open rectangle. `document_at` also accepts
outside points so a captured drag can continue beyond the viewport. Rectangular
regions are the current profile; arbitrary path hit testing is not implemented.

`semantic_regions` clips the same rectangles with the same view and includes the
snapshot revision. Semantic-only records can remain discoverable without catching
pointer hits. The adapter maps app IDs to runtime-owned semantic nodes, adds
window placement, validates the revision before dispatch, and queues supported
actions through the normal runtime. These records do not establish a second
focus, lifecycle or accessibility tree. Drawing consumes `records()` using the
same camera and viewport clip; the API does not submit GPU work.

Run the CPU example and contracts:

```text
cargo run -p view-core --example canvas --locked
cargo test -p view-core --test canvas_contracts --locked
cargo test --manifest-path tests/compatibility/facade-consumer/Cargo.toml --test canvas --locked --target-dir target/consumer
```

This API began in M3B-01. The [embedding proof](embedding.md) now exercises a
bounded app-owned tiled brush document, rendered preview and one-stroke undo.
See [M3B evidence](../plan/evidence/m3b-completion.md) for executed layers and
native-input certification limits.
