# M3A 2D renderer evidence

Scope: section 4, M3A-01–M3A-14, on 2026-10-08. M3B 3D/canvas/host proofs and M2B
standard-control certification remain separate work. This report supports the
[canonical checklist](../implementation-checklist.md), not a second progress board.

## Implementation and acceptance mapping

| Items | Implementation / evidence |
| --- | --- |
| M3A-01 | Optional `view-wgpu` package and facade feature; feature/limit/format negotiation, explicit error policy, retained device generation; unsupported target/limit and foreign-device fixtures |
| M3A-02 | Borrowed acquired surface frames, coalescing, suspension, single submit/present; real hidden-window DX12/Vulkan fixture |
| M3A-03–M3A-04 | Revision-tagged owned preparation, checked 36-byte vertex layout, ordered WGSL draws, atomic render/capture commands; captured pixels survive target overwrite |
| M3A-05 | lyon fill/stroke, core transforms and DPI, half-open rectangular clips; analytic triangle, stroke at 2× scale and fractional clip pixels |
| M3A-06 | Same cosmic buffer/font collection used for glyphon; shared selection/text/preedit/caret geometry and ordering; font reload retention and glyph raster fixtures |
| M3A-07 | Linear premultiplied SDR, explicit image metadata, alpha-correct filtering, offscreen group opacity; independent 188/255 half-intensity and 137/188/192 filtered-alpha expectations |
| M3A-08 | Bounded image/geometry/buffer/glyph caches and live frame slots, whole atlas epoch eviction and wgpu-retained encoded resources; image/glyph eviction, cache hits, prepared-frame retention, 100 labels sharing one font atlas |
| M3A-09–M3A-11 | Pass DAG diagnostics; provider negotiation, scoped direct/offscreen draws, existing-texture composition, real device ownership validation, completion retirement and resize/recreation callbacks; provider green triangle is sampled into UI output |
| M3A-12 | Stale generation rejection, bounded error/loss reporting, explicit provider recreation and surface failure policy; zero/occlusion/restore lifecycle tests; no hardware-reset recovery certification |
| M3A-13 | Real offscreen rendering/readback with owned OFL font fixture and 2/255 tolerance; failure paths reject unsupported limits/formats, stale images, foreign devices, target feedback and excess prepared frames |
| M3A-14 | Windows hardware execution only; Linux/macOS GPU execution remains uncovered and the checklist gate stays open |

The CPU contracts exercise pass graphs (cycles, duplicate writers, missing
dependencies/imports), surface state, direct fallback, SDR conversion and shared
editor paint ordering/font namespaces. The independent consumer compiles and uses
the public facade renderer profile without root workspace feature unification.

## Environment and verification

Local host: Windows x64, OS build 10.0.26300 (Windows 11), rustc 1.98.1
`48a229cea`, LLVM 22.1.8. Hardware: NVIDIA GeForce RTX 3050 6GB Laptop GPU,
PCI 0000:01:00.0. DX12 reports driver `32.0.16.1714`; Vulkan reports NVIDIA
`617.14`. Device type is `DiscreteGpu`, not a software adapter.

Final verification passed on this local working tree:

- `pwsh -File scripts/verify.ps1 -Gpu -Backend dx12` — exit 0. Includes all five
  required root Cargo checks, optional text/platform features, renderer CPU tests,
  independent minimal/harness/text/shaping/wgpu/all-feature consumers, owned fixture
  hashes, dependency inventory, existing hidden platform checks, DX12 compute,
  M3 offscreen pixels and the hidden DX12 surface fixture.
- `cargo run -p view-wgpu --example offscreen --locked -- vulkan` — exit 0,
  same analytic pixel assertions on the Windows NVIDIA Vulkan backend.
- `cargo run -p view-wgpu --example surface --locked -- --run-hidden vulkan` —
  exit 0, hidden native acquisition/presentation/suspension/resize.
- Affected Markdown local links and code fences plus `git diff --check` passed.

There are five renderer CPU contract tests. The all-feature independent consumer
passes six tests, including its renderer case. GPU fixtures are explicit opt-in
executables, not silently skipped tests. The offscreen test additionally rejects
same-target sampling before encoding and checks premultiplication before filtering.

