# view — fresh-session handover

Snapshot: 2026-10-18, after M3A renderer and M2B controls implementation, push,
three-platform CPU CI, and local Windows DX12 verification. The
[canonical checklist](docs/plan/implementation-checklist.md) remains the only
progress board. Recheck Git before changing anything.

## Start here

Repository: `F:\DEV\ui\view` (Windows, PowerShell).
Remote: https://github.com/FunToHard/view.git. Delivery branch: `codex/m2b-controls`.

- Renderer: `27b3ec4` — Implement M3A wgpu 2D renderer and GPU verification.
- Controls: `b36874eb20c89bfc46569318f6c32a7fd8c26b37` — Implement M2B standard controls and accessibility.
- At the user's request, `a8a0b5f` and `f6971b0` were combined into the controls
  commit, including the final gallery regression fix. The renderer remains separate.
- Both implementation commits are pushed. Local HEAD and remote branch matched,
  with a clean working tree before this handover update. This branch has not been
  merged to main by this task. Use `git status --short` and `git log -3 --oneline`
  for the latest documentation commit/worktree state.

Read before implementation:

1. [AGENTS.md](AGENTS.md).
2. [Design index](docs/design/README.md) and [decisions](docs/design/09-decisions-and-questions.md).
3. [Milestone plan](docs/plan/README.md), [backlog](docs/plan/initial-backlog.md), and the canonical checklist.
4. The affected subsystem design; sketches are not existing API signatures.
5. [Rendering guide](docs/guides/rendering.md), [controls guide](docs/guides/controls.md),
   [text guide](docs/guides/text-editing.md), and [package profiles](docs/plan/feature-profiles.md).
6. [M3A evidence](docs/plan/evidence/m3a-renderer.md) and
   [M2B evidence](docs/plan/evidence/m2b-controls.md).

The user authorized implementation, commits, pushing and CI verification, then
requested the squash and this handover update. Do not infer authorization to
merge, publish releases, provision infrastructure, or implement the remaining
roadmap. Historical planning-only statements do not revoke implementation scope.

## Product and boundaries

