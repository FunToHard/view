# Package and feature profiles

| Profile | Command | Evidence meaning |
| --- | --- | --- |
| Facade default | `cargo check -p view --locked` | Public package alone |
| Add-on process/control contracts | `cargo test -p view-addons --all-features --locked` | Actual local child negotiation/failure and host-control/permission tests; no physical input |
| Add-on review proof | `cargo run -p view-addons --bin review-addon --locked -- --proof` | Native guest, host document transaction and undo, cleanup receipts |
| Add-on rendering | `cargo run -p view-addons --features render --bin review-addon --locked -- --gpu dx12` | Explicit offscreen host-control rendering; no native window claim |
| Independent add-ons | `cargo test --manifest-path tests/compatibility/facade-consumer/Cargo.toml --features addons --locked --target-dir target/consumer` | Optional facade protocol outside root feature unification |
| Webview lifecycle CPU | `cargo test -p view-webview --all-features --locked` | Mock lifecycle/bridge plus native adapter compilation on Windows; no browser launched |
| Independent webview | `cargo test --manifest-path tests/compatibility/facade-consumer/Cargo.toml --features webview --locked --target-dir target/consumer` | Portable facade contract without native feature unification |
| Independent native webview | `cargo check --manifest-path tests/compatibility/facade-consumer/Cargo.toml --features webview-native --locked --target-dir target/consumer` | Windows adapter dependencies outside root feature unification; no browser launched |
| Explicit Windows WebView2 | `pwsh -File scripts/verify.ps1 -NativeWebview` | Adds passive owned-profile native lifecycle fixture; no physical input |
| Automation CPU | `cargo test -p view-automation --locked` | Selectors, revision/predicate/geometry and guarded-driver mocks; no physical input |
| CLI/MCP | `cargo test -p view-tooling --all-features --locked` | Actual local rmcp subprocess and shared dispatcher parity |
| Independent automation | `cargo test --manifest-path tests/compatibility/facade-consumer/Cargo.toml --features automation --locked --target-dir target/consumer` | No native/MCP feature leak |
| Automation GPU artifacts | `cargo run -p view-application-proofs --features automation --locked -- --gpu dx12` | Actual readback receipts, inspector and bounded bundle |
| Selected desktop | `pwsh -File scripts/verify-desktop.ps1 -SelectedDesktop -Scenario en-US` | Physical input; explicitly opt-in, F12 stop; separate from normal CI |
| Japanese IME | `pwsh -File scripts/verify-desktop.ps1 -SelectedDesktop -Scenario ja-JP` | Requires installed/selected Japanese IME; missing prerequisite fails as uncovered |
| Resources and codec | `cargo test -p view-resource --all-features --locked` | Request races, stream/backpressure, schema/asset budgets, opt-in BMP |
| Public authoring | `cargo test -p view --all-features --locked` | Nested keyed branches and retained/immediate islands |
| Community Markdown | `cargo test -p view-markdown --locked` | Specialized block preview profile, no renderer/network dependency |
| Independent Markdown/resources | `cargo test --manifest-path tests/compatibility/facade-consumer/Cargo.toml --features markdown,resources --locked --target-dir target/consumer` | Separate workspace/lockfile and profile adapter |
| Application proofs | `cargo run -p view-application-proofs --locked` | Deterministic editor, storefront and IDE workflows |
| Application GPU | `cargo run -p view-application-proofs --locked -- --gpu dx12` | Scene/gizmo/text/asset offscreen readback |
| Application native | `cargo run -p view-application-proofs --features native --locked -- --run-hidden` | Three hidden Windows/DX12 surfaces; no physical input |
| Core alone | `cargo test -p view-core --no-default-features --locked` | Runtime contracts without facade or qualification dependencies |
| Canvas contracts | `cargo test -p view-core --test canvas_contracts --locked` | CPU precision, pan/zoom, snapshot, hit and semantic contracts |
| Independent canvas consumer | `cargo test --manifest-path tests/compatibility/facade-consumer/Cargo.toml --test canvas --locked --target-dir target/consumer` | Canvas facade without optional backend features |
| Harness alone | `cargo test -p view-testing --no-default-features --locked` | Controlled driver over public core |
| Headless example | `cargo run -p view-testing --example headless --locked` | Actions, region state and idle behavior |
| Facade minimal | `cargo check -p view --no-default-features --locked` | No default feature dependency leak |
| Facade all current features | `cargo check -p view --all-features --locked` | Includes optional editor, shaping and wgpu 2D renderer |
| Renderer CPU contracts | `cargo test -p view-wgpu --locked` | Graph, color, surface state and shared text visuals; no GPU implicit in tests |
| Optional scene CPU | `cargo test -p view-wgpu --features scene --locked` | Camera, transform, mesh, revision and budget contracts without a GPU |
| Optional scene GPU | `cargo run -p view-wgpu --features scene --example scene --locked -- dx12` | Explicit depth, clipping, composition, resize and device recreation readback |
| Host embedding GPU | `cargo run -p view-wgpu --features scene --example embedded --locked -- dx12` | One host encoder/submission, independent scene/UI cadence, clipped viewports, scale/recreation and tiled brush proof |
| Independent scene consumer | `cargo test --manifest-path tests/compatibility/facade-consumer/Cargo.toml --features scene --locked --target-dir target/consumer` | Optional facade scene API outside workspace feature unification |
| Renderer hardware fixture | `cargo run -p view-wgpu --example offscreen --locked -- dx12` | Real rendering, pixel readback, cache and provider contracts |
| Hidden native surface | `cargo run -p view-wgpu --example surface --locked -- --run-hidden dx12` | Explicit hidden-window acquire/submit/present and resize/suspend; no physical input |
| Independent renderer consumer | `cargo test --manifest-path tests/compatibility/facade-consumer/Cargo.toml --features wgpu --locked --target-dir target/consumer` | Optional facade renderer outside root feature unification |
| Text alone | `cargo test -p view-text --no-default-features --locked` | Shared editor without shaping |
| Text shaping | `cargo test -p view-text --all-features --locked` | Owned-font shaping, wrapping, geometry and cache tests |
| Platform text | `cargo test -p view-platform --features text --locked` | IME event adapter without physical input |
| Windows isolated clipboard | `cargo run -p view-platform --features clipboard-test --example clipboard_isolated --locked -- --isolated` | Real clipboard round trips in a private noninteractive window station |
| CPU plus native clipboard | `pwsh -File scripts/verify.ps1 -NativeClipboard` | Explicit opt-in; fails if a fresh noninteractive station cannot be created |
| Independent text consumer | `cargo test --manifest-path tests/compatibility/facade-consumer/Cargo.toml --features text --locked --target-dir target/consumer` | Exercises the facade text feature outside workspace feature unification |
| Qualification workspace | `pwsh -File scripts/verify.ps1` | CPU APIs/tests plus Windows probe only on Windows |
| Independent consumer | `cargo check --manifest-path tests/compatibility/facade-consumer/Cargo.toml --locked --target-dir target/consumer` | Separate workspace/lockfile; no dependency unification with development probes |
| Independent harness consumer | `cargo test --manifest-path tests/compatibility/facade-consumer/Cargo.toml --features harness --locked --target-dir target/consumer` | Explicit optional harness shares facade runtime types |
| Explicit GPU | `pwsh -File scripts/verify.ps1 -Gpu -Backend dx12` | Adds real device/shader/readback qualification; choose vulkan/metal on other hosts |