[Recorded environment](m3a-environment.json) preserves the local compiler/OS,
adapter/backend and execution classification. Generated logs, dependency inventory
and PPM capture live in `target/verification/`; they are not GPU benchmarks.

No Linux/macOS GPU host or remote CI execution was available in this request.
Existing workflow dispatch uses explicitly provisioned runners and now runs the
M3 fixture through `scripts/verify.ps1 -Gpu`. No runner was provisioned and no
workflow was dispatched, branch pushed or package published.

T-07 is covered at the 2D contribution/clip/composition layer; full 3D depth/overlay
proof remains M3B. T-10 covers cache retention/invalidation. T-11 covers logical
DPI conversion in pixels, not moving a visible window between monitors or native
IME placement. T-12 covers zero-size/occlusion/reconfiguration and generation
invalidation, not actual driver removal, sleep/resume or successful recovery from
every surface/device failure. Hidden queue presentation does not prove scanout.

## Dependencies and limitations

The root lockfile retains the already-qualified registry versions: wgpu 30.0.1,
glyphon 0.12.0, cosmic-text 0.19.0, lyon 1.0.19 and bytemuck 1.25.2. No new registry
family is selected. wgpu owns GPU validation/backend lifetimes; glyphon owns glyph
raster/atlas integration; cosmic-text owns shaping; lyon owns tessellation;
bytemuck derives checked host layouts. The independently resolved consumer adds
102 locked packages for this optional rendering graph. The root Windows
qualification inventory contains 124 registry packages; this is not the minimal
facade dependency count. Glyphon enables cosmic-text's upstream default features.

License metadata: wgpu/cosmic-text/lyon are MIT OR Apache-2.0; glyphon is
MIT OR Apache-2.0 OR Zlib; bytemuck is Zlib OR Apache-2.0 OR MIT. Existing M0
qualification and the retained upstream package sources establish the selected
maintained family; this change does not claim a new vulnerability audit. Native
backend features are DX12/Vulkan/Metal. Linux example winit backend features are
declared directly. Cross-platform execution of this revision remains separate.

The [rendering guide](../../guides/rendering.md) records the supported rectangular
clip, single-sample SDR, text transform and provider profiles. Glyph memory is
conservatively reserved, not queried from the driver. Application-owned targets,
captures and imported texture writes require host lifecycle discipline. There are
no throughput/latency claims, path-mask certification, native IME certification or
full-control conformance claims.

## Idle text cache retention correction (2026-10-22)

Review Desk exposed continuous private-memory growth during repeated native
repaints. Controls wrap cached `ShapedText` clones in fresh `Arc`s. The renderer
previously compared those wrapper addresses, retaining a new GPU text renderer,
viewport and shaped layout on every repaint until its large cache budget filled.

`ShapedText` now carries immutable clone-stable identity. Renderer reuse compares
that identity plus the existing presentation key. New shaping results (including
reshaping after eviction) remain distinct, preserving font and geometry validity.
No dependency or unsafe-code change is introduced.

Verification:

- The shaping test checks cache-hit/clone identity and isolation of changed metrics
  and newly shaped results after eviction.
- The actual DX12 offscreen fixture repaints 100 fresh wrappers around the same
  shaped result: 100 additional glyph-cache hits and unchanged glyph accounting.
- An app-specific 400-frame hidden-window reproduction measured private bytes
  rising from about 490 MiB to 1,435 MiB over 20 seconds before the correction.
  After correction, the same run stayed around 485 MiB (485.2–485.6 MiB after
  warm-up). These are local functional diagnostics, not portable benchmarks or a
  proof that all memory growth is eliminated.
- The uncommitted demo also avoids unchanged-size surface reconfiguration, polls
  GPU completion, and supersedes its named process-owner lease on restart. Removing
  redundant reconfiguration alone did not fix the measured text retention.

Local raw samples are in ignored `work/review-desk/target/memory-before.csv`,
`memory-after.csv` (surface/polling change only), and `memory-fixed.csv`.
Linux/macOS graphics execution and M3A-14 remain open.

