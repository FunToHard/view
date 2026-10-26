# Bounded rich document services

`view-text` (or `view::text` with the `text` feature) provides the experimental
`rich-document/1`, `measured-pagination/1` and `disjoint-rebase/1` data profiles.
They extend the [plain text services](text-editing.md) without changing
`PlainEditor` or `LargeDocument`. The application owns its `RichDocument` and
document command/undo authority independently of runtime node identity. There are
no new dependencies, platform APIs, unsafe blocks, background work or networking.

```rust
use view_text::{RichDocument, RichDocumentConfig, RichOperation, RichParagraph,
    RichStyle};

let mut document = RichDocument::new(
    vec![RichParagraph::plain("Title"), RichParagraph::plain("Body")],
    RichDocumentConfig::default(),
).unwrap();
let base = document.revision();
document.apply(base, vec![RichOperation::Style {
    paragraph: 0,
    range: 0..5,
    style: RichStyle { bold: true, ..RichStyle::default() },
}]).unwrap();
assert!(document.paragraphs()[0].runs[0].style.bold);
document.undo(document.revision()).unwrap();
assert!(!document.paragraphs()[0].runs[0].style.bold);
```

## Rich document profile

The document contains at least one paragraph, each with styled UTF-8 runs and
paragraph formatting. Inline fields are bold, italic, underline, logical font
size in thousandths (1 through 1,000,000), and RGBA color. Paragraph fields are
finite nonnegative before/after spacing, keep-together and explicit page-break.
Paragraphs prohibit embedded CR/LF; paragraph insertion/removal expresses hard
breaks. Empty paragraphs have no runs. Adjacent equal styles coalesce.

`RichOperation` supports replacement with explicitly styled runs, applying an
inline style, setting paragraph format, and inserting/removing paragraphs. A
transaction executes operations sequentially; later coordinates refer to earlier
results in that transaction. The base revision belongs to this document owner.
An empty or value-identical transaction preserves revision/history. A real edit
advances revision once, including formatting-only edits. Invalid/stale/over-limit
transactions leave value, formatting, revision, undo, redo and rebase history
unchanged. Every intermediate document also satisfies the configured bounds.

Offsets and ranges use paragraph-local UTF-8 bytes at extended grapheme
boundaries. Style boundaries also respect whole paragraph graphemes, including
clusters assembled across submitted run boundaries. Inserting combining text can
merge with an adjacent cluster when its resulting style is compatible; a style
split inside the resulting cluster rejects atomically. This is a logical data
policy. It does not change the existing shaping backend's bidi/visual caret
policies or certify rich-text IME/selection behavior.

Undo/redo restores whole transactions, including styles and paragraph structure,
while advancing revision. Count and byte budgets cover both undo and redo.
Each history entry retains before/after snapshots; byte accounting includes UTF-8
payload and paragraph/run record sizes, excluding allocator spare capacity and
entry/container overhead. Metadata is bounded by the history entry count. A
transaction too large to retain clears older undo/redo, creating a history barrier.
A new local or remote edit clears redo. Remote edits use the same application
history: undo is transaction-order undo, with no per-author selective undo.

Default limits are 4 MiB UTF-8, 100,000 paragraphs, 100,000 normalized runs,
1,024 operations per transaction, 100 history entries, 16 MiB retained history,
and 128 rebase transitions under a separate 16 MiB rebase byte budget. Submitted
run lists also obey aggregate byte/run limits before normalization allocations.
Configuration is explicit and can be reduced. Inputs are
caller-owned; working candidates/snapshots can temporarily coexist. This is not
a process RSS bound. Edits clone and validate the document; there is no rope,
incremental layout or interactive performance claim.

## Measured pagination profile

The host supplies `DocumentMeasurements` for the current document revision:
one nonempty list of `MeasuredDocumentLine` per paragraph, with contiguous,
complete logical source ranges and positive finite logical line-box heights.
An empty paragraph requires exactly one empty-range line. Line ends must be
grapheme boundaries; gaps, overlap, truncated coverage and stale revisions reject.
The host qualifies wrapping width, styles, fonts, font revision and DPI before
submitting measurements. The paginator cannot verify that a caller measured the
right font or used an up-to-date backend configuration.

`RichDocument::paginate` is pure and returns revision-tagged pages containing
paragraph/line ranges, y coordinates and fragment heights. Top/bottom margins
reserve usable height. A line ending exactly at the bottom fits; splits occur
between measured lines. Keep-together paragraphs move intact to the next page.
Explicit page breaks do not create an initial empty page. Space-before applies
once to the first fragment; space-after is clamped/discarded at the page edge.
A line, first-line-plus-space-before, or keep-together paragraph taller than
usable height returns `UnbreakableContent`. The default output/input processing
caps are 100,000 lines and 10,000 pages; there is no partial result on failure.

Line order remains the host's logical source order, including mixed RTL/LTR text.
This service does not reshape text, implement rich glyph painting, widow/orphan
control, keep-with-next, headers/footers, footnotes, floating content, tables,
print/export, page width negotiation or page-level visual caret navigation.

## Conservative collaboration profile

`apply_remote` accepts one `RichRemoteEdit`: a replacement at a base owner-local
revision. It walks retained changes and shifts the replacement across disjoint
earlier text edits. Disjoint inline styling is retained; formatting of another
paragraph is independent. Adjacent nonempty replacements can rebase. Overlap,
same-offset insertions, and insertion points touching another replacement's
endpoints return `Conflict` to avoid inventing insertion affinity or ordering.
Any intervening paragraph structure edit or undo/redo is a conflict barrier;
same-paragraph format changes also conflict. This deliberately conservative
policy can reject changes a full collaborative editor might merge.

Rebase history retains at most 128 revision transitions by default under a
separate 16 MiB aggregate accounting budget. Each transition retains the base
paragraph UTF-8 strings plus at most the configured operation count in
range/length records. Submitted remote ranges are validated against these base
strings before transforming; validating only the resulting current range would
be incorrect because regional-indicator grapheme pairing can change after an
earlier insertion. Accounting includes UTF-8 payload, String records, Change
records and touch records, excluding allocator spare capacity. If a transition
cannot fit the budget, older history is cleared to preserve a complete chain.
If the complete base-to-current chain has expired, the error is
`HistoryUnavailable`; the application must resynchronize/review. A future base
revision rejects as stale. After rebasing, the ordinary atomic local edit path
revalidates graphemes, limits and styles and creates one history transaction.

The host must route messages to the correct document, authenticate authors, order
delivery, deduplicate operations, persist data and obtain user resolutions. There
are no operation IDs, wire protocol, causal graph, CRDT, general OT convergence
guarantee, offline merge, collaborative cursor mapping or selective undo. Revision
values do not identify document owners. Retrying a remote edit is not idempotent.

Acceptance evidence and reproducible checks:
[F-03 rich document evidence](../plan/evidence/f03-rich-documents.md).
