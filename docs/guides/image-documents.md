# Bounded SDR image documents

`view_wgpu::document` is an optional CPU helper for small SDR image workflows.
It does not replace the hardware renderer, attach a document to the runtime tree,
or perform work during build/layout. The application owns its document and calls
its methods from an application action or worker boundary as appropriate.

`ImageDocument` stores sparse 16×16 tiles of straight-alpha, 8-bit sRGB RGBA.
`ImageBudget` bounds the image pixel count, tile count, undo history count and
bytes, and export size. `set_pixel`, clipped `apply`, `composite`, and `box_blur`
calls commit atomically as a single undo operation. An operation that produces
no changed pixels adds no undo entry. `Invert`, encoded-channel BT.709 grayscale,
and fixed-point `Exposure` operate on encoded bytes; they are not a color-managed
processing graph. Failed budget checks preserve document and history state.

`composite` accepts a borrowed tightly packed `RgbaImage`, a signed `(x, y)`
origin and an optional borrowed `AlphaMask`. It validates exact byte lengths and
matching source/mask dimensions before reading them. Negative placement and
right/bottom overhang clip source and mask together without rescaling. Mask
coverage is linear 0–255 and multiplies source alpha. The operation decodes sRGB
color to linear light, premultiplies, applies source-over, then unpremultiplies
and encodes the result back to straight-alpha sRGB RGBA8. For example, half white
over opaque black produces `[188, 188, 188, 255]`, rather than an encoded-channel
average. Zero effective source alpha preserves destination bytes, including
hidden color under zero alpha. Input slices remain application-owned and are
never retained in history.

`box_blur` supplies a separable box filter with an explicit radius of 0–32.
It averages linear-light premultiplied color and alpha, so hidden color in
transparent source pixels does not introduce colored fringes. It samples the
unchanged document through a halo around the output rectangle; only writes are
clipped to that rectangle. At document boundaries, it replicates the nearest
edge pixel. An optional mask matches the original requested rectangle before
clipping and interpolates the original and filtered premultiplied values.
Coverage zero preserves exact original bytes; processed results whose rounded
alpha becomes zero use transparent black. Radius zero is a no-op. This is a
bounded neighborhood effect, not Gaussian blur or a general effect graph.

`ImageOperationBudget` independently limits source bytes (including borrowed
mask/source data), temporary payload and work-region pixels. Blur source/work
accounting includes its clipped sampling halo; its scratch accounting includes
the horizontal floating-point intermediate, staged tiles, undo clones, and a
conservative per-tile metadata allowance. Limits and checked arithmetic are
evaluated before expensive allocation. History capacity is conservatively
preflighted against every allocated tile intersecting the output, so a tight
budget can reject an operation even if a mask would leave many tiles unchanged.
The final tile/history commit is checked again before mutation. The application
may supply larger processing limits explicitly; the radius ceiling stays 32.
`apply` uses the default operation limits; `apply_with_budget` accepts explicit
limits for the encoded-channel point effects. These bounds are payload/accounting
limits, not measurements of allocator or process working-set overhead.

The source/mask and effect contracts are compiled as public-API integration
tests. They cover nonuniform coverage and signed clipping, linear-light
composition, transparent-color rejection, two-dimensional blur support,
edge replication, exact undo, zero extents, overflow/pathological inputs, and
atomic rejection for each processing/storage budget. Executed checks and coverage
limits are recorded in the [F06 evidence](../plan/evidence/f06-masked-image-effects.md).

`encode_bmp` emits top-down 32-bit BMP V4 with explicit alpha masks and the sRGB
color-space marker. `encode_rgba8` emits the documented `VWRGBA\0\x01` header,
little-endian dimensions, and row-major RGBA bytes. Both are lossless byte
exports and enforce `max_export_bytes`; the caller remains responsible for file
selection and I/O. No image codec dependency is added.

`PenSample::validate` rejects non-finite or out-of-range document positions,
pressure, and tilt values. Native pressure and tilt capability flags remain
false: validation does not establish tablet event translation or hardware
certification. The existing WGPU renderer and GPU presentation path are
unchanged. Professional color management, wide gamut, HDR, high bit depth,
general effect graphs, arbitrary convolution kernels, and broad image formats
remain outside this bounded SDR helper. Floating-point processing is quantized
back to RGBA8 per transaction; no cross-platform bit-identical floating-point
or hardware/tablet certification is claimed.

Run the CPU contracts with:

```text
cargo test -p view-wgpu --test document_contracts --test masked_document_contracts --locked
```
