# 2D/3D rendering and interoperability

Status: selected rendering baseline P-04/P-05. First-class 2D/3D support is requirement R-07. Revision: 0.4.

## Rendering roles

| Role | Responsibilities | Can be replaced/hosted? |
| --- | --- | --- |
| UI painter | Text, primitives, images, clipping, layer composition | Custom primitives and paint providers |
| 2D canvas | Document/view transforms, retained or immediate draws, hit regions | Application owns its document/processing graph |
| 3D viewport | Camera, render contribution, picking, overlays, frame demand | Application renderer or first-party scene provider |
| Frame compositor | Ordered contributions, attachment compatibility, presentation | Host-owned presentation supported |

UI paint order, 2D layer order, and 3D depth testing are separate concepts. A perspective scene has its own depth attachment. UI overlays normally compose after the scene; an explicit depth-aware overlay is a different rendering contract.

## Two supported ownership modes

**view-owned application:** view creates platform windows, selects a device, manages surfaces, schedules frames, and accepts scene/canvas contributions. Application rendering shares the agreed device and queue.

**Host-owned application:** an engine/tool owns its loop and GPU device. It supplies platform events, timing, render-target information, and a compatible device/queue. view prepares UI contributions; the host owns submission and presentation. It must also supply or opt into native services and accessibility integration.

There is one presentation authority per surface. A host may supply an offscreen target instead of a window. The architecture must not require an application to start a second event loop or secretly create a second GPU device.

Sharing Rust wgpu objects also requires a compatible crate/API version, not merely the same physical adapter. Publish a backend compatibility policy and isolate wgpu-specific APIs in the rendering adapter. A renderer built against an incompatible wgpu version needs a supported bridge or an aligned dependency; the core abstraction cannot erase this constraint automatically.

wgpu resources and operations are device-scoped. Device and surface contracts must be followed at this boundary. [Device](https://docs.rs/wgpu/latest/wgpu/struct.Device.html), [Surface](https://docs.rs/wgpu/latest/wgpu/struct.Surface.html)

## Render contribution contract

A provider declares its capabilities and follows these conceptual phases:

1. **Negotiate:** features, limits, formats, sample counts, target size, color space, and ownership mode.
2. **Prepare:** consume a coherent scene/document revision, perform allowed uploads, and establish dependencies.
3. **Encode:** record passes against assigned resources using explicit read/write declarations.
4. **Compose:** expose a resolved output or a compatible direct-pass contribution at a defined layer position.
5. **Complete/retire:** observe submission progress when needed; release/reuse transient resources safely.
6. **Resize/suspend/recreate:** rebuild device- or target-dependent state and report unavailable content.

This is a lifecycle contract, not a finalized Rust trait. User rendering must not submit to a framework-owned surface behind the scheduler's back. Submission ownership and queue ordering are negotiated; uploads cannot race in-flight readers through untracked reuse.

Start with a small pass-dependency representation and validation. An engine may keep its internal render graph; view need only understand its external dependencies. Cycles, multiple undeclared writers, incompatible attachments, and stale resource handles produce diagnostics.

## Composition paths

**Offscreen texture contribution:** the default proposal for 3D scenes and complex effects. It allows clipping, scaling, overlays, independent refresh, and predictable isolation without CPU readback. It does incur a render-target allocation and composition/bandwidth cost.

**Direct contribution:** an opt-in advanced path for compatible attachments and ordering, useful when profiling justifies avoiding an intermediate. The provider restores no assumed global state: each encoder/pass establishes what it needs. Complex masks, filters, or isolated group opacity may require the offscreen path.

**Existing texture contribution:** supported when the texture belongs to a compatible device and has suitable usages, format, size, and synchronization. The descriptor records alpha convention, transfer/color interpretation, origin, sample count, and content revision. Multisampled output must be resolved before ordinary sampling. [wgpu textures](https://docs.rs/wgpu/latest/wgpu/struct.Texture.html)

Zero-copy means avoiding a required CPU copy, not zero bandwidth, zero synchronization, or free composition. An unrelated D3D/Vulkan/Metal device cannot be assumed to share a wgpu texture. Native external-memory interop is a future, backend-specific capability with explicit support tests; it is not promised as portable core functionality.

## First-party 2D scope proposal

Provide transforms, paths, fills/strokes, text, images, clipping, layer opacity, pan/zoom, hit regions, and overlays. Use lyon tessellation and the selected cosmic-text/glyphon stack, qualified together with wgpu. Rectangles alone are insufficient for the intended tools.

Design for tiled image/document content, async processing, dirty regions, and bounded texture caches. Image decoding, filtering, brush engines, and document formats are modules or application systems, not mandatory core dependencies. Preview and final-quality rendering may run at different cadences.

The initial color baseline is explicitly defined SDR composition with documented linear/premultiplied operations. Professional color management, wide gamut, HDR, high-bit-depth editing, and export pipelines are deferred beyond this release; they are not implied by the phrase “Photoshop-like.” Preserve color metadata at the resource boundary now.

Keep document coordinates independent of viewport pixels. Large documents may need higher-precision coordinates and rebasing before GPU conversion; do not force every application model to store positions as screen-space f32 values. The exact public geometry types are a design evaluation item.

## First-party 3D scope proposal

A reference module should provide a camera, mesh/material submission, transforms, depth-tested rendering, picking, and UI/gizmo overlays. The first proof can use a simple material model; production PBR, shadows, skeletal animation, asset formats, and physics are separate feature decisions.

No mandatory ECS or universal scene graph. A provider can receive an application snapshot or hold versioned scene handles. Projection, handedness, clip/depth convention, world units, and viewport transforms are explicit in the first-party module and adapters.

Picking may use CPU queries or GPU IDs. GPU readback is asynchronous; a result includes scene/camera/viewport revisions so stale picks do not select the wrong object. Dragging remains responsive while a pick is pending. Semantic scene actions and named objects allow automation without requiring pixel-perfect coordinates.

## Caching and lifetime

Cache paint commands, geometry, glyph rasters, atlas allocations, GPU buffers, and expensive layers independently. Cache keys include the revisions and device generations that affect validity. Atlas eviction or repacking invalidates references; indices cannot silently point to a new glyph.

CPU ownership and GPU completion are separate. Use deferred retirement or an equivalent submission-aware allocator. Pools have budgets, eviction rules, and instrumentation. Resize storms are coalesced; zero-sized/minimized surfaces do not allocate invalid targets.

Initially redraw the visible composition when presentation is requested while reusing valid caches. Do not depend on previous swapchain contents. Damage-based redraw is a later optimization requiring preserved backing storage and correct overlap/effect bounds.

## Performance evidence

Measure static UI beside an animated scene, multiple viewports, transformed/clipped canvases, large textures, mixed DPI, and sustained resizing. Report UI CPU time separately from scene CPU/GPU time and composition overhead. Establish budgets only after selecting representative hardware and workloads.

Track p50/p95/p99 frame and interaction times, idle work, allocations, bytes uploaded, draw/pass counts, resource residency, and recovery time. Claims about 60/120 Hz are acceptance targets to be measured, not established properties of the architecture.
