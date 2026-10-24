# Standard controls (preview)

Enable `view/controls` for backend-independent controls, `controls-render` for
wgpu decoration, and `accessibility` for semantic translation. None is a default
facade dependency. `view-platform/accessibility` owns native AccessKit adapters;
`text` enables native editor/clipboard services and `desktop-services` enables
Windows dialogs, context menus and shell activation.

Store `Button`, `TextInput`, `TextArea`, `NumericField`, `Slider`, selection and
popup state in a `Runtime` node. Their identity is that node's generational
`NodeId`. Keep application documents and effects in the application model.
Call behavior methods once from routed input, then invalidate paint/semantics
(and layout when size changes). Standard and custom field decorations share
`TextField`/`PlainEditor`; replacing decoration must not replace the editor session.

## Input and lifecycle

- Buttons activate on first Enter down, Space up, or primary release inside.
  Repeat Enter, blur, disable and cancellation do not activate. The host uses
  `Runtime::capture_pointer` after a press and `cancel_pointer` on native capture
  loss. Hiding, modal exclusion and unmount revoke logical capture. Native capture
  is the window adapter's responsibility.
- Checkboxes cycle Off/On, optionally Mixed. Radio groups have stable keys, an
  explicit required-selection policy, and arrows wrap over enabled members.
  The runtime owns group focus; `set_focused` distinguishes active focus from the
  group's remembered member. Tab enters/leaves the group.
- Text fields expose commands, printable text and IME as separate inputs. Winit
  keyboard text is delivered separately from command keys. Never insert the
  character label of a keyboard shortcut or also insert an IME commit as text.
  Prepare current layout before geometry-dependent input. A stale layout is
  rejected; horizontal navigation has a logical fallback before first preparation.
  `TextField::scroll_by` applies a clamped wheel delta against the prepared layout
  and reports whether the offset changed. Normal form Tab traverses; a multiline
  engine may explicitly opt into insertion.
- Prepare `TextPresentation` with actual style, bounds and DPI. Its clipped
  selection, preedit and caret use the shaping engine's geometry. Pass `candidate`
  to `ImeService::set_ime_cursor_area`; do not add window origin or scale twice.
  Long preedit gets a transient display scroll, without changing committed text.
  Focus loss/unmount cancels preedit; equal external echoes preserve it, real
  replacements cancel it. Very small/invalid field bounds are rejected.
- Native context menus return the selected command index. Applications dispatch
  that command through the same `TextField::command` used for keyboard input.
  Read-only fields retain selection and Copy; mutations reject. Disabled fields
  reject actions. A failed platform service is not silently replaced with a fake
  clipboard or command interpreter.
- Numeric fields keep intermediate text (including invalid `-`) separate from
  the committed number. Enter validates/rounds/commits, Escape restores, arrows
  step, and read-only/disabled modes forbid commits. Sliders clamp/snap to range.
- Virtual lists hold stable string keys and virtual focus independently of
  realized rows. Keyboard navigation reveals focus; keyed reorder preserves it;
  removal repairs it. Realization exceeding the declared budget is an error.
  Trees require parents before children; collapse returns descendant focus to
  the ancestor. These are preview list/tree controls, not general tables.
- `PopupStack` handles screen-edge placement, nesting, modal containment checks,
  dismissal and live focus return. The host applies its modal scope to `Runtime`
  and removes owned popups on teardown. `Menu` returns typed actions and never
  executes application effects. `CommandPalette` filters that same menu.
  `SplitPane` enforces both minimums and restores the original size on cancellation.

## Style and accessibility

Resolve `StyleOverride` from the inherited `Style`. `Theme::reload` advances a
revision; compare each node's old and newly resolved styles with `Theme::difference`.
Colors dirty paint only; metrics dirty layout and paint. An overridden color does
not dirty a node when only that inherited color changes. Font and image resource
ownership/reload remain with the text/renderer adapters and their existing versions.
`Image::paint` resolves an app asset to a live renderer image, with no I/O in paint.

