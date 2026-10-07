# Bootstrap package and feature profiles

| Profile | Command | Evidence meaning |
| --- | --- | --- |
| Facade default | `cargo check -p view --locked` | Public package alone |
| Facade minimal | `cargo check -p view --no-default-features --locked` | No default feature dependency leak |
| Facade all current features | `cargo check -p view --all-features --locked` | Currently identical: no facade features exist yet |
| Qualification workspace | `pwsh -File scripts/verify.ps1` | CPU APIs/tests plus Windows probe only on Windows |
| Independent consumer | `cargo check --manifest-path tests/compatibility/facade-consumer/Cargo.toml --locked --target-dir target/consumer` | Separate workspace/lockfile; no dependency unification with development probes |
| Explicit GPU | `pwsh -File scripts/verify.ps1 -Gpu -Backend dx12` | Adds real device/shader/readback qualification; choose vulkan/metal on other hosts |

The independent consumer deliberately disables facade defaults. Its graph must
contain only the consumer and view at this stage; check using Cargo metadata/tree.
It only imports the current empty facade and therefore does not certify any UI API.

All packages remain unpublished. Platform and dependency probes are development
tools, not supported application APIs. wgpu/text/accessibility dependencies belong
to those probes only. Future core/platform/render/control features get separate
minimal/default/selected-full consumer cases when implemented; no placeholder
features or empty production crates are added to simulate coverage.

The graphics probe enables native DX12/Metal/Vulkan and WGSL. Linux winit enables
X11/Wayland only for that target. AccessKit's Unix service feature is not enabled
yet: Linux compilation does not certify native accessibility. Glyphon's own
cosmic-text dependency enables upstream defaults through Cargo feature unification;
see the generated dependency inventory rather than assuming local feature flags
remove transitive defaults. winit/windows bindings are target-gated for the
Windows-only platform probe.

`scripts/dependency-inventory.ps1 -Target <triple>` records actual selected versions,
features, license/MSRV metadata, package counts and duplicated families. It may
download target dependencies. Root and consumer lockfiles are tracked. The declared
minimum/compiler is 1.98.1; no lower compiler or unexecuted OS is advertised tested.