The independent consumer deliberately disables facade defaults. Its default graph
contains only consumer, view and view-core; the consumer's optional `harness`
feature adds view-testing; those profiles have zero registry dependencies.
`text` adds view-text and unicode-segmentation. `text-shaping` additionally enables
cosmic-text, tested through its separate consumer profile. `wgpu` adds view-wgpu,
glyphon, lyon and bytemuck with the qualified wgpu/text family. The default binary executes a typed action through the facade;
the harness profile tests a controlled input/commit through public interfaces.
None of these profiles certifies native UI behavior. Core/testing remain featureless;
the facade's `text` feature is opt-in.

All packages remain unpublished. Platform and dependency probes are development
tools, not supported application APIs. Core has zero third-party dependencies and
no feature flags; the harness depends only on core, and the facade adds view-text
only when requested. view-text depends on core and unicode-segmentation, with
cosmic-text behind the shaping feature. GPU dependencies also belong to optional
view-wgpu; optional control semantic translation and native accessibility adapters reuse the qualified AccessKit family. Native window dependencies belong to
view-platform as well as platform probes. Future platform/render/control features get separate
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

Controls profiles: `cargo test -p view-controls --no-default-features --locked`
checks behavior without a GPU or native loop; `--all-features` also compiles the
native gallery and tests AccessKit translation. The independent consumer has
separate `controls`, `accessibility`, and `controls-render` profiles. Facade
controls do not enable the gallery or native windowing. `scripts/verify.ps1`
runs these profiles and the CPU gallery; `-Gpu` adds explicit GPU readbacks and
Windows hidden gallery presentation. See [control coverage](evidence/m2b-controls.md).