Commit `accessibility::Entry` records in parent-before-child order, using the same
runtime IDs, bounds and control state. `SemanticBridge` produces full AccessKit
updates and monotonic non-reused native IDs. Parent replacement removes absent
children. Duplicate IDs/roots, stale revisions, invalid ranges/selections and
multiple focus targets reject atomically. Hidden/disabled ancestors constrain
descendant capabilities. Grapheme byte indices convert to AccessKit character
indices; UTF-16 conversion stays in the native AccessKit backend. Clusters larger
than AccessKit's u8 character-length representation expose value without ranges.
Text-range geometry and line/run navigation are not yet certified native services.

Install `NativeAccessibility` before ever showing a window. `PlatformShell`
forwards native events before application handling and disposes the adapter with
its owner. Callbacks only read a committed tree or queue revision-tagged actions;
the UI thread validates and executes them. Overflow is reported. Revalidate
against the current bridge and runtime; never apply queued requests to a recycled
node or a newer field value. Windows and macOS native adapters are compiled;
Linux AT-SPI is deliberately not enabled in the qualified feature profile.

Desktop services are explicit application actions. File dialogs borrow a live
UI-thread owner, return one path or cancellation, and perform no file I/O.
Incoming file drops report paths; applications decide whether to read them.
Outgoing drag/drop, non-file payloads and Linux/macOS dialogs/clipboard/shell
services are not claimed. Shell activation accepts an absolute file path or HTTPS
target and no command-line arguments; validation is not a security sandbox.

## Gallery and evidence

```text
cargo run -p view-controls --features gallery --example gallery -- --headless
cargo run -p view-controls --features gallery --example gallery -- --gpu dx12
cargo run -p view-controls --features gallery --example gallery -- --run-hidden dx12
cargo run -p view-controls --features gallery --example gallery -- --show dx12
```

Only `--show` opens an interactive window. It supports focus traversal, editing,
native context commands, ranges, radio/checkbox behavior, virtual lists/trees and
native semantic actions. The hidden fixture never takes foreground/input. The
GPU fixture writes readbacks to `target/verification/gallery-*.bmp`; profile
reports are written to `target/verification/control-profiles.txt`. GPU runs
are explicit and are not part of ordinary CPU CI. The gallery is a proof fixture,
not a responsive application template or certification of all controls.

See [M2B evidence](../plan/evidence/m2b-controls.md) for the per-control matrix and
remaining native coverage. An unexecuted native requirement remains uncovered;
passing engine fixtures does not certify a styled text control.

## Horizontal tabs (preview)

`TabList` reuses keyed radio-selection behavior with required selection. Construct
it from `TabOption` entries (stable unique keys, labels, disabled flags). Empty or
all-disabled sets are rejected. Programmatic `select` rejects stale/disabled keys
without changing the previous selection. The host calls `set_focused` as its one
focus stop enters/leaves focus.

Left/Right wrap over enabled entries; Home/End select the endpoints. Key-up,
Ctrl/Alt/Meta chords, vertical arrows and Tab are not consumed. Activation is
automatic on navigation; manual activation, vertical tabs, closing/reordering,
docking and tear-off are not implemented. Applications own panel lifetime and
state, hide inactive interactive descendants from focus/hit testing/semantics,
and decide where focus goes when a panel is removed.

`semantics` exposes a selected tab rather than checkbox state. The existing
custom-role extension maps tab, tab-list and tab-panel to AccessKit roles. The
gallery demonstrates automatic activation and a descriptive panel, including a
disabled tab. It does not certify assistive-technology navigation or expose complete
native tab-to-panel relationships yet. CPU tests cover selection/keyboard rejection;
the gallery test covers panel changes and translated selected/disabled roles.

Run the expanded gallery with:

```text
cargo run -p view-controls --features gallery --example gallery --locked -- --headless
cargo run -p view-controls --features gallery --example gallery --locked -- --show dx12
```

The interactive gallery uses the framework GPU adapter, not the local Review Desk
software painter. Its memory/performance envelope is separate from that demo.
