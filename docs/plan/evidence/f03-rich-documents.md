# F-03 bounded rich document, pagination and collaboration profiles

This follow-up implements experimental `rich-document/1`,
`measured-pagination/1` and `disjoint-rebase/1` in `view-text`, complementing
the [existing large-buffer service](f03-large-document.md). Contracts, limits,
application ownership, history invariants and unsupported behavior are recorded
in the [rich document guide](../../guides/rich-documents.md). No new dependency,
platform branch or unsafe code is introduced.

The reason for a separate data model is to add structured formatting and pure
pagination without mixing application document identity with the UI tree or
expanding `PlainEditor` into an uncertified rich control. Caller-supplied measured
lines preserve the established shaping/backend boundary. Bounded change metadata
supports explicit conservative stale-edit handling without claiming a CRDT.

Public API behavior fixtures:

- `crates/view-text/tests/rich_document.rs`: formatting/run splitting and
  coalescing, structure edits, complete undo/redo round trips, sequential atomic
  transactions, grapheme and cross-run cluster rejection, normalized empty runs,
  metric validation, data/operation/history limits, no-op revisions, stale
  requests, and redo preservation after rejection.
- The same suite checks remote replacement rebasing over sequential insert/delete
  and style changes, independent paragraphs, adjacent replacements, overlapping
  replacements, same-position and endpoint insertion conflicts, structural and
  history barriers, expired rebase history, invalid remote boundaries and future
  revisions, plus remote edits sharing application undo authority. A regional-
  indicator regression checks that an invalid base grapheme boundary cannot
  become valid merely by rebasing. Aggregate preflight and rebase-byte eviction
  fixtures cover the source history's separate accounting budget.
- `crates/view-text/tests/pagination.rs`: complete measured coverage, exact page
  boundaries, paragraph fragments, spacing, keep-together and explicit breaks,
  empty paragraphs, Unicode/RTL logical coverage, stale style measurements,
  invalid metrics, unbreakable content and page/line limits.

## Local verification

Executed on Windows x64 on 2026-10-26:

- `cargo fmt --all -- --check`: passed.
- `cargo test -p view-text --locked --test rich_document --test pagination`:
  passed, 25 behavior tests (18 rich-document, seven pagination).
- `cargo clippy -p view-text --all-targets --all-features --locked -- -D warnings`:
  passed.
- `cargo test -p view-text --all-features --locked`: passed, 58 integration
  behavior tests, including the existing plain-editor, large-document and
  owned-font shaping suites. Zero unit/doc tests are not behavior evidence.
- `cargo doc -p view-text --all-features --no-deps --locked`: passed.
- Local relative links and fenced-block balance checked in the new guide/report.

The filesystem emitted incremental-cache hard-link warnings and copied cache
files instead. These commands exited successfully. Workspace-wide and independent
consumer qualification is recorded by the integrating change's main evidence;
the package results here do not imply GPU/native or other-platform coverage.

## Coverage limits

This is a bounded data profile with CPU behavior evidence. No rich control,
native selection/IME/accessibility, GPU rich glyph rendering, printing, file
format interchange, Linux/macOS execution, large-document latency or collaboration
transport/convergence certification is claimed. F-03 remains subject to the
canonical checklist's broader workload and product-profile acceptance; this file
is evidence, not a second progress board.