The 2026-10-23 correction also adds max_text_entries (default 256), reported as text_entries in CacheStats. The DX12 fixture churns 100 presentation keys with a two-entry limit, then renders a frame retained before eviction and verifies identical pixels. It passed locally. scripts/verify.ps1 passed after these framework changes, including formatting, workspace check/clippy/tests/docs and supported-profile/consumer checks. This does not add Linux/macOS GPU coverage.

## Resize buffer retention correction (2026-10-23)

The GPU controls gallery reproduced excessive retention during resize. Its
normalized vertex keys change with window dimensions. A payload-only 16 MiB
buffer budget retained thousands of small vertex/index pairs, whose actual
driver allocation overhead was much larger. `max_buffer_entries` now defaults
to 256 independently of the byte budget; zero disables retention. Prepared
frames retain their buffers after cache eviction. SurfaceState suppresses
unchanged-size configuration and the gallery polls GPU completion each draw.

The opt-in hidden fixture runs 1,000 native size requests and draws over 400
distinct dimensions, without injecting physical input:

```text
cargo run -p view-controls --features gallery --example gallery --locked -- --resize-soak dx12
```

Local Windows/NVIDIA RTX 3050 DX12 debug-build samples at approximately 500 ms:

| Measurement | Before | After |
| --- | --- | --- |
| Peak private memory | 3,065.02 MiB | 498.51 MiB |
| Buffer payload/key bytes after 400 cycles | 5,916,800 | 88,064 |
| Retained buffer entries | not instrumented | 256 |

After warm-up (10 seconds), final-run private memory was 497.27–498.51 MiB.
The final 1,000-cycle fixture exited successfully with no reported GPU validation
errors. The baseline and first corrected diagnostic completed the cycles but
hit a harness-only extra-idle shutdown error; the final fixture fixes that guard.
Raw local logs/samples are ignored under `target/verification/gallery-resize-*`.
The final run overlapped compilation, so working set and elapsed time are not
comparable performance measurements. Private bytes are process commitments,
not physical RAM or GPU-resident bytes. This finite soak does not establish a
30 MiB baseline, indefinite stability, physical drag behavior or other platforms.

Regression coverage includes 1,000 distinct geometry keys with zero/two-entry
limits and pixel equality for a frame held across eviction, plus duplicate
resize and occlusion-resume state tests.

`scripts/verify.ps1 -Gpu -Backend dx12` exited 0 after this correction:
formatting, workspace check/clippy/tests/docs, supported feature profiles,
independent consumers, DX12 offscreen eviction/pixel regressions, scene fixtures,
gallery captures at 100/150/200%, and hidden native surface checks passed.
Log: ignored `target/resize-memory-verification.log`. Changed rendering
documentation links/fences and `git diff --check` also passed. Linux/macOS GPU
execution and physical desktop interaction remain unverified in this run.

## Size-independent mesh projection (2026-10-23)

The follow-up resize-latency fix stores physical-pixel positions in cached meshes
and performs viewport projection in the vertex shader. Each prepared frame owns
an immutable 16-byte viewport uniform, shared by its full-size isolated layers.
The gallery renders on native resize delivery using the current client size,
instead of waiting for a separate redraw event. Accessibility geometry still
updates on resize; skipping those updates would leave incorrect native bounds.

The DX12 offscreen regression passed 1,000 distinct sizes with one retained mesh
and 1,001 cache hits (including the initial comparison frame). It checks fixed
pixel dimensions in a larger target, then churns actual geometry to force cache
eviction and verifies the original prepared frame's pixels. Zero-retention mode
and existing transforms, DPI, text/mesh ordering and isolated layers also pass.

The hidden native 1,000-resize fixture exited 0. Gallery buffer entries stayed at
43, with 14,792 payload/key bytes and 85,957 hits, compared with the preceding
256-entry/88,064-byte cache. Native resize callbacks now draw in addition to the
fixture's explicit draws; elapsed times are not an equivalent-work benchmark.
Local debug private commitment peaked at 499.02 MiB, with approximately 498.7 MiB
near completion. This optimization removes buffer churn, not the GPU baseline.
Logs/samples: ignored `target/verification/gallery-resize-projection.*`.
Compilation overlapped the soak; no drag-latency or visible-stretch elimination
claim is made from these hidden tests.

