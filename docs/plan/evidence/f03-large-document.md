# F-03 large-document service evidence

This increment implements the plain-text editor-service slice of F-03 in
`view-text`: bounded document size/line count, indexed line access, revision-
checked literal search with bounded results, atomic base-coordinate edit
batches, and delta-based undo/redo. Unicode edits require valid UTF-8 boundaries.
Batch ranges cannot overlap. Each committed batch/history operation increments
the document revision once; stale or invalid operations leave text and revision
unchanged. It adds no dependency.

Behavior coverage is in `crates/view-text/tests/large_document.rs`, including
line indexing, Unicode byte boundaries, overlap/staleness rejection, search
truncation, atomic limit failures, and undo/redo revision progression. Index
construction stops at the configured line limit, checks the byte limit before
index allocation, and each transaction rebuilds the line index once. Batch edit
count has its own cap. Undo history bytes count retained inserted/deleted UTF-8
payload; metadata is bounded by the maximum history transaction count multiplied
by the maximum batch edit count. An edit too large to retain creates a history
barrier and clears earlier undo/redo ranges. Adjacent base-coordinate edits are
coalesced in their inverse where output ranges collapse; same-offset duplicate
edits reject to keep insertion ordering unambiguous.

## Local verification

- `cargo test -p view-text --locked`: passed on Windows after implementation.
- `cargo fmt --all -- --check`: currently reports formatting differences in
  concurrent repository edits, including files outside `view-text`; the new
  `view-text` sources and test were formatted directly with rustfmt.
- No Linux/macOS execution, native UI, GPU, latency benchmark, or 64 MiB stress
  measurement is claimed.

This does not complete all of F-03. The follow-up
[bounded rich document profiles](f03-rich-documents.md) add structured
transactions, measured-line pagination and conservative disjoint edit rebasing
as separate application data services. A rich visual editor, full document
layout/PDF and conflict-free network collaboration remain distinct work. The
current flat `String` model clones the buffer and rebuilds all line starts for
each edit; a rope/piece-tree and workload evidence are required before claiming
large-buffer interactive performance.
