# view — fresh-session handover

Snapshot: 2026-10-08, after M2A implementation, passing three-platform CPU CI and merge to main. This file transfers context; the canonical checklist remains the only progress board. Recheck Git before changing anything.

## Start here

Repository: `F:\DEV\ui\view` (Windows, PowerShell).
Remote: https://github.com/FunToHard/view.git. Current branch: `main`.
Tested implementation: `a927a9e4522b74267f7a73d7cac20a967a64c344`.
This handover and CI evidence are a subsequent documentation-only commit. Use `git log -3 --oneline` and `git status --short` for the exact current head.

Read before implementation:

1. [AGENTS.md](AGENTS.md).
2. [Design index](docs/design/README.md) and [decisions](docs/design/09-decisions-and-questions.md).
3. [Milestone plan](docs/plan/README.md), [backlog](docs/plan/initial-backlog.md), and [canonical checklist](docs/plan/implementation-checklist.md).
4. The affected subsystem design; sketches are not existing API signatures.
5. [Text guide](docs/guides/text-editing.md), [package profiles](docs/plan/feature-profiles.md), and [M2A evidence](docs/plan/evidence/m2a-text-editing.md).

Historical planning-only paragraphs do not revoke implementation authorization. The latest completed request was to push M2A, verify CI, merge to main and update this file. Do not infer authorization for releases, infrastructure or the entire remaining roadmap from this handover.

## Product and boundaries

