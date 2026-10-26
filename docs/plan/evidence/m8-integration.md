# Windows implementation integration — 2026-10-25

Follow-up verification on 2026-10-26 corrected missing Windows extended-key flags
in the opt-in input adapter. Home/End and their modified chords now use the E0
variant on both press and release, rather than keypad behavior. The expanded
en-US physical scenario passed twice with normalized key/modifier evidence:
`target/verification/m8-desktop-pass-1.txt` and `m8-desktop-pass-2.txt`. It checks
Home/Shift+End, Ctrl+Home/Ctrl+Shift+End, Delete, Ctrl+Backspace, existing pointer/
wheel/edit/undo/modal workflows and closed-target rejection. Earlier failures are
retained in the matrix and retry logs; those attempts are not passes.

The keyboard trace is capped at 4,096 log entries and enabled only by the explicit
desktop-test feature and scenario. The named layout prerequisite remains intact.
The E0 requirement is described by [Microsoft's input guidance](https://microsoft.github.io/PowerToys/modules/keyboardmanager/keyboardmanager/).

The same follow-up adds [styled document transactions, measured pagination and
bounded disjoint rebasing](f03-rich-documents.md),
[linear-light masked composition and blur](f06-masked-image-effects.md), and the
[hidden native docking restoration proof](controls-expansion.md). Docking stages
real native windows, preserves application document/caret state, revokes old UI
resources and restores physical outer bounds inside the actual monitor work area.
Rich visual editing, conflict-free networking, dock drag UX and HDR remain distinct
expansion work. No new registry dependency was required.

The final text hardening increment adds measured multiline PageUp/PageDown with
preferred horizontal position, native activation independent of retained logical
focus, and demand-driven caret deadlines. Repeated focus synchronization preserves
blink phase and typing-history grouping. Inactive hosts reject late editing and
cancel transient input; no-caret states wait without an expired timer loop.
View Pad/native pointer and geometric-key routes refresh CPU layout for edits
queued before paint. Shared TextField preparation preserves manual scrolling,
reveals on explicit caret activity and clamps offsets on re-layout. Focused
regressions cover these contracts; [native evidence](m5-automation.md) records
physical Page keys and observed Shift cleanup without claiming Japanese IME.

This run implements bounded preview and expansion work requested by the owner.
It does not declare the whole roadmap or developer preview release complete.
Completion status remains in the [canonical checklist](../implementation-checklist.md).

Implementation includes select/table/presentation controls, shared-editor password
entry, docking data/codec/runtime visibility, bounded large-buffer transactions,
sparse SDR document effects/export, OBJ/translation assets and directional
metallic-roughness scene shading. Diagnostics add bounded redacted reports,
host-owned panic composition, WER configuration guidance and supervised recovery.
No software rendering substitute or mandatory core backend dependency was added.

Integration review corrected revealed-password caret mapping, declarative gallery
child ownership, scene upload-budget expectations, docking input allocation limits,
and an add-on test's asynchronous stderr observation race. The text agent also
fixed oversized-edit history barriers, bounded line indexing/batch metadata, and
inverse ranges for adjacent deletion transactions. Tokio's minimum and locked
version are now patched 1.53.3; remaining dependency advisories are documented in
the [security/maintenance review](m8-dependency-review.md).

## Reproducible evidence

| Command | Artifact / scope |
| --- | --- |
| `pwsh -NoProfile -File scripts/verify.ps1` | Final source rerun passed, exit 0: `target/verification/m8-complete-final.log`; all five required workspace gates, optional profiles, proofs, independent consumer |
| `pwsh -NoProfile -File scripts/qualify-release.ps1 -Profiles` | Final source rerun passed, exit 0: `target/verification/m8-packages-complete.log` and `release/qualification.json`; 18 packages, 36 profile groups, zero audit failures, repeated core archive bytes match |
| `pwsh -NoProfile -File scripts/qualify-windows.ps1 -Gpu -NativeWebview -SelectedDesktop` | `target/verification/windows-matrix/cases.json`; named native/GPU cases and explicit uncovered entries |
| `pwsh -NoProfile -File scripts/verify-desktop.ps1 -SelectedDesktop -Scenario en-US` | Final source rerun passed, exit 0: `target/verification/m8-desktop-complete.log`; Page/Shift+Page, owned foreground-loss cancellation, OS-observed Shift release, closed/hidden-target rejection. `windows-matrix/final-desktop-supplement.json` supplements the retained earlier matrix failures. |
| `cargo test -p view-controls --test text_reveal_scheduling --locked` | Final four focused regressions passed in `target/verification/m8-reveal-final.log`, including actual painted password caret geometry; matching all-feature/all-target Clippy passed with warnings denied. |
| `cargo run -p view-application-proofs --locked -- --stress` | Passed: `target/verification/m8-stress-final.log`; 1,000-operation CPU correctness workloads including sparse tiled-image edit/undo |
| `cargo run -p view-controls --features docking --example docking --locked -- --verify-hidden` | Passed: `target/verification/m8-docking-final.log`; actual two-window native restoration with read-only DPI-awareness guard |
| `cargo run -p view-wgpu --features scene --example scene --locked -- dx12` | Passed: `target/verification/m8-scene-final.log`; hardware lighting including an opposing-light ambient-color assertion within 2/255 |
| `pwsh -NoProfile -File scripts/measure-gpu-memory.ps1 -Runs 3 -SkipBuild` | [performance evidence](m8-performance.md); executed hardware DX12 distributions, not an input-to-display latency measurement |
| `cargo check -p view --no-default-features --target wasm32-unknown-unknown --locked` | Passed compile-only capability subset |
| `cargo check -p view-core --target wasm32-wasip2 --locked` | Passed compile-only dependency-free core |

The CPU stress run realized 25 rows out of 100,000 over 1,000 viewport positions;
1,000 three-window edit/undo cycles retained a stable mounted-node count and zero
queued actions. Another 1,000 idle flushes produced zero builds/layouts/commits.
The producer channel held at most four 4 KiB imports and the cache at most eight;
1,000 current/stale search pairs preserved latest-request ownership. Debug timing
under concurrent compilation is exploratory, not a performance regression baseline.
A 4,096×4,096 sparse document completed 1,000 edit/undo pairs with at most one
allocated tile and history entry, returning to zero tiles/history after each undo.

The final release audit passed 36 recorded profile groups across 18 packages with
zero audit failures. Renderer/platform all-feature Clippy and tests also passed
after the final lighting/DPI changes (`m8-render-platform-final.log`); the monitor
precondition test passed without requiring an attached monitor
(`m8-monitor-final.log`). No ordinary test injected mouse or keyboard input.

Final text/input verification receipts are `m8-complete-final.log`,
`m8-packages-complete.log`, `m8-reveal-final.log` and
`m8-desktop-complete.log`. The intermediate `m8-complete.log` retains a password
scroll failure from before the final masked scroll adapter was compiled; it is
not a passing receipt. Final frozen sources passed the full rerun and the stronger
painted-caret regression. `target/verification/m8-source-manifest.json` identifies
the local build/document inputs and receipts; the tree remains uncommitted.

The matrix marks missing Japanese IME, mixed-monitor/text-scale/native AT,
sleep/resume/display removal, and Linux/macOS GPU execution uncovered. Existing
hosted CI results apply to their earlier committed revision, not these local
changes. WER dumps were not enabled or collected. Publication is disabled and the
facade crate name is occupied; no release candidate or external publication is
claimed. Full docking UX, rich visual editing, complete typography/layout,
collaboration transport/CRDT, advanced brushes,
HDR/color management and the other expansion backends remain partial or deferred
as documented in their individual profiles.
