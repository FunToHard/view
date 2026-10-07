# M0 qualification evidence

Date: 2026-10-07. This report records executed local work, not full milestone
closure. See the [canonical checklist](../implementation-checklist.md).

## Selected dependency family

Versions below are the tested lockfile resolution. Workspace requirements are
compatible ranges; upgrades require rerunning qualification, not silently treating
the old evidence as proof of a new resolution.

| Dependency | Tested version | Purpose and boundary |
| --- | --- | --- |
| winit | 0.30.13 | Window/event API; stable 0.30 line matches AccessKit adapter |
| windows | 0.62.2 | Official native bindings; narrow Windows-only probe features |
| wgpu | 30.0.1 | Native GPU API; glyphon requires the 30.x family |
| cosmic-text | 0.19.0 | Text types/layout foundation; matches glyphon 0.12 |
| glyphon | 0.12.0 | GPU text integration; compile and object-creation proof |
| accesskit / accesskit_winit | 0.25.1 / 0.34.1 | Semantic update and winit window/event type compatibility |
| lyon | 1.0.19 | Tessellation; independent triangle-area fixture |
| unicode-segmentation | 1.13.3 | Explicit grapheme boundary fixture |
| bytemuck | 1.25.2 | Checked POD vertex bytes; no hand-written unsafe cast |

The [machine-readable inventory](dependencies-windows.json) records actual license,
minimum-Rust metadata, selected feature resolution and duplicate families. Missing
upstream rust-version is reported as null; the executed compiler is 1.98.1, not a
claim to have tested older versions. [Primary upstream revisions](upstream-maintenance.json)
record activity observed during qualification. Activity is maintenance evidence,
not a bug-free certification or guarantee of future support.

Compatibility selection is based on the actual upstream
[glyphon manifest](https://docs.rs/crate/glyphon/0.12.0/source/Cargo.toml) and
[AccessKit adapter manifest](https://docs.rs/crate/accesskit_winit/0.34.1/source/Cargo.toml),
downloaded Cargo manifests and compiler-checked shared types. No renderer or
accessibility implementation is copied into core.

## Cost and feature boundaries

The filtered Windows workspace metadata contains **127 packages**, including
**124 registry dependencies and 3 local workspace packages**. This is the entire
qualification stack. The isolated platform tree contains **30 packages including
the probe**. The independent facade consumer contains **2 local packages and no
registry dependencies**. Counts describe package nodes, not binary size or runtime
memory; no performance claim follows from them.

Duplicate families include font-types/read-fonts/skrifa from distinct shaping and
rasterization dependency paths; hashbrown/rustc-hash/smol_str and syn also have
multiple compatible transitive consumers. The selected graph has one wgpu,
cosmic-text, glyphon, winit and AccessKit version. Do not force incompatible
transitive versions together merely to reduce a count; review upstream alignment
on upgrades. Use cargo tree --duplicates to inspect paths.

Cargo metadata's resolved feature list can reflect workspace/target feature
resolution rather than only compiled target code. The inventory script also emits
the target-specific feature tree. Linux X11/Wayland settings do not imply those
native backends were compiled/executed on Windows. Glyphon enables cosmic-text
defaults transitively, including fontconfig-related features; locally disabling
cosmic-text defaults cannot override another dependency's additive features.
AccessKit Unix service integration is not selected/certified by this probe.

## Executed local validation

Command: `pwsh -File scripts/verify.ps1 -Gpu -Backend dx12`.

- rustfmt, all-target workspace check, strict Clippy, tests and rustdoc passed.
- Four platform tests passed: raw-handle representation, layout/constants,
  compile-only Win32 signatures and Windows event-loop builder configuration.
- Two graphics dependency tests passed: owned Unicode grapheme boundaries and
  triangle tessellation area/POD upload layout.
- Minimal/all-current-feature facade checks and independent consumer build passed.
  The facade is still empty and has no feature flags; these prove packaging only.
- Both owned fixture hashes passed. No bundled fonts/images or network fixtures
  are silently implied; see the [fixture inventory](../../../tests/fixtures/README.md).
- The GPU probe created glyphon GPU objects with the shared wgpu device, dispatched
  owned WGSL and read back the independently expected `[7, 10, 13, 16]`.
- The separate opt-in command `cargo run -p platform-probe --example native-window
  --locked -- --run-hidden` created and closed an actual native window successfully:
  decorated=true, scale=1, physical inner size 800x600. It displayed no window and
  injected no input. This is lifecycle smoke, not M1 frame/input certification.

Recorded local environment: Windows 11 Pro x64, build 10.0.26300, Rust 1.98.1,
NVIDIA GeForce RTX 3050 6GB Laptop GPU, device type DiscreteGpu, backend Dx12,
driver 32.0.16.1714. This is hardware correctness evidence for this small workload,
not a performance benchmark, rendered text test or native presentation test.

Raw logs, actual environment and feature inventory are regenerated under
`target/verification/` and uploaded by CI when run. They are intentionally excluded
from Git to avoid machine-specific trace churn. Source fixtures, manifests,
lockfiles and this concise evidence are retained.

## Clean-checkout reproduction

Baseline commit: `9db5751` (Bootstrap view and qualify M0 dependency and CI foundation).
Created a separate local clone with `git clone --no-hardlinks --no-checkout`, then
checked out that revision detached. Its status was clean before verification.
Ran its own `scripts/verify.ps1` successfully: formatting, all-target compilation,
strict Clippy, six tests, rustdoc, facade profiles, independent consumer, fixture
hashes, dependency inventory and Windows platform probe all passed.

The root Cargo target cache was reused through CARGO_TARGET_DIR; the consumer used
its separate target directory. This proves clean **source-checkout** reproducibility
with caches, not an offline installation or a cold-machine build. No untracked
source or work/ files were copied into the checkout. Both lockfiles are tracked;
target/ and work/ artifacts remain ignored. GPU and hidden native-window tests were
run in the primary checkout at the same source baseline, separately from CPU CI.

## Remaining gates

- M0-10: three-OS CPU workflows are configured but have not run remotely.
- M0-12: Windows local GPU execution passed; Linux Vulkan/macOS Metal runner
  provisioning and execution remain unavailable. GitHub reported zero self-hosted runners.
- M0-13: dedicated desktop, DPI/display matrix and named Japanese IME fixture are
  not provisioned/verified. No physical input was injected or user input settings changed.
- M0-14: complete locally as recorded above.
- M0-15: remains open until required infrastructure evidence and the Phase 2
  native-shell contract exist. Hidden native window creation alone cannot certify
  caption/snap/focus/IME behavior or the shared UI runtime.

The CPU workflow uses pinned official checkout/upload action commits, read-only
contents permissions and short artifact retention. GPU jobs require manual dispatch
to provisioned isolated runners; see the [runner contract](../runner-contract.md).
Writing a workflow is not proof of an executed job. No paid hardware, runner
registration, remote publication or credentials are created by these local checks.
