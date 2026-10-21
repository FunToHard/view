# view

**The devil's framework.**

A Rust framework being designed for native desktop applications that combine retained UI, on-demand immediate UI, editable 2D content, and real-time 3D rendering through wgpu.

The intended applications include game-engine editors, IDEs, image editors, and other demanding creative and development tools. Native platform behavior, extensibility, automation, and measurable performance are design requirements.

**Status: runtime, native shell, shared text editing, preview controls, optional 2D/3D rendering and host-embedding proofs implemented; design baseline 0.4.** The unpublished `view` facade exposes dependency-free core ownership, layout, input and committed semantics. Optional text/shaping and wgpu rendering features add editing and SDR composition; the `scene` feature adds basic depth-tested scenes and picking. Native IME/physical-input certification remains open. Rust 1.98.1 and MIT OR Apache-2.0 licensing are configured. Earlier runtime/text CPU tests passed on Windows, Linux and macOS; rendering/embedding have local Windows GPU evidence, with Linux/macOS graphics coverage still open. Actual coverage is recorded in the implementation checklist.

Start with the [design index](docs/design/README.md). The first delivery target is Windows; Linux and macOS participate in CI testing with explicitly recorded coverage limitations.

The [project plan](docs/plan/README.md) defines the executable milestones. The [repository plan](docs/plan/repository-and-maintenance.md) selects one Git monorepo and a modular Cargo workspace, without first-party submodules.

Use the [ordered implementation checklist](docs/plan/implementation-checklist.md)
to track the complete developer preview from bootstrap through release validation.
Completed setup, remaining work and deferred expansions are recorded separately.

For the implemented headless API, start with the
[runtime guide](docs/guides/headless-runtime.md) and run
`cargo run -p view-testing --example headless --locked`.

For 2D paint, text, images, layers and providers, see the
[rendering guide](docs/guides/rendering.md). The explicit offscreen fixture is
`cargo run -p view-wgpu --example offscreen --locked -- dx12`.

For controls, styling, accessibility and the interactive gallery, see the
[controls guide](docs/guides/controls.md) and [preliminary evidence](docs/plan/evidence/m2b-controls.md).

For canvas, scene picking, drag/undo, viewport scheduling and host-owned rendering,
see the [canvas](docs/guides/canvas.md), [scene](docs/guides/scene.md) and
[embedding](docs/guides/embedding.md) guides. Run the explicit offscreen host proof:
`cargo run -p view-wgpu --features scene --example embedded --locked -- dx12`.

## Development

For canonical builders, async resources, remote sections and the engine-editor /
native-storefront / IDE proofs, see the [authoring reference](docs/guides/authoring.md).
The separate Markdown preview package follows the
[public community-control contract](docs/guides/community-controls.md).
Optional CLI/MCP automation, inspection and explicitly selected desktop tests are
documented in the [automation guide](docs/guides/automation.md).
Run `cargo run -p view-application-proofs --locked` for deterministic application
flows; native/GPU modes are explicit. [M4 evidence](docs/plan/evidence/m4-authoring-proofs.md)
distinguishes local implementation checks from unexecuted platform/native gates.

See [CONTRIBUTING.md](CONTRIBUTING.md) for prerequisites and [AGENTS.md](AGENTS.md)
for repository guidance. From the repository root:

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo doc --workspace --no-deps --locked
```

The pinned toolchain is also the declared bootstrap minimum. For the complete
qualification suite run `pwsh -File scripts/verify.ps1`; an explicit GPU check adds
`-Gpu -Backend dx12`. See [profiles](docs/plan/feature-profiles.md) and
[runner requirements](docs/plan/runner-contract.md). Qualification does not imply
implemented framework behavior, native E2E or full cross-platform product support.

## License

Licensed under either the [MIT license](LICENSE-MIT) or the
[Apache License, Version 2.0](LICENSE-APACHE), at your option.
