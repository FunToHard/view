# view

**The devil's framework.**

A Rust framework being designed for native desktop applications that combine retained UI, on-demand immediate UI, editable 2D content, and real-time 3D rendering through wgpu.

The intended applications include game-engine editors, IDEs, image editors, and other demanding creative and development tools. Native platform behavior, extensibility, automation, and measurable performance are design requirements.

**Status: M1A headless runtime complete; design baseline 0.4.** The unpublished `view` facade exposes dependency-free `view-core` ownership, keyed reconciliation, typed actions, cancellation, demand scheduling and committed structural snapshots. `view-testing` provides controlled time/input and service fixtures. Native input, layout, semantics, controls and rendering remain later milestones. Rust 1.98.1 and MIT OR Apache-2.0 licensing are configured. Runtime CPU tests passed on Windows x64, Linux x64 and macOS ARM64; M0 infrastructure gates remain open. Actual coverage is recorded in the implementation checklist.

Start with the [design index](docs/design/README.md). The first delivery target is Windows; Linux and macOS participate in CI testing with explicitly recorded coverage limitations.

The [project plan](docs/plan/README.md) defines the executable milestones. The [repository plan](docs/plan/repository-and-maintenance.md) selects one Git monorepo and a modular Cargo workspace, without first-party submodules.

Use the [ordered implementation checklist](docs/plan/implementation-checklist.md)
to track the complete developer preview from bootstrap through release validation.
Completed setup, remaining work and deferred expansions are recorded separately.

For the implemented headless API, start with the
[runtime guide](docs/guides/headless-runtime.md) and run
`cargo run -p view-testing --example headless --locked`.

## Development

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
