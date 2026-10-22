# M4 authoring and application proofs

Implementation increment: 2026-10-19. Windows x64 local evidence; no remote CI
execution or Linux/macOS runtime result is inferred. Canonical progress remains
the [implementation checklist](../implementation-checklist.md).

## Implementation mapping

| IDs | Implementation / acceptance |
| --- | --- |
| M4-01/02/06 | `view::authoring` trees, fixed tuples, properties, keyed iterators/branches; authoring tests retain local state, validate nested duplicate keys before mutation, resolve removal/focus and preserve retained/immediate islands |
| M4-03/04 | Optional `view-resource`: owner lease, versioned effects/completions, subscriptions, cancellation, retained refresh, explicit retry/offline/reconnect; foreign/reordered/replayed/unmounted completions rejected |
| M4-05 | Bounded FIFO plus replaceable previews, sequence resynchronization, worker fixture, bounded LRU/expiry and RGBA/opt-in BMP; queue saturation retains required events; known pixels/padding/limits tested |
| M4-07 | Versioned local section registry with allowed typed actions/assets, stable keys, fallback, depth/items/bytes and duplicate checks; forbidden remote actions rejected |
| M4-08/09/10 | `view-application-proofs`: shared document, hierarchy/inspector, scene picking/gizmo/undo, detached IDE/Markdown, virtual catalog, retained search, refresh/offline/reconnect, local remote sections, decoded images/progress and worker reordering |
| M4-11/12/13 | Public community contract and separate `view-markdown` package; explicit syntax, selection/copy, links/images/semantics/source-update profile; independent consumer and reusable profile adapter |
| M4-14 | Eight runnable facade examples: minimal, form, keyed_list, async_branch, two_windows, immediate, custom_control, viewport |
| M4-15 | Local compiler-driven onboarding exercise below; independent blind human/model evaluation is not claimed |
| M4-16 | Integrated headless flows, worker race tests, offscreen GPU output and three hidden native surfaces; no private patches in consumer packages |

## Executed results

`pwsh -File scripts/verify.ps1 -Gpu -Backend dx12` completed successfully on
2026-10-19. This ran the required workspace format/check/Clippy/test/doc gates,
supported feature profiles, the independent consumer matrix, all eight runnable
examples and the integrated application, GPU and hidden-native proofs. Clippy
used `-D warnings`; Cargo checks used the committed lockfile resolution.

Focused behavior coverage includes authoring (3 tests), resource contracts (5),
optional BMP decoding (1), Markdown (2), application integration (3), bounded
worker reordering (1), IDE source cancellation/update (1), and the captured local
coordinate regression (1). The independent consumer exercises Markdown through
its conformance adapter and resources through runtime-owned cancellation.

Minimal dependency trees were inspected: `view` depends only on `view-core`,
and `view-markdown` depends on that minimal facade. `cargo package --list` for
Markdown passed; this is a package-content check, not registry publication or
registry package verification. The full-run transcript and environment metadata
are under `target/verification/`; PowerShell's transcript does not retain every
native command's output, so the tool exit status and returned output were also
checked. Dedicated M4 GPU/native logs retain their adapter and PASS output.

After the final geometry-validation and section-action edits, all five required
workspace gates passed again, followed by all-feature Clippy/tests for the three
M4 packages (`target/verification/m4-final-checks.log`). Both M4 DX12 offscreen
and hidden-native commands passed again (`m4-gpu.log`, `m4-native.log`). The latter
builds reported a filesystem incremental-cache hard-link fallback to copying;
the required Clippy run used `CARGO_INCREMENTAL=0` and passed without warnings.
Affected Markdown local links/fences and `git diff --check` also passed.

Actual DX12 adapter: NVIDIA GeForce RTX 3050 6GB Laptop GPU, driver
32.0.16.1714, PCI 0000:01:00.0. Three offscreen compositions passed non-background
text/asset checks and an analytic selected-gizmo pixel check. Three hidden native
windows each created a surface, rendered and closed, without physical input.
This is correctness evidence, not a frame-rate/startup benchmark.

## Onboarding exercise and corrections

Conditions: implementation author using the repository's public APIs and compiler,
not a blinded fresh reader or independent model. Reference version is 0.1 on the
pinned Rust 1.98.1 toolchain. The eight examples are the version-matched pack.

Observed correction needs (not silently omitted successful-only reporting):

- Invented `TextField::single_line` corrected to `TextInput::new(...).0`.
- Incorrect fallibility assumption on `ScrollState::new` corrected (returns Self).
- Pick point/size were initially passed to `SceneSnapshot::pick`; corrected to
  `PickTracker::request(stamp, point, size)` followed by `scene.pick(request)`.
- Wrong argument order in `Runtime::update` corrected to `(id, dirty, closure)`.
- Assumed default wgpu InstanceDescriptor replaced with its explicit constructor;
  capture API corrected from async `read` to synchronous `readback`.
- A resource lifetime test attempted to unmount a window root; corrected to a
  removable child owner. Roots close through `close_window`.

The quick reference documents actual constructors, update order, effect timing,
features and ownership. Final compiler/behavior checks verify these corrections.
This small exercise does not establish universal AI reliability, comparative API
ergonomics or independent onboarding success. Erasure is one description allocation
per node with bounded tree depth/count, not a zero-allocation claim. Incremental
compile timings and binary sizes depend on profile/features/cache; no invented
cross-machine performance threshold is supplied.

One local dev-profile observation from the reference pack run: `minimal.exe` was
452,096 bytes (PE only, excluding PDB; `controls,resources` features enabled), and
its first observed invocation in the qualification run reported 6.73 seconds of
Cargo build time with warm dependencies and `CARGO_INCREMENTAL=0`. These are
environment-specific observations, not clean-build, release-size or regression
budgets. No generic-versus-erased optimization claim is inferred.

Integration uncovered one behavior failure beyond authoring mistakes: captured
gizmo motion used window coordinates as local coordinates. The runtime mapping
now includes translated/scaled/scrolled ancestors and has an analytic regression
plus the application's captured drag/one-undo test.

## Coverage boundaries

Native proof is Windows/DX12; no Linux/macOS GPU/native execution in this increment.
The storefront uses a bounded fake service, not live HTTP/authentication/downloads.
Clipboard in the proof is memory-only. No physical mouse/keyboard, real IME
candidate/composition, native AT client, visible scanout or production storefront
certification follows from hidden windows. Markdown is a specialized block preview,
not CommonMark/rich text. Existing M0/M2B/M3A native/platform gates stay open.

No new registry dependencies were introduced. New local package boundaries isolate
optional resources/codecs, the separately consumed control and application-only
platform/GPU integration. Lockfiles record those local packages; minimal facade
and Markdown consumers remain free of GPU, native and networking dependencies.
