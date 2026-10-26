# F06 bounded SDR masks and neighborhood effects

Local execution: Windows x64, 2026-10-26. This report covers application-owned
image processing in `view_wgpu::document`; it does not certify native tablet
input, GPU filtering, hardware color output, HDR, or Linux/macOS execution.
The [image document guide](../../guides/image-documents.md) defines the public
profile and resource accounting.

The helper now accepts borrowed, tightly packed RGBA8 sources and coverage
masks, with signed/clipped placement and linear-light premultiplied source-over.
A separable radius-0–32 box blur reads an immutable document through a halo,
replicates document edges, and optionally mixes its result through a coverage
mask. Storage and export stay straight-alpha sRGB RGBA8. Legacy point effects
remain explicitly encoded-channel operations.

Before processing allocations, checked arithmetic and operation budgets bound
borrowed source/mask bytes, work pixels, floating-point intermediates and staged
tile/undo payload. History preflight is conservative across intersected allocated
tiles. The final commit checks tile/history limits and publishes one undoable
transaction; rejection preserves pixels, tile allocation and history. Existing
point-effect calls use the same processing preflight, with an explicit-budget
variant available. No dependency, image codec, runtime document ownership or
software rendering backend was added.

Executed and passed:

```text
cargo fmt -p view-wgpu -- --check
cargo test -p view-wgpu --test document_contracts --test masked_document_contracts --locked
cargo clippy -p view-wgpu --lib --test document_contracts --test masked_document_contracts --locked -- -D warnings
```

The two integration suites pass 4 existing and 10 mask/effect tests. Behavior
evidence includes nonuniform masks and source-coordinate clipping; linear-light
source-over with partial alpha; transparent hidden-color rejection; 2D impulse
support and edge replication; clipped blur halo reads; masked premultiplied
interpolation; cross-tile allocation and exact single-step undo; and atomic
source/scratch/work/history/tile budget rejection. A separate direct 3×3 scalar
reference checks a nonuniform 2D color/alpha fixture with an oversized clipped
mask rectangle, including the bottom/right corner. Zero-sized output regions,
maximum-radius one-pixel images, extreme signed origins, malformed lengths and
enormous sparse extents are covered.

Cargo reported incremental-cache hard-link fallback warnings and copied files;
all listed commands exited successfully. Full-workspace and independent-consumer
qualification is tracked by the parent expansion verification, not claimed by
these focused checks. Processing quantizes every transaction to RGBA8 and does
not promise cross-platform bit-identical floating-point results. Professional
color management, wide gamut, high bit depth, arbitrary kernels/effect graphs
and broad image format support remain outside this profile.

Local relative links and Markdown code fences passed checks in this report and
the affected image document guide.
