# Qualification fixtures

`manifest.json` records origin, license, version and SHA-256 over UTF-8 text with
LF line endings. Run `pwsh -File scripts/check-fixtures.ps1` from the root. A
content change requires an intentional manifest/version update and review.

- `text/graphemes.txt`: original combining-mark, ZWJ-emoji and flag sequence;
  expected segmentation is independently specified in the qualification test.
- `shaders/sequence.wgsl`: original deterministic compute fixture producing
  `[7, 10, 13, 16]`; no clocks, external files, network or random seed.

Both are first-party MIT OR Apache-2.0 material. No fonts, images, captured user
data, protocol recordings or network recordings are bundled yet. Introduce those
with their owning milestones, provenance and applicable licenses before use.
System-installed fonts are not portable golden-test fixtures. No font rendering,
network or protocol behavior is certified by the current inventory.
