# Community controls and behavioral completeness

Status: revision 0.4 selected P-20/P-21 baseline. No controls or test harness are implemented yet.

## Publish controls as ordinary Rust crates

A developer should be able to publish a Markdown previewer, code editor, timeline, chart, or inspector as a normal Cargo library. Consumers depend on it and compose its exported View/component using the same mechanisms as first-party controls. This is compile-time source integration; it does not require a runtime plugin loader. Cargo already supplies the package distribution workflow. [Cargo publishing](https://doc.rust-lang.org/cargo/reference/publishing.html)

```rust
// Hypothetical API and package name, not an existing published crate.
use view::prelude::*;
use community_markdown::MarkdownPreview;

fn document_panel(model: &Document) -> impl View + '_ {
    Column::new((
        Text::new("Preview"),
        MarkdownPreview::new(&model.source)
            .on_link(Action::OpenLink),
    ))
}
```

The author implements composition first, custom layout/paint only where needed, and a render provider only for genuine custom GPU work. A composite control does not need to register a global plugin, create an event loop, or implement its own window. It can expose typed properties/events and optional advanced state handles.

The exact View/action-generic/lifetime signatures remain subject to the public API prototype. Published controls must compile against the host's compatible view contract; two incompatible copies of core View/NodeId types are not interchangeable merely because they share names.

## Package contract

A control package declares supported view versions, minimum Rust version, platform/feature requirements, resource behavior, required services, and optional dependencies. It documents controlled versus internal state, event types, keys/lifecycle, style hooks, accessibility, automation selectors/actions, and its conformance profiles.

The core control interface should not expose wgpu objects unless the package is a GPU provider. This reduces version coupling for ordinary reusable widgets. Parser/codecs belong to the control's feature graph; the base framework does not acquire every Markdown/image/editor dependency.

A Markdown previewer needs explicit policies for supported syntax, selectable text, links, scrolling, images/asset loading, source updates, and semantic headings/links. Link activation is an application action by default. It should not secretly launch external programs or fetch arbitrary URLs just because source text was supplied. Rich HTML, math, diagrams, code highlighting, and live execution are separate declared features.

An independently maintained test fixture should consume the control only through its public crate API. The eventual package checks include Cargo packaging/dry-run validation, feature combinations, documentation examples, and supported-platform builds; no package is published during design work.

## Standard controls need behavior profiles

The framework cannot prevent every missing feature by adding more individual examples. Adopt a versioned capability/behavior profile for each standard control family, reusable behavior engines, and a conformance runner.

Distinguish:

- **Primitive:** drawing/input building block with no claim of complete control behavior.
- **Standard control:** implements the required behavior of its declared profile and target platform.
- **Specialized control:** publishes its profile extensions and deliberate differences.

A painted rectangle accepting character events is a primitive. It cannot be advertised as the standard TextInput while deletion or selection is absent. A profile test skipped on an unavailable native runner remains uncovered; it does not certify the control.

Profiles have IDs, revisions, mandatory behaviors, optional behaviors, configuration variants, and explicit not-applicable cases. A conformance report identifies the exact control version, profile, platform, options, and test coverage. Passing a suite is useful evidence, not a promise of zero bugs or indefinite compatibility.

## Shared behavior, replaceable presentation

Proposal: separate a text editing session from text shaping/layout and control decoration. TextInput, TextArea, and styled variants reuse that session where their semantics match. It owns selection/composition, editing commands, history integration, and scroll-to-caret behavior; rendering consumes its observable state.

Application text ownership must be explicit. Controlled edits carry document/value revisions or edit transactions; an old value echo cannot reset current selection or overwrite IME composition. External replacements follow a documented selection/composition/history policy. Never rebuild the entire editing session on each paint or network refresh.

Application document undo and field-local undo coordinate through command scope. A rich document editor can replace the backing text/document model while implementing the editing contract. RichTextEditor is a separate profile; a plain-text editing engine does not automatically supply Word-style structure, formatting, pagination, tables, or collaborative editing.

Apply the same approach to focusable/pressable behavior, range adjustment, selection models, popup ownership, and scrolling. Composition reuses behavior without forcing one visual theme. Native service integration remains in the platform adapter.

## Proposed Windows SingleLineText profile v1

This is a proposed baseline to refine against native behavior, not a claim that one standard covers every editor. Windows edit-control documentation provides a useful reference for selection, clipboard, modification, and undo behaviors. [Edit controls](https://learn.microsoft.com/en-us/windows/win32/controls/about-edit-controls), [Text operations](https://learn.microsoft.com/en-us/windows/win32/controls/edit-controls-text-operations)

| Area | Required behavior / policy | Evidence |
| --- | --- | --- |
| Insertion | Committed text insertion and replacement of selection; valid text limits | Typing/paste at beginning, middle, end, and over selection |
| Deletion | Backspace/Delete and selected-range deletion; defined word-delete commands | Empty/boundary/range/Unicode fixtures |
| Navigation | Arrow movement, Home/End, word movement, platform modifier mappings | Expected caret positions and scrolling |
| Selection | Shift extension, pointer drag, word selection, select-all; direction/anchor preserved | Keyboard/pointer and out-of-bounds drag |
| Unicode | Explicit text indexing and boundary policy; no byte slicing through UTF-8 | Combining marks, emoji sequences, supplementary characters and mixed scripts |
| Bidi/shaping | Defined logical/visual movement and caret/hit behavior through shaped runs | Mixed LTR/RTL, ligatures, selection rendering |
| Clipboard | Copy/cut/paste; multiline paste policy for a single-line field | Native clipboard plus deterministic adapter tests |
| History | Undo/redo with defined typing/paste/composition grouping | Round trips restore text and appropriate selection |
| IME | Preedit, commit/cancel, caret/candidate placement, composition replacement | Named native input-method fixtures |
| Focus | Tab entry/exit, focus indication, capture loss, caret visibility/blink | Keyboard-first and native activation tests |
| Scrolling | Horizontal overflow and caret reveal without losing selection | Long content and drag selection beyond bounds |
| Modes | Read-only allows applicable selection/copy; disabled behavior is explicit | States, focus eligibility and semantics |
| Commands | Standard shortcut/context-menu actions routed consistently | UI, keyboard and semantic commands agree |
| Values | External updates, validation/error state, empty/placeholder, length policy | No stale value echo or composition corruption |
| Accessibility | Name/role/value, selection/text actions as supported, error/read-only state | Native bridge and semantic contract tests |
| Lifecycle | Rebuild preserves compatible session; removal resolves focus/capture/IME | Keyed reconciliation and unmount cases |
| Automation | Observe value/selection/focus and exercise physical/semantic actions distinctly | Conformance harness and desktop E2E |

Unicode segmentation supplies grapheme and word boundary rules, with tailoring for particular environments. It does not alone decide all native deletion or bidi navigation behavior; the profile documents those policies and tests their interaction with shaping/IME. [Unicode UAX #29](https://www.unicode.org/reports/tr29/)

The profile must explicitly define whether lengths/indices refer to bytes, scalar values, grapheme clusters, or platform offsets. Password entry uses a separate profile with masking and clipboard/diagnostic policies. Multiline fields add line navigation, newline handling, vertical movement, wrapping, and scroll behavior. Code editors can opt into Tab indentation; ordinary form fields should not accidentally trap Tab.

## Other profile families

| Family | Common omissions to cover |
| --- | --- |
| Button / toggle | Keyboard activation, press/cancel, disabled semantics, focus, repeated activation rules |
| Checkbox / radio | Checked/mixed policy, group movement, labels, required exclusivity |
| Slider / numeric input | Keyboard steps/page steps, bounds, precision, invalid intermediate text, commit/cancel |
| List / tree / table | Stable selection, range/multi-select, keyboard navigation, virtualized focus, expand/collapse |
| Menu / popup / dialog | Escape/outside behavior, focus containment/return, nesting, screen-edge placement |
| Scroll view | Nested routing, boundaries/chaining, keyboard scrolling, reveal-target, drag/capture |
| Selectable document | Selection across blocks, links, copy formats, navigation, semantic structure |

Use interaction matrices, not just isolated features: selection plus IME, read-only plus copy, refresh plus focus, and virtualization plus keyboard navigation catch failures that a feature count misses.

## Conformance lifecycle

1. Name the control's profile and supported target/configuration before implementation.
2. Write the required behavior matrix and independent expected outcomes.
3. Reuse the shared behavior engine; extend it or a documented specialized profile where necessary.
4. Run headless command/state tests, layout/render fixtures, semantic tests, and applicable native E2E.
5. Publish passed/failed/uncovered/not-applicable results and known limitations.
6. Turn every reported missing expected behavior into a profile decision plus regression coverage.

The SDK should ship reusable tests and profile adapters so a community author does not invent the standard keyboard/selection suite anew. Assert that unsupported actions are actually absent/rejected; a list of advertised capabilities is not enough. Reference native controls and independent analytic fixtures where useful, without treating screenshot equality as the definition of all behavior.

Fuzz/property tests can exercise valid selection bounds, undo round trips, edit sequences, and lifecycle transitions. They complement explicit native semantics rather than infer correctness from the same implementation under test.

## Initial demonstration

Build the standard text field from the shared engine and prove Delete, selection, clipboard, undo, and IME before it is called complete. Then build a visually different field using the same behavior and run the same suite. Finally consume a Markdown preview control from a separate crate using only public interfaces. These experiments require implementation authorization and are tracked in the delivery plan.
