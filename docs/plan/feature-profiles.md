# Package and feature profiles

| Profile | Command | Evidence meaning |
| --- | --- | --- |
| Facade default | `cargo check -p view --locked` | Public package alone |
| Core alone | `cargo test -p view-core --no-default-features --locked` | Runtime contracts without facade or qualification dependencies |
| Harness alone | `cargo test -p view-testing --no-default-features --locked` | Controlled driver over public core |
| Headless example | `cargo run -p view-testing --example headless --locked` | Actions, region state and idle behavior |
| Facade minimal | `cargo check -p view --no-default-features --locked` | No default feature dependency leak |
| Facade all current features | `cargo check -p view --all-features --locked` | Currently identical: no facade features exist yet |
| Qualification workspace | `pwsh -File scripts/verify.ps1` | CPU APIs/tests plus Windows probe only on Windows |
| Independent consumer | `cargo check --manifest-path tests/compatibility/facade-consumer/Cargo.toml --locked --target-dir target/consumer` | Separate workspace/lockfile; no dependency unification with development probes |
| Independent harness consumer | `cargo test --manifest-path tests/compatibility/facade-consumer/Cargo.toml --features harness --locked --target-dir target/consumer` | Explicit optional harness shares facade runtime types |
| Explicit GPU | `pwsh -File scripts/verify.ps1 -Gpu -Backend dx12` | Adds real device/shader/readback qualification; choose vulkan/metal on other hosts |

The independent consumer deliberately disables facade defaults. Its default graph
contains only consumer, view and view-core; the consumer's optional `harness`
feature adds view-testing. Both profiles have zero registry dependencies (check
using Cargo tree). The default binary executes a typed action through the facade;
the harness profile tests a controlled input/commit through public interfaces.
Neither profile certifies native UI behavior. There are no facade/core/testing
feature flags yet; the feature belongs only to the compatibility consumer.

All packages remain unpublished. Platform and dependency probes are development
tools, not supported application APIs. Core has zero third-party dependencies and
no feature flags; the facade and harness each depend only on core. wgpu/text/accessibility dependencies belong
to those probes only. Future platform/render/control features get separate
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
