# Qualification fixtures

`manifest.json` records origin, license, version and SHA-256 over UTF-8 text with
LF line endings, or exact bytes for `sha256Binary` entries. Run `pwsh -File scripts/check-fixtures.ps1` from the root. A
content change requires an intentional manifest/version update and review.

- `text/graphemes.txt`: original combining-mark, ZWJ-emoji and flag sequence;
  expected segmentation is independently specified in the qualification test.
- `shaders/sequence.wgsl`: original deterministic compute fixture producing
  `[7, 10, 13, 16]`; no clocks, external files, network or random seed.

Both are first-party MIT OR Apache-2.0 material. `fonts/` contains unmodified
Noto Sans, Noto Sans Hebrew and Noto Sans Arabic from the qualified cosmic-text
0.19.0 crate archive. These Google-copyright fonts are SIL OFL 1.1; the accompanying
`NotoSans-LICENSE` is retained and hashed. They are owned shaping inputs rather
than installed-font goldens. No emoji font is bundled; missing glyphs are observable.
No images or user/network recordings are bundled.

Packages that compile fixtures with `include_bytes!`/`include_str!` carry only the
referenced fixture copies under their own `fixtures/` directory, so Cargo archives
do not depend on repository-relative files. Each package's fixture manifest records
the canonical path and SHA-256; `scripts/qualify-release.ps1` verifies both source
and copy hashes and checks archive inclusion. Font copies retain `NotoSans-LICENSE`.
The root fixture manifest remains the provenance and license source of truth.

The optional scene GPU example (`crates/view-wgpu/examples/scene.rs`) constructs
original analytic quad meshes and fixed camera/material values in source. It uses
no asset files, randomness or installed resources; first-party MIT OR Apache-2.0
applies. Expected pixels are analytic assertions, not downloaded image goldens.

`crates/view-wgpu/examples/embedded.rs` and its `support/brush.rs` use original
analytic scene/host shaders and bounded generated 16×16 brush tiles. Their fixed
12-tick clock and pixel assertions require no assets, randomness, personal data
or native input. Generated captures/logs stay under `target/verification`.
