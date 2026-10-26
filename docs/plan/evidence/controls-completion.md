# Select and collection implementation

UI-03/UI-04 add shared popup-owned select behavior, menu/dialog/list/tree
presentations and the read-only `bounded-table/1` profile. No new crate or
dependency was required. The gallery now hosts dropdown and table controls,
including keyed accessible option/row/cell children, keyboard actions and GPU
presentation. Declarative child ownership is retained through reconciliation.

`collections_completion.rs` checks pending navigation versus committed selection,
Escape cancellation, external dismissal, disabled/stale keys, typeahead, keyed
sorting, two-axis table virtualization and visible-cell budgets. It also checks
menu reverse entry, typed dialog commands, modified key release cleanup and tree
labels/indentation/leaf state. Existing accessibility, contracts and gallery
scenarios remain applicable. `password_docking.rs` separately exercises the
bounded expansion profiles, including Unicode graphemes, reveal caret mapping,
clipboard restrictions, transactional moves, codec rejection and focus/capture
revocation.

CPU tests passed during the workspace integration run. Native and GPU execution
is recorded by `scripts/qualify-windows.ps1` in
`target/verification/windows-matrix/cases.json`; an earlier gallery run caught
and led to fixing an imperative mount under a declarative select owner.
Only the final rerun determines the GPU status. No native screen-reader or
Japanese IME certification is inferred from semantic translation or GPU pixels.

The 2026-10-26 text increment adds measured Page navigation, idempotent focus
synchronization and explicit caret-reveal requests. `text_page_navigation.rs`
covers hard/wrapped lines, preferred x, selection/clamping, resize, tiny viewports
and Unicode/bidi policy. `text_reveal_scheduling.rs` preserves focused manual scroll
across redraws, reveals after editing/navigation, clamps on resize and checks masked
versus revealed password coordinates. View Pad/gallery activation and idle-deadline
tests, plus native queued-edit geometry tests, cover host wiring separately from
shared editing. Selected desktop Page and lifecycle evidence is recorded in
[M5 automation](m5-automation.md).
