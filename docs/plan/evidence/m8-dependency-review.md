# Dependency security and maintenance review

The local audit used cargo-audit 0.22.2 installed under `target/tools`, independently
of the application's dependency graph. RustSec database commit
`21d909e4c8bd4e14c7aea58bf980c881a0053ab1` was current through 2026-10-24.
The 328-package lockfile scan reported no vulnerability-class entries, but did
report unsoundness and maintenance advisories. These are not a clean bill of health.

- RUSTSEC-2026-0386: Tokio's Unix AsyncFd ownership contract. Updated the workspace
  lockfile from 1.53.2 to patched 1.53.3. Windows execution alone cannot verify the
  Unix API fix; upstream regression evidence and renewed OS CI remain relevant.
- RUSTSEC-2026-0253: lru 0.16.4 through glyphon 0.12.0 is not panic-safe when
  dropping a key panics during pop. Glyphon's cache key is `GlyphonCacheKey`, a
  Copy enum with no Drop implementation; this usage cannot invoke the described
  panicking-key destructor. The advisory remains in the dependency inventory.
  Migrating glyphon's semver-incompatible lru dependency requires upstream or
  separately qualified renderer changes; it is not silently suppressed.
- RUSTSEC-2026-0192: ttf-parser 0.25.1 is unmaintained. Its replacement is a text
  stack qualification task, including shaping, metrics, font parsing and pixels.
  Retain this explicit maintenance risk; an unrelated version bump does not fix it.

Reproduce with `target/tools/bin/cargo-audit audit --db target/advisory-db --json`.
The retained result is `target/verification/dependency-audit.json`. Tool installation
and public advisory retrieval did not upload source or diagnostics. Review does
not prove all dependencies safe or replace parser/protocol/unsafe-boundary tests.

Refreshed on 2026-10-26 against the final lockfile: the public database still
reported the same commit. The scan retained zero vulnerability-class entries and
the two documented warnings (`lru` unsoundness and `ttf-parser` maintenance).
The Tokio warning was absent after the patch/minimum-version update. The earlier
receipt remains at `target/verification/dependency-audit-2026-10-25.json`.