Full `scripts/verify.ps1 -Gpu -Backend dx12` exited 0 after this change, including
workspace format/check/clippy/tests/docs, feature profiles, independent consumers,
DX12 offscreen/scene/application fixtures, gallery DPI captures and hidden native
surface checks. Log: ignored `target/resize-projection-verification.log`.
Changed documentation links/fences and `git diff --check` passed. No new
dependency, unsafe code or Linux/macOS execution claim is introduced.

## Shrink presentation follow-up (2026-10-23)

The owner still observed a blank client area when shrinking. The gallery prepared
GPU resources after surface acquisition/reconfiguration and ignored recoverable
acquisition results. Preparation now precedes acquisition, minimizing work after
swapchain replacement. Retry/reconfigure results schedule another redraw, and
successful presentation sends winit's pre-present notification. These address
identified scheduling gaps; hidden tests cannot prove elimination of a visible
compositor flash. The 1,000-step native soak now uses triangular size changes,
with repeated growth and shrinkage, instead of mostly increasing dimensions.

Verification passed: workspace fmt/check/clippy/tests/docs, controls all-feature
clippy, gallery behavior tests, and the revised DX12 1,000-step native grow/shrink
soak (exit 0, no reported GPU validation errors). Buffer entries remained 43;
private commitment near completion was 498.27 MiB. Logs are ignored under
`target/shrink-resize-verification.log` and
`target/verification/gallery-grow-shrink.*`. Documentation links/fences and diff
whitespace checks passed. Visible shrink/scanout remains unverified.

## Profile-guided resource reuse (2026-10-24)

AutoProfile analysis of the owner's `viewProfile.etl` identified sampled CPU
stacks involving DX12 bind-group creation and cosmic-text BufferLine cloning.
Current-source inspection confirmed per-draw sampled-image binding creation and
deep-cloned layouts even on shaping-cache hits. Inclusive stack percentages
overlap; this trace is not a before/after benchmark or allocation-lifetime proof.

`ShapedText` now owns an `Arc<ShapedData>` containing the immutable cosmic buffer,
text, caret/selection geometry and font references. Initial shaping moves its
buffer into that allocation instead of cloning it. Cache hits and clones share
it; pointer identity replaces the separate identity token. New shape results and
font revisions remain distinct, and retained layouts survive cache eviction.

The renderer retains sampled-image bindings with the white texture and each
uploaded/imported image. Prepared frames clone the handles. Image eviction drops
the cache's ownership without changing prepared pixels. Isolated layers retain
their own fresh binding because each owns a fresh target. No standalone binding
cache, dependency, unsafe code or new platform-specific branch is added.

Regression coverage checks shared raster-buffer addresses after cache hits,
cloning and backend destruction; independent results after reshaping; one
sampled binding across 1,000 size changes for solid geometry; and 100 mixed
image/text/layer preparations allocating only the required layer binding each
time. Existing eviction/readback checks verify retained image pixels and stale
image-ID rejection. Editing, navigation and font reload tests remain applicable.

Verification passed on Windows/DX12: targeted shaping tests and offscreen pixel
regressions; full `scripts/verify.ps1 -Gpu -Backend dx12` (exit 0), including
workspace fmt/check/clippy/tests/docs, feature profiles, independent consumers,
scene/application fixtures, gallery DPI captures and hidden native surfaces.
The release gallery was rebuilt and passed its 1,000-cycle native grow/shrink
soak (exit 0), retaining 43 mesh entries and creating just one retained
sampled-image binding throughout. Glyphon/viewport bindings are separate from
this counter. No before/after CPU or memory percentage improvement is claimed
without a comparable new profile.

Local logs: ignored `target/profile-optimization-verification.log` and
`target/verification/profile-optimized-release-soak*.log`. Changed documentation
links/fences and diff whitespace checks passed. Linux/macOS execution remains
outside this local run; no physical-input/scanout claim is added.
