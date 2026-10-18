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
