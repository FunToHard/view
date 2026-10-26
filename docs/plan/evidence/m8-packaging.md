# Local package qualification

On 2026-10-25, `scripts/qualify-release.ps1` audited all 18 workspace
packages: coordinated 0.1.0 metadata, edition 2024, pinned/minimum Rust 1.98.1,
path-plus-version dependencies, embedded MIT/Apache licenses, package file lists,
and dependency order. Core is dependency-free, the harness depends only on core,
and the independent minimal consumer resolves exactly view, view-core and itself.

The final audit and profile matrix were repeated on 2026-10-26. Its core archive
was packaged and verified twice with identical SHA-256
`4D5976E13A77FB9B2F7175DB179353B88312AAFA080FBDB18D4DD4F86DAC907A`.
Machine-readable results live in `target/verification/release/qualification.json`.
Package-local copies of referenced font fixtures include attribution and hashes;
they prevent source archives from referring outside their package directory.

This is a dirty-working-tree content audit, not a release candidate. Registry
names, registry-dependent archive verification, current cross-platform execution,
native acceptance and clean reviewed-revision reproduction remain release gates.
Publication remains disabled. No package or diagnostic artifact was uploaded.
Read-only crates.io API lookups on 2026-10-25 found `view` occupied (0.4.1).
The thirteen queried `view-*` library/tool names returned 404; this is neither
reservation nor a future availability guarantee. Results are retained in
`target/verification/release/registry-names.json`. The facade name requires an
owner decision before publication; local development keeps the requested name.

The subsequent `-Profiles` run passed: every package's minimal/default/full
all-target checks and the independent consumer's minimal, individual-feature and
combined-feature tests. The report records each executed profile. This uses the
pinned 1.98.1 compiler, which is also the declared minimum.

The final text/input source rerun passed on 2026-10-26:
`target/verification/m8-packages-complete.log`, 18 packages, 36 profile groups
and zero audit failures. New Page/bidi and View Pad host regressions use their
own package-local font copies; the audit verifies their manifests, hashes,
licenses and archive-local include paths. The repeated core archive hash above
is unchanged.
