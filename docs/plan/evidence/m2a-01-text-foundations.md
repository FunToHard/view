# M2A-01 text foundations — 2026-10-08

Introduced unpublished `view-text` with backend-independent text snapshots,
owner-local value revisions, validated replacement requests, directional selection,
transient composition, layout measurement/hit/caret interfaces and editor-session
interfaces. The facade exposes these through opt-in `view::text`. No registry
dependency was added; both lockfiles include the local package.

UTF-8 byte offsets require scalar-boundary validation against their snapshot.
Conversions reject UTF-16 surrogate interiors and out-of-range offsets. These
foundations deliberately do not equate scalar boundaries with grapheme carets.
Layout inputs specify font/DPI invalidation, and results retain their source and
CPU resources independently of future renderer atlas allocations.

Four public contract tests cover independently specified mixed Unicode offsets,
reversed selection, stale/malformed edits and invalid metrics. A separate workspace
consumer exercises the optional facade text feature. `pwsh -File scripts/verify.ps1`
passed locally on Windows x64 with Rust 1.98.1: formatting, locked all-target
workspace check, strict Clippy, workspace tests, rustdoc, isolated package tests,
facade feature checks, independent default/harness/text consumer profiles, fixture
hashes and dependency inventory. Existing Windows hidden native smoke also passed.
The new text tests exercised no native services, GPU or physical input. Linux/macOS
execution for this new slice has not yet run. Local logs are retained under
`target/verification/m2a-01-local.log`.

M2A-02 onward remains open: no font discovery/shaping implementation, grapheme or
bidi caret policy, editing/history engine, clipboard/IME integration, renderer or
standard-control conformance is claimed. M0 coverage remains unchanged.
