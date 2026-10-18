# M2B controls and accessibility evidence

Scope: checklist section 5, 2026-10-09. This is preliminary implementation evidence,
not physical native-control certification. M2B-11 remains open: no dedicated
desktop session was explicitly selected for actual IME candidate-window input.

## Control / profile matrix

| Control / contract | Implementation and automated evidence | Native limitations |
| --- | --- | --- |
| Label/image/button | Shared text preparation, explicit image resolver; press/release, repeat, blur/disable cancellation and semantic capabilities; renderer image lifetime tests | Physical mouse/keyboard and screen-reader activation uncovered |
| Toggle/checkbox/radio | Explicit Mixed cycle; required/exclusive keyed groups, enabled-member arrows; disabled actions reject | Native announcement/roving focus not certified |
| Theme/style | Typed inherited/local overrides, revisioned reload, resolved paint-versus-layout invalidation test | OS theme/high-contrast integration not advertised |
| TextInput/custom field | Identical mandatory shared profile execution, shaped selection/preedit/caret, native command dispatch; readonly+copy, external echo/replacement and unmount tests | Actual Japanese IME and screen-reader editing uncovered |
| Plain TextArea | Same profile plus newline/Tab policy, wrap and navigation from shared editor | Native multiline range geometry/navigation uncovered |
| Numeric/slider | Invalid intermediate text, precision/bounds, commit/cancel, read-only rejection, keyboard steps and drag cancellation | Native value announcements uncovered |
| Scrollbar/view | Thumb dragging and page steps, clamp/cancel; core scroll/reveal/chaining reused | Physical wheel/trackpad coverage uncovered |
| Virtual list/tree | 10,000-key bounded realization, End/reveal, keyed reorder, removal, collapse/ancestor focus; gallery publishes keyed realized semantic children | Full virtual accessibility realization actions outside current rows not advertised |
| Split/popup/dialog/menu/palette | Minimum constraints and rollback; edge placement, modal/nesting checks, live focus return, disabled command filtering and keyboard menu navigation | Native modal/menu physical interaction not certified; popup host must apply runtime modal scope |
| Accessibility | AccessKit full updates, generational mapping, inherited hidden/disabled policy, text grapheme ranges, revision-tagged action validation, atomic invalid-update rejection | Hidden adapter installation/update is not a screen-reader or UIA-client test; Linux AT-SPI disabled |
| Desktop services | Existing owner-scoped Windows Unicode clipboard; incoming file drops; Win32 owner-modal open/save/folder dialogs, context menus and explicit shell activation | Dialog/shell effects are compiled, not automatically opened; Linux/macOS services return unsupported; outgoing/non-file DnD excluded |

Behavior tests: `crates/view-controls/tests/contracts.rs` and
`crates/view-controls/tests/accessibility.rs`. Tests use independent expectations
for values, selected keys, bounded ranges, focus return and revocation; the gallery
uses the same persistent controls in the core runtime. Runtime capture now has an
explicit owner-validated API; hide/modal exclusion/unmount revoke it, and capture
reset delivers one cancellation per owner.

## Executed and pending verification

Windows local control tests and shared profiles pass. DX12 gallery GPU readback at
100%, 150%, and 200% runs on NVIDIA GeForce RTX 3050 6GB Laptop GPU, driver
32.0.16.1714. The first 100% image was visually inspected: standard/custom fields,
selection/preedit, ranges, radio/checkbox and list/tree are visible and clipped.
Hidden native gallery surface and AccessKit installation/publication pass without
showing or activating the window. Native candidate UI, assistive-technology client
interaction and physical input are **uncovered**, not passed by these fixtures.

The standard verification script includes all required workspace checks, controls
default/all-feature profiles, gallery CPU preparation and independent facade
consumers (`controls`, `accessibility`, `controls-render`). `-Gpu -Backend dx12`
adds renderer and gallery hardware/readback/hidden-surface fixtures. Final local/hosted outcomes are also reported with the delivered commit; hosted CPU jobs cannot
close GPU or physical-input gaps.

The generated shared-editor reports intentionally keep certification false:
native-clipboard/native-ime/native-accessibility and unexecuted rendered/profile
cases remain uncovered. The separate renderer/control fixtures add evidence to
this matrix, without rewriting unrelated profile cases to Passed. A distinct
green, padded, thick-border field runs the identical editor profile as the
standard field; decoration owns no alternate editing state machine.

## Dependency and boundary review

`view-controls` adds only existing first-party core/text dependencies by default;
text shaping enables the already-qualified cosmic-text stack. Rendering and
AccessKit translation are optional. The gallery's winit/platform features are
explicit and not enabled by normal facade consumers. No new registry version is
selected in the root lockfile: accesskit 0.25.1, accesskit_winit 0.34.1 and windows
0.62.2 reuse the qualification family already recorded in the repository.
These maintained upstream adapters are not represented as bug-free. accesskit_winit is Apache-2.0; AccessKit core is
MIT OR Apache-2.0, windows bindings MIT OR Apache-2.0; target native adapters and
their transitive graph remain visible in the dependency inventory. Windows Shell
bindings add selected generated APIs, not a browser/webview or command runtime.
Core and facade remain unsafe-free; COM/Win32 operations have adjacent lifetime,
thread and memory-release safety explanations in view-platform.


Continuation verification, 2026-10-18: the fresh Windows workspace formatting,
all-target compilation, Clippy, workspace tests and documentation builds passed.
The gallery regression for pointer activation and capture revocation passed.
This regression is now part of the standard verification script. Extended profile,
GPU and hosted CI outcomes are verified during delivery; their environment and
profile reports remain under target/verification and in CI artifacts.
