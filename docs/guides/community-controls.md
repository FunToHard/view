# Public control composition — 0.1

Ordinary control packages depend on compatible `view`/`view-core` 0.1 APIs and
Rust 1.98.1. They do not require a process add-on, registration server, platform
patch, window or event loop. The [authoring guide](authoring.md) is the reference.

| Contract | Public interface | Ownership |
| --- | --- | --- |
| Composition | `IntoElement`, `Element`, `Description` | Pure owned descriptions; stable sibling keys |
| Layout | `LogicalRect`, constraints/flex/stack engines | Logical coordinates, validated finite geometry |
| State/events | `Runtime::state/update/enqueue`, typed actions | Generational runtime owner; app document separate |
| Input | Runtime pointer/keyboard routing, shared control engines | One focus/capture model; no second dispatcher |
| Semantics | `Description::role/name/value/semantic_actions`, committed snapshot | Same node IDs as layout/input |
| Paint | Optional `view::render::{PaintSnapshot, Paint, Draw, Primitive}` | Explicit renderer/device and resources |
| Testing | Headless runtime and package-specific profile adapter | CPU/render/native evidence reported separately |

A package may provide pure paint records or use the host's semantic/text painter;
it need not depend on wgpu for composition. Custom input must preserve the shared
control behavior it claims. Primitive controls cannot advertise standard editing
behavior they do not implement. Layout/paint do not perform IO or application
effects. Action queue saturation returns the required action to its producer.

## view-markdown block-preview/1

`view-markdown` is a separate unpublished Cargo package, depending only on the
minimal public facade. It has no parser, codec, platform, renderer, add-on or
network dependency. It demonstrates a deliberately small specialized preview,
not a CommonMark implementation or a rich-text editor.

Supported syntax: paragraphs (one nonempty source line per block), ATX headings
1–6, bare triple-backtick fenced code, and whole-line `[label](https://target)` /
`![alt](https://target)` references. Other syntax, HTML and unsafe schemes render
literally. No scripts, raw HTML, inline markup parser or external program launch.
Image references expose alt text and an asset request candidate; the app decides
whether to fetch/decode/display them. They are not fetched by the package.

Selection is an explicit UTF-8 scalar-boundary range across flattened block text,
with newline separators; copy returns plain text. The profile does not claim
pointer selection, visual/bidi navigation, rich clipboard formats, editing or
native document accessibility. Host scrolling uses the normal runtime scroll
container. Links participate in keyboard focus and declare Click/Focus semantics;
headings/links/images use named semantic roles. Hosts supply the paint/input
adapter and retain authorization over `LinkAction` requests.

`update(source, base_revision)` parses before mutation, rejects stale revisions and
oversized sources, then clears selection and advances revision. Block keys include
the content revision; replacement remounts blocks, so the app chooses focus
fallback. `activate(block_index, revision)` rejects stale/non-link activation and
returns a typed request. No implicit link side effect occurs.

Default limits are 256 KiB source, 4,096 blocks and 2,048 bytes per target. Hosts
may choose tighter limits. Source/doc lifetime and network/cache policy are
independent of the preview node. Native input/AT certification remains uncovered.

`conformance::SelectablePreview` and `check_selection` verify the profile's
cross-block copy, byte bounds, Unicode-boundary rejection and atomic failure.
The independent project in `tests/compatibility/facade-consumer` enables
`markdown` and runs the adapter plus public composition/semantics checks. It does
not depend on workspace feature unification. Package contents are inspected with
`cargo package -p view-markdown --list --allow-dirty`; registry publication and
registry-dependent packaging verification are separate release work.