view (The devil's framework) is a native Rust UI framework, Windows 11 x64 first.
Linux/macOS have CPU CI coverage, not full native product parity. One Cargo
workspace, Rust 1.98.1, edition 2024, resolver 3; packages remain unpublished.
First-party code is MIT OR Apache-2.0. Both lockfiles are tracked.

- Retained and on-demand immediate UI share identity, ownership, lifecycle, input
  and semantics. Build/measure are pure; effects execute once.
- Application documents/scenes stay outside the UI tree. Demand-driven UI and
  independently scheduled viewports use separate work/revision streams.
- Ordinary Rust builders, typed actions and explicit revisions; no mandatory DSL,
  ECS, VM, browser, network stack or global mutable application model.
- Core stays backend-independent. Unsafe/FFI belongs in adapters with documented
  ownership/thread invariants. The facade forbids unsafe code.
- Standard controls share behavior engines and declared profiles. Engine tests
  alone do not certify native control behavior.
- Physical input requires an explicitly selected desktop session, foreground
  checks and held-input cleanup. None was selected for this task.
- Future webview/add-on/tooling modules stay optional. Add-ons are trusted native
  processes, not automatically sandboxed or Rust-ABI-stable.

## Implemented state

| Package / area | Current behavior |
| --- | --- |
| `view-core` | Identity, lifecycle, actions, revisions, keyed/immediate ownership, layout/hits, focus/modal/scroll, committed semantics; capture revocation when an owner becomes hidden or modal-excluded |
| `view-testing` | Deterministic harness clock/input/completion queue and service fixtures |
| `view-text` | Shared grapheme-aware editor, history, controlled values, validation, clipboard/IME state, multiline/reveal, profile reports; optional cosmic-text shaping and bounded caches |
| `view-wgpu` | Optional 2D renderer: surfaces/offscreen targets, ordered paint snapshots, shapes/paths/images/glyphs, clipping/layers/SDR alpha, bounded caches, provider lifecycle and contribution validation |
| `view-controls` | Shared behaviors, themes, labels/images/buttons, toggles/radio, text/numeric fields, sliders, scrolling/virtualized lists/trees, split panes/popups/menus/command palette; optional rendering and AccessKit bridge |
| `view-platform` | winit lifecycle/input/IME, optional AccessKit native adapter, Windows clipboard, scoped file/folder dialogs, native context menu and validated shell activation; normalized file-drop events |
| `view` | Core by default; optional text/shaping, wgpu, controls, controls-render and accessibility profiles |
| Gallery / consumers | Explicit headless, GPU, hidden-window and visible gallery modes; independent consumer tests prevent workspace feature-unification assumptions |

M1A, M1B and M2A were previously merged to main. M3A and M2B are on the delivery
branch above. M3A-01–13 and M2B-01–10/12–14 are checked with evidence; M3A-14 and
M2B-11 remain open. M3B 3D/host proofs are not implemented by this task.

The gallery uses runtime-owned controls, shared text geometry, revision-aware
semantics and demand-driven caret blink. Its final regression fix routes pointer
cancellation using the routed action and gives pointer/accessibility button
activation the same application effect as keyboard activation.

## Contracts to preserve

- Editor indices are UTF-8 bytes; committed carets are grapheme boundaries.
  UTF-16 conversion rejects surrogate interiors; preedit cursors use scalar boundaries.
- Revisions belong to one owner. Equal-value echoes preserve state; stale echoes
  reject; real external replacements clear history/composition according to policy.
- Shape immutable snapshots. Preserve font ownership and content/metrics/DPI/cache
  identities through renderer submission and resource retirement.
- Commit, GPU submission and presentation are distinct observations. Keep wgpu out
  of core and validate provider device generations and attachment compatibility.
- AccessKit snapshots validate atomically; native IDs are not reused. Actions carry
  semantic revisions and reject stale, disabled or unsupported requests. Native
  callbacks queue work; application effects run through the normal runtime.
- Read-only fields remain selectable/copyable. Styling must retain shared editing,
  focus, selection, preedit and accessibility behavior.
- Native service adapters enforce UI-thread ownership. Linux/macOS desktop services
  can return Unsupported; Linux AT-SPI is disabled in the qualified feature set.

## Verified delivery

[CPU CI run 39105597775](https://github.com/FunToHard/view/actions/runs/39105597775)
passed on exact commit `b36874e`: Windows 2025, Ubuntu 24.04 and macOS 15. All three
jobs completed successfully; the macOS runner-capacity annotation was not a failure.

Local `pwsh -File scripts/verify.ps1 -Gpu -Backend dx12` completed successfully on
2026-10-18. It covered formatting, locked workspace/all-target checks, strict Clippy,
tests/docs, supported feature profiles, independent consumers, dependency inventory,
platform probes and hidden-window lifecycle/error fixtures. The separate gallery
regression test also passed:

```powershell
cargo test -p view-controls --features gallery --example gallery --locked
```

Actual local GPU evidence used NVIDIA GeForce RTX 3050 6GB Laptop GPU,
driver `32.0.16.1714`, DX12:

- Compute dispatch/readback and renderer shapes, paths, clips, blending, layers,
  images, glyph ordering/font lifetime, cache eviction and revision capture passed.
- Gallery readbacks at 100%, 150% and 200% passed, including shared
  selection/preedit/caret geometry and visible-versus-hidden caret pixel checks.
- Hidden gallery surface, AccessKit installation/update and IME candidate-geometry
  service call passed. This is not an actual IME or assistive-technology client test.
- Hidden native surface acquire/submit/present, coalesced resize and
  zero-size/occlusion suspension passed; visible scanout was not certified.

Earlier Windows Vulkan renderer evidence is recorded in the M3A report; the final
full local verification above used DX12. Generated logs, environment metadata and
readbacks are in ignored `target/verification/`. CI artifacts expire after 14 days;
durable scope and limitations remain in the evidence reports and this handover.

## Open coverage and next work

- **M2B-11:** actual OS IME candidate/preedit placement across clipping, scrolling
  and DPI needs an explicitly selected desktop session. Model tests and the native
  geometry service call do not close this gate.
- **M3A-14:** Linux/macOS GPU execution remains missing. CPU CI success does not
  certify graphics, native windows, clipboard/IME or accessibility parity.
- Physical mouse/keyboard E2E and native assistive-technology client behavior remain
  uncertified. Per-control reports are preliminary, not full native certification.
- The user explicitly said skip M0. Keep M0-12, M0-13 and M0-15 open; do not
  automatically provision runners or broaden infrastructure scope.
- Historical native clipboard round trips passed on a system-assigned noninteractive
  station, but the hardened fresh-station replay was blocked by station availability.
  This task did not replay it. Do not weaken isolation or touch the interactive
  clipboard to obtain a pass.

Choose subsequent work from the user's request and canonical checklist. Close
remaining coverage only with actual evidence. Do not restart the already implemented
M3A/M2B packages or treat M3B and later roadmap items as authorized automatically.

## Useful commands

```powershell
pwsh -File scripts/verify.ps1
pwsh -File scripts/verify.ps1 -Gpu -Backend dx12
cargo test -p view-controls --all-features --locked
cargo test -p view-controls --features gallery --example gallery --locked
cargo test --manifest-path tests/compatibility/facade-consumer/Cargo.toml --all-features --locked --target-dir target/consumer
```

Explicit clipboard verification requires a fresh isolated station:

```powershell
pwsh -File scripts/verify.ps1 -NativeClipboard
```

Normal builds/tests must not seize input. Generated artifacts belong in `target/`
or `work/`. Check `$LASTEXITCODE` when sequencing native PowerShell commands.
Documentation-only edits need local link/fence checks and `git diff --check`, not
another Rust rebuild.
