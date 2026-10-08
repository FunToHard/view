# Shared text editing and shaping

Enable `view/text` for `PlainEditor`, and `view/text-shaping` for `CosmicBackend`.
Platform IME/clipboard helpers are under `view-platform/text`. These are behavior
engines and a CPU text backend, not a certified standard control.

```rust
use view::text::{EditKind, EditorConfig, EditorSession, PlainEditor};
let mut editor = PlainEditor::new("", EditorConfig::default()).unwrap();
editor.insert("Hello", EditKind::Typing, 0).unwrap();
editor.delete(true, false, 10).unwrap();
assert_eq!(editor.snapshot().text.text(), "Hell");
```

Keep the session in compatible node-local state across rebuilds. Dispatch input
once and shape immutable snapshots during layout. Keyboard/menu/semantic commands
must route through the same engine. Control lifecycle/semantics wiring is a later
milestone; dropping the session releases field history and composition.

## Value and edit policies

- Indices are UTF-8 bytes. Committed selection/caret endpoints are extended
  grapheme boundaries. UTF-16 conversion rejects surrogate interiors. IME cursor
  ranges use scalar boundaries in preedit text.
- Backspace/Delete remove a selection or one grapheme. Unicode word-start commands
  skip punctuation/whitespace. Arrows are logical; visual movement is explicit
  through shaped geometry. Home/End use hard lines; vertical movement uses wrapped
  lines and remembers horizontal position. Selection preserves anchor/direction.
- CRLF/CR normalize to LF. Single-line paste substitutes spaces or rejects the
  entire edit, according to configuration. Enter returns false for host submit
  handling. Multiline Enter inserts LF. Tab traverses unless multiline insertion
  is explicitly enabled.
- Limits count graphemes in the resulting value, including combining merges.
  Limits and host validators reject the whole edit. Errors are returned for host
  error presentation/semantics.
- Equal-revision equal-value echoes preserve selection/preedit/history. Stale
  echoes reject. Real external replacements require the current base revision,
  cancel preedit, clear history/scroll, and reset or clamp selection. Revisions
  belong to a text owner and cannot be compared across owners.
- Typing within 1,000 ms groups only at a continuing empty selection. Navigation,
  focus, paste and composition break groups. Paste and IME commit each form one
  transaction. Undo/redo restore text/selection while advancing value revision.
  History is bounded. Application undo should own the surrounding domain
  transaction rather than replaying a second field undo.
- Read-only permits selection/copy but rejects edits/history. Disabled rejects
  selection/clipboard/edits and removes caret focus. Placeholder is visible only
  for an empty value without nonempty preedit. The caller supplies milliseconds;
  focused caret blink uses 500 ms phases with no background timer.

## Shaping, composition and native services

System font discovery is explicit. `from_fonts` uses only caller-owned bytes.
Reload increments the backend font revision and clears a bounded LRU. Requests
must carry that revision. Content, text revision, family, metrics, width, wrap,
DPI and font revision all affect cache identity. Metrics remain logical units;
DPI affects raster/cache identity, not an extra logical-space scale.

Fallback follows cosmic-text; missing glyphs are counted. Ligature carets divide
cluster advance equally among graphemes. Bidi/wrap positions carry affinity.
Layouts retain font references and remain valid after backend reload/drop. GPU
atlas allocation and rendered glyph output remain M3 work.

Preedit never changes committed text/revision/history. Its display snapshot
replaces the captured selection; commit records one edit and cancel restores the
committed observation. The native adapter preserves IME cursor-hidden state.
Shape the display snapshot for candidate reporting; the adapter adds control
window origin after subtracting viewport scroll. Native rendered candidate
placement requires later dedicated IME/render acceptance tests.
Candidate geometry snaps an IME scalar offset inside a display grapheme according
to affinity. Explicit host scroll offsets clamp to content extents; pointer
selection rejects invalid viewport geometry before changing selection.

Clipboard operations use a service trait. Tests use `MemoryClipboard`; Windows
uses `NativeClipboard` borrowed from a live window. Busy access returns an error
for host retry. The explicit smoke fixture uses a private noninteractive window
station, performs UTF-16 round trips, and exits without switching desktops,
injecting input or accessing the user's interactive clipboard. Creation is
strict: an existing generated station makes the fixture fail rather than reuse
its clipboard. Run it explicitly with `scripts/verify.ps1 -NativeClipboard` in
an environment where a fresh station can be created.

## Profiles

| Behavior | SingleLineText v1 | PlainMultiline v1 | Independent expected fixture |
| --- | --- | --- | --- |
| Selection/replacement | Mandatory | Mandatory | `abcd`, reverse selection 3..1, insert X → `aXd` |
| Grapheme/word deletion | Mandatory | Mandatory | `a👩‍💻` Backspace → `a`; `one two` word Delete → `two` |
| History/controlled values | Mandatory | Mandatory | Undo `abcZ` → `abc`; stale echo rejects without changing composition |
| Modes/limits/validation/blink | Mandatory (configured limits optional) | Mandatory (configured limits optional) | Read-only rejects edits; 500 ms virtual time hides caret |
| Clipboard/IME state | Mandatory | Mandatory | Paste over `b` in `abc` → `a😀c`; preedit leaves committed value unchanged |
| Bidi/geometry/reveal | Mandatory | Mandatory | RTL visual-left advances logical byte index; wrapped drag reveals caret |
| Insert newline | Not applicable (host submit) | Mandatory | Enter before `a` → LF + `a` |
| Insert Tab | Not applicable (traverse) | Optional (traverse by default) | Configured insertion before `a` → Tab + `a` |
| Native IME/render/accessibility/control lifecycle | Mandatory, uncovered | Mandatory, uncovered | Dedicated later native/render scenarios |

Reports retain adapter/version, platform, profile, options and individual results.
A failed configured case or uncovered mandatory case prevents certification.

`SingleLineV1` and `PlainMultilineV1` expose mandatory, optional and not-applicable
cases. `ProfileAdapter` runs independent expected edits through public commands.
The reusable runner tests nine common headless cases plus multiline newline/Tab.
Limits/validation, blink/placeholder and shaping/reveal have direct API tests;
generic adapter reports leave unexecuted cases uncovered. Native IME, rendered
caret, control command/lifecycle wiring and native accessibility remain uncovered.
These reports cannot certify a complete standard text control.

See [M2A evidence](../plan/evidence/m2a-text-editing.md) for executed verification.