view (The devil's framework) is a native Rust UI framework, Windows 11 x64 first. Linux/macOS have CPU CI coverage, not full native product parity. One Cargo workspace, Rust 1.98.1, edition 2024, resolver 3; packages remain unpublished. First-party code is MIT OR Apache-2.0. Both lockfiles are tracked.

- Retained and on-demand immediate UI share identity, ownership, lifecycle, input and semantics. Build/measure are pure; effects execute once.
- Application models/documents/scenes stay outside the UI tree. Demand-driven UI and independently scheduled viewports use separate work/revision streams.
- Ordinary Rust builders, typed actions and explicit revisions; no mandatory DSL, ECS, VM, browser, network stack or global mutable application model.
- Core stays backend-independent. Unsafe/FFI belongs in adapters with documented ownership/thread invariants. The facade forbids unsafe code.
- Shared behavior engines and profile tests precede standard-control certification. Engine tests alone do not prove rendered/native controls.
- Native decorations remain the baseline. Physical input requires an explicitly selected desktop session, foreground checks and held-input cleanup.
- Future webview/add-on/tooling modules stay optional. Add-ons are trusted native processes, not automatically sandboxed or Rust-ABI-stable.

## Implemented state

| Package / area | Current behavior |
| --- | --- |
| `view-core` | Generational identity, lifecycle, actions/cancellation, revisions/demand, keyed/immediate ownership, layout/transforms/clips/hits, focus/modal/scroll and committed semantics |
| `view-testing` | Deterministic harness clock/input/completion queue and service fixtures |
| `view-platform` | winit shell/window lifecycle, DPI/client origins, input/IME translation, callback errors; optional editor IME and Windows clipboard adapters |
| `view-text` | Grapheme/word editing, directional selection, logical/visual/vertical/pointer movement, history, controlled values, validation, modes, blink, clipboard/IME state, multiline/reveal and profile reports |
| `view-text/shaping` | cosmic-text system/owned fonts, fallback, wrapping, bidi caret/hit geometry, bounded cache and retained font lifetime |
| `view` | Core by default; optional `text` and `text-shaping` features |
| Fixtures/consumers | Three OFL Noto fonts with license/hashes; independent default/harness/text/shaping consumers |

M1A, M1B and M2A implementations are on main. M1B's ten review corrections covered empty clips, transform order/layout origins, scrolling geometry, modal/hidden focus, committed semantics, Linux backend features, callback errors and client screen origins. See [M1B evidence](docs/plan/evidence/m1b-review-fixes.md).

No production `view-wgpu` renderer or `view-controls` package exists yet. M2A is the shared behavior/backend layer, not a certified TextInput/TextArea.

## Text contracts to preserve

- Indices are UTF-8 bytes; committed carets are grapheme boundaries. UTF-16 conversion rejects surrogate interiors. IME preedit cursors use scalar boundaries.
- Revisions belong to one text owner. Equal-revision/equal-value echoes preserve state; stale echoes reject. Real external replacements clear field history/composition and reset or clamp selection by policy.
- Typing groups within 1,000 ms at a continuing caret. Paste and composition are separate transactions. Undo/redo restore text/selection while advancing value revision.
- CRLF/CR normalize to LF. Single-line newline policy is space or rejection; multiline Tab insertion is opt-in. Limits count resulting graphemes.
- Shape immutable snapshots. Cache identity includes content/revision, font/metrics, width/wrap, DPI and font collection revision. Reload invalidates caches; existing layouts retain font references. GPU atlas lifetime remains future work.
- Logical/visual movement is explicit. Candidate geometry snaps scalar preedit offsets by affinity and accounts for scroll/control origin.
- Profile reports retain options and uncovered cases; they do not certify a complete control.

## Verification and Git

[M2A CPU CI run 37773960528](https://github.com/FunToHard/view/actions/runs/37773960528) passed Windows 2025, Ubuntu 24.04 and macOS 15 on `a927a9e`. That exact commit was fast-forwarded to main and pushed. [Actual CI environments](docs/plan/evidence/m2a-cpu-environments.json) record OS/architecture/compiler and execution flags.

The full script checks formatting, locked all-target builds, strict Clippy, workspace tests/docs, optional text/platform features, independent consumers, fixture hashes and dependency inventory. M2A has 21 text tests with shaping, one normalized IME adapter test and five combined consumer tests. Generated properties run 3,200 edit steps. Windows also passes hidden-window lifecycle and ready/event/idle callback-error checks. Full local verification passed.

Merged branches were retained, not deleted:

- `codex/m1a-runtime`: `3d685d5` (implementation `7b3053b`).
- `codex/m1b-review-fixes`: `6676e67` (implementation `86183dd`, portability `d7add72`).
- `codex/m2a-text`: `a927a9e` (foundation `32089b5`).

## Open coverage

The user explicitly said **skip M0**. Preserve unchecked gates: M0-12 GPU platform coverage, M0-13 dedicated interactive Windows session and M0-15 engineering gate. Do not expand into runner provisioning automatically. Prior local Windows DX12 compute/readback does not prove rendering or other GPU platforms.

Native clipboard round trips passed against the system-assigned noninteractive logon station. The fixture was then hardened with `CWF_CREATE_ONLY` to refuse reuse. Its final fresh-station replay is blocked in the current session because that generated station exists. Named station creation requires administrator membership. Do not weaken isolation or overwrite the interactive clipboard to get a green result. This fixture is explicit opt-in and was not run by CPU CI.

Native Japanese IME sequences/candidate placement, rendered text/carets, physical drag/input, native accessibility and complete control lifecycle/shortcut conformance remain uncovered. Linux/macOS CPU success does not establish native-window, clipboard/IME or GPU parity.

## Next planned work

The canonical sequence next reaches **M3A-01**: introduce `view-wgpu` and negotiate adapter/device capabilities, followed by surfaces/offscreen targets and paint/render records. Read [rendering and interop](docs/design/04-rendering-and-interop.md) and affected acceptance scenarios first. Keep wgpu out of core and preserve commit/submission/presentation distinctions.

M2B controls/accessibility follow rendering in the ordered checklist. Scope the next implementation from the user's fresh-session request rather than assuming authorization for the remaining roadmap.

## Commands

```powershell
pwsh -File scripts/verify.ps1
cargo test -p view-text --all-features --locked
cargo test -p view-platform --features text --locked
cargo test --manifest-path tests/compatibility/facade-consumer/Cargo.toml --all-features --locked --target-dir target/consumer
```

Explicit native clipboard verification, only with a fresh isolated station available:

```powershell
pwsh -File scripts/verify.ps1 -NativeClipboard
```

Generated output belongs in ignored `target/` or `work/`. CI artifacts expire after 14 days; durable summaries/manifests live in `docs/plan/evidence/`. Check `$LASTEXITCODE` when sequencing native PowerShell commands. Documentation-only edits need links/fences/JSON and `git diff --check`, not another Rust rebuild.

The user authorized pushing/merging this work and updating this file; that delivery is complete. Future external actions follow the next request and AGENTS.md.
