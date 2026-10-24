# 2D rendering with view-wgpu

Enable `view/wgpu` and use `view::render`, or depend directly on `view-wgpu`.
The minimal facade and core still have no GPU dependency. The compatible backend
family is wgpu 30, glyphon 0.12 and cosmic-text 0.19; use the re-exported `wgpu`
types when sharing resources. Align the Rust crate version as well as the device.

## Preparing and presenting

Prepare the GPU frame before acquiring/reconfiguring a native surface, so uploads
and text preparation do not extend the interval between replacing swapchain
buffers and presenting new content. If acquisition returns `Retry` or
`Reconfigure`, schedule another redraw; do not silently discard the request.
`Suspended` waits for a size/visibility change. The controls gallery follows this
order and sends winit's pre-present notification before presentation.

Create a wgpu instance and request an adapter, passing the native surface as
`compatible_surface` when applicable. `GpuContext::request` checks required
features and limits before requesting a device. Host-owned applications instead
call `GpuContext::from_device` with the paired device/queue and an explicit
`DeviceErrorPolicy`. `HostManaged` preserves host callbacks; forward device loss
with `mark_lost`. `Capture` installs a bounded 64-message sink and loss callback.

Create a `Renderer` for the negotiated attachment format and chosen `Budgets`.
Create a `PaintSnapshot` from committed content, supplying its `Revision`, physical
extent and logical-to-physical `ScaleFactor`. Paint items retain their order.
`prepare` creates an owned `PreparedFrame`; it does not submit or present.

For framework-owned offscreen work, `submit` consumes the frame and returns a
`Submission`. `submit_capture` additionally copies the pixels in that same command
buffer and returns a `Capture`; `readback` consumes it with a bounded 30-second
wait. Later target reuse cannot change that capture's pixels or revision. Readback
returns RGBA, including for BGRA attachments. It is a diagnostic operation, not
the normal composition path.

Host-owned applications call `encode` on their encoder and then `submitted` after
their own queue submission. Do not discard a prepared frame after encoding while
retaining an unsubmitted encoder indefinitely: acknowledge or discard that work.
Call device polling to deliver completion callbacks and release frame slots.
GPU completion, queue presentation and actual monitor scanout are distinct.

Text cache identity follows the immutable `ShapedText` result, including clones
returned by the shaping cache. A newly allocated `Arc<ShapedText>` wrapper does
not create new content. `shares_layout` distinguishes clones from new shaping
results; changed metrics, font collections, or reshaping after eviction retain
distinct identities. This prevents idle repaints from retaining duplicate GPU
text entries. Origin, clip, color, transform, scale and target size still
participate in the renderer cache key.
Retained text draws are also bounded by `Budgets::max_text_entries` (default 256).
Editing or resizing cannot accumulate unlimited presentation keys. Prepared frames
retain evicted draws until submission completes; atlas epoch lifetime is unchanged.

`WindowSurface` coalesces resize requests at acquisition and suspends zero-size
or occluded work. Its acquired frame borrows the surface exclusively, preventing
reconfiguration or a second acquisition until presentation/discard. `present`
submits once and presents once. Timeout/occlusion returns `Retry`; outdated or
suboptimal returns `Reconfigure`; loss requires host surface recreation; validation
returns an error. The host decides when to retry; there is no busy redraw loop.

## Paint and color profile

- Quads, lyon path fills/strokes, images and ordered text use WGSL/wgpu pipelines.
  Mesh vertices use checked bytemuck POD layouts. Shape transforms use core's
  `Transform2D`, followed by the snapshot DPI scale.
- Clips are half-open rectangles in committed window logical coordinates. Raster
  coverage uses pixel centers, including fractional DPI and empty clips. This
  profile does not implement arbitrary path masks or rotated clip masks.
- Paint colors are straight sRGB in `[0,1]`. Shaders blend linear premultiplied
  values into RGBA8/BGRA8 sRGB attachments. Straight-alpha image texels are
  premultiplied before bilinear interpolation. `TextureMetadata` declares alpha,
  transfer and top/bottom origin. sRGB premultiplied storage means sRGB encoding
  of *linear-premultiplied* RGB, not multiplication in encoded sRGB space.
- `Paint::Layer` renders its children into a cleared offscreen texture and applies
  opacity once to their combined output. Overlapping children do not acquire
  repeated opacity. Layers redraw completely; no preserved swapchain content is
  assumed. Image processing, HDR and professional color conversion are outside
  this SDR profile. Geometry edges currently use single-sample rasterization.
- Text uses the exact cosmic buffer and retained font collection behind
  `ShapedText`, including bidi/fallback. `TextVisuals` composes selection, text,
  preedit underline and caret from shared editor geometry. The caller shapes the
  transient IME display value and supplies its preedit range; this helper does not
  mutate committed text or certify native IME behavior. Text accepts positive
  uniform scaling and translation; rotate/skew text through an offscreen image.

## Caches and ownership

Image IDs are context-qualified monotonic content identities. Uploading or
importing another revision yields another ID; eviction makes the old ID fail
with `StaleResource`. Geometry uses full path/stroke keys, and immutable vertex/
index buffers use their complete byte content. Neither cache reuses mutable GPU
storage underneath a prepared frame.

