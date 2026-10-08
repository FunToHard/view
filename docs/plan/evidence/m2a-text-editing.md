# M2A shared text implementation — 2026-10-08

## Implementation

M2A-02 through M2A-14 are implemented in `view-text` and the optional native text
adapter. [The editing guide](../../guides/text-editing.md) specifies Unicode,
history, revision, composition, line, mode, timing and cache policies.

| Items | Acceptance implementation |
| --- | --- |
| 02–03 | cosmic-text, system/owned fonts, fallback, bounded LRU, retained fonts, grapheme/word segmentation, logical/visual bidi navigation and hit/selection geometry |
| 04–06 | Versioned profiles, replacement/deletion, grapheme/word commands, directed selection, logical/visual/vertical/pointer movement |
| 07–09 | Bounded grouped history, controlled revisions/validation, modes, placeholder, explicit-clock blink, caret reveal |
| 10–12 | Clipboard fixtures/native Windows adapter, IME composition and candidate adapter, multiline wrapping/navigation and newline/Tab policies |
| 13–14 | Public profile adapters/reports, independent expected edits, generated valid-selection/undo properties, explicit uncovered native/rendered cases |

The reusable runner executes nine common headless cases and multiline newline/Tab
cases. Limits/validation and blink/placeholder have direct tests; generic adapters
do not inherit that coverage. Native/rendered cases remain uncovered in adapter
reports. These engine/backend tests do not certify a standard text control.

## Dependencies and fixtures

Core and the default facade stay backend-free. `view/text` adds unicode-segmentation
1.13.3 (MIT OR Apache-2.0). Optional shaping reuses cosmic-text 0.19.0 (MIT OR
Apache-2.0), already qualified with glyphon 0.12.0 and wgpu 30.0.1 in M0. Its
fontdb/harfrust/swash/bidi graph is isolated behind the shaping feature and checked
with an independent consumer. This reuses the qualified maintained family rather
than selecting a new version; no zero-bug claim is made. No new registry package
was added to the root graph.

Windows clipboard adds binding features to windows 0.62.2. A separate fixture
feature adds station/desktop bindings for isolation. Linux/macOS expose IME
dispatch but no native clipboard implementation. First-party packages remain
unpublished, edition 2024/Rust 1.98.1. Backend types stay out of core. Three Noto
fonts and their OFL 1.1 license are retained with hashes/archive provenance in the
[fixture manifest](../../../tests/fixtures/manifest.json).

## Verification

Targeted editor, Unicode offset, owned-font shaping and normalized IME tests pass.
Generated properties run 32 deterministic seeds with 100 steps, checking valid
selections and undo/redo text/selection round trips. Explicit fixtures cover mixed
scripts, ligature/emoji caret validity, empty lines, wrapping, bidi movement,
cache invalidation and retained layout lifetime. Missing emoji glyphs are not
reported as rendered support.

Native Windows UTF-16 clipboard round trips (including empty/replacement and NUL
rejection) passed against the system-assigned noninteractive logon station outside
the sandbox. The adapter implementation is unchanged from that passing run.
The fixture was then hardened to require `CWF_CREATE_ONLY`, preventing reuse of
an existing station. That final fixture refuses this session because its generated
station already exists; a fresh-isolation replay is still uncovered. Explicitly
named stations also require administrator membership, per the
[Windows API contract](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-createwindowstationw).
No interactive desktop switch or physical input occurred. Native clipboard
execution is opt-in with `scripts/verify.ps1 -NativeClipboard`; ordinary CPU
verification never accesses the clipboard.

`pwsh -File scripts/verify.ps1` passed locally on Windows x64, Rust 1.98.1:
formatting, locked all-target workspace check, strict Clippy, workspace tests,
rustdoc, text/platform all-feature lint/tests/docs, independent package and
default/harness/text/shaping consumers, fixtures and dependency inventory.
[Actual local environment](m2a-local-environment.json) records the executed host
and explicit native-clipboard/GPU/input flags.
The combined consumer also passed five tests, including virtual harness-clock
blink without idle runtime work. The text crate ran 21 tests with shaping enabled
(four index contracts, twelve editor/property/profile tests and five shaping
tests); the platform IME adapter added one test. Default workspace runs omit the
optional shaping/IME tests, which the script executes separately.

Host-driven scroll offsets clamp to content extents. Invalid pointer viewport
geometry rejects before selection mutation. Candidate reporting snaps scalar
preedit cursor offsets to display grapheme boundaries by affinity. These final
boundary cases have explicit shaping regression tests. Raw local output is in
`target/verification/m2a-final.log` (ignored generated evidence).

## Three-platform CI and merge

[CPU qualification run 37773960528](https://github.com/FunToHard/view/actions/runs/37773960528)
passed on Windows 2025, Ubuntu 24.04 and macOS 15 for
`a927a9e4522b74267f7a73d7cac20a967a64c344`, subsequently fast-forwarded to `main`.
All three jobs executed the full CPU script, including optional shaping/platform
text suites and independent consumers. Windows also ran hidden native shell and
callback-error checks. The opt-in fresh-station clipboard fixture was not requested.
[Actual CI environments](m2a-cpu-environments.json) preserve compiler/OS/architecture
and execution flags. This supersedes the earlier local-only platform coverage.

## Remaining acceptance

M0 runner/session gates remain unchanged. No rendered glyph output, native Japanese
IME keyboard/candidate-position sequence, physical drag-selection, native
accessibility or full control lifecycle/shortcut conformance is claimed. These
need M3/M5 and control integration. Linux/macOS CPU execution passed as recorded
above; native-window/clipboard/IME and GPU parity are not implied.