Mesh buffers store transformed physical-pixel positions. Window-size conversion
to clip space happens in the vertex shader using an immutable, frame-owned
16-byte viewport uniform. Size-only changes therefore reuse otherwise unchanged
mesh buffers. DPI, transform, color, UV or actual geometry changes still produce
distinct keys. Each prepared frame owns its projection, including full-size
isolated layers; preparing a later size never mutates an earlier frame.

Sampled-image bindings are owned alongside their immutable textures. Solid draws
share the renderer's white-texture binding, while uploaded/imported images each
reuse their own binding. Prepared frames clone binding handles, preserving
textures after image eviction. Fresh isolated-layer targets get fresh bindings;
there is no unbounded independent binding cache. The cumulative
`CacheStats::sampled_bindings_created` counts these retained bindings, excluding
temporary import-validation, glyphon and frame-viewport bindings.

`Budgets::max_buffer_entries` (default 256) also caps retained vertex/index
buffer pairs. Tiny meshes have driver/resource overhead that their payload-byte
budget cannot measure. Zero disables retention; prepared frames still own their
buffers. `CacheStats::buffer_entries` reports retained pairs, not total live GPU
resources. Hosts must poll GPU completion so submitted resources can retire.
Repeated unchanged-size surface resize requests are ignored; restoring an
occluded surface still schedules configuration.

Glyph atlases are grouped by retained font collection identity. Font reload creates
a new namespace. Labels share an atlas, but live atlas slots are never trimmed or
reassigned. Budget eviction drops an entire epoch; prepared frames retain the old
epoch. Atlas exhaustion clears cached epochs and returns `BudgetExceeded`; retry
the frame against the fresh cache, or reduce the workload if it cannot fit.
The glyph cache reserves a conservative ceiling of `5 * max_texture_dimension_2d²`
bytes per font collection plus glyph buffers, because glyphon does not expose
actual atlas residency. These counters are reservations, not GPU telemetry.

`Budgets` bounds image, geometry, buffer and glyph caches, frame allocations,
draw count, layer nesting and live frame count. Submitted frames retain a slot
until completion. wgpu additionally retains encoded resource handles until GPU
use ends, so dropping/evicting a CPU cache entry never destroys an in-flight
binding. Cached resources plus up to `max_frames` retained snapshots can coexist;
frame limits are per snapshot, not a process-wide memory quota. Application-owned
targets, imported allocations, font data and readback buffers retain their own
ownership. Image-cache accounting includes imported base-level pixels even though
the host may also retain the texture. Callers must bound their own target/capture
collections. There is no claimed driver allocation or memory-pressure measurement.

## Contributions and recovery

`validate_passes` checks unique pass IDs, imports, one writer per resource,
explicit transitive read dependencies, unknown resources and cycles, returning
the execution order. Resource IDs are local to that graph. Actual image/context
handles are checked again during preparation/encoding. A frame cannot sample its
own paint target. The host executes its provider passes in the validated order.

`ProviderSession` coordinates negotiate, prepare, scoped encode, submission,
completion retirement, resize and recreation. Offscreen providers render a resolved
target; import it with `Renderer::import_texture`, then compose it as an image.
That import checks the epoch, real wgpu device ownership under an error scope,
usage, format/transfer, dimension, layer/mip count and sample count. Multisample
providers resolve externally before import. No CPU copy is required.

The optional [basic scene provider](scene.md) requests a private Depth32Float
attachment for offscreen encoding. The session clears depth to 1 each pass and
rebuilds it on extent/device changes. Providers without that request retain the
color-only profile. `provider_mut` is available only after outstanding work is
discarded or retired; attachment requirements stay fixed for the session.

Direct contributions receive a fresh compatible render pass with `Load`, at the
host's explicit ordering point. Single-sample SDR attachments are the supported
profile; isolation selects the offscreen fallback. Providers establish all their
own state and must not submit/present behind the host's back. A session rejects
duplicate/out-of-order submission acknowledgements, blocks resize while encoded
work awaits submission, and limits outstanding submissions to three. The host
must call `submitted` only after submitting that contribution's encoder.

Imported content must remain stable through readers' completion. Publishing a
new content ID does not make concurrent writes to the same wgpu texture safe.
Queue order and revisions remain the producer's responsibility. Unrelated native
devices, depth/MSAA direct passes and arbitrary external-memory sharing are not
supported by this profile.

Device replacement creates a new context generation. Old targets, prepared frames
and images reject it. Create a new renderer and recreate providers; ordinary
surface loss requires creating a new surface with the native owner. Provider
prepare/encode failures call `failed`; recreation of outstanding work reports its
invalidation. Callback failure or device loss does not promise successful recovery.

## Reproducible fixtures

For staged release-build working-set/private-memory measurements on hardware
DX12, see the [GPU memory guide](gpu-memory.md).

```text
cargo test -p view-wgpu --locked
cargo run -p view-wgpu --example offscreen --locked -- dx12
cargo run -p view-wgpu --example surface --locked -- --run-hidden dx12
cargo run -p view-controls --features gallery --example gallery --locked -- --resize-soak dx12
pwsh -File scripts/verify.ps1 -Gpu -Backend dx12
```

Use `vulkan` or `metal` only on a provisioned compatible host. The offscreen fixture
writes `target/verification/m3-offscreen.ppm`, compares analytic pixels with a
2/255 channel tolerance and prints actual adapter/backend details. The surface
fixture is explicit opt-in, creates a hidden nonactive window and never injects
input. See [M3A evidence](../plan/evidence/m3a-renderer.md) for actual coverage.
