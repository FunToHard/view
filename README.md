# view

**The devil's framework.**

A Rust framework being designed for native desktop applications that combine retained UI, on-demand immediate UI, editable 2D content, and real-time 3D rendering through wgpu.

The intended applications include game-engine editors, IDEs, image editors, and other demanding creative and development tools. Native platform behavior, extensibility, automation, and measurable performance are design requirements.

**Status: M0 qualification; design baseline 0.4.** The workspace contains an unpublished, dependency-free `view` facade plus development-only platform and graphics/text compatibility probes. Rust 1.98.1, contributor guidance, MIT OR Apache-2.0 licensing and GitHub Actions workflows are configured. There is no runtime/UI API or performance certification yet. Windows 11 x64 remains the first product target; actual coverage is recorded in the implementation checklist.

Start with the [design index](docs/design/README.md). The first delivery target is Windows; Linux and macOS participate in CI testing with explicitly recorded coverage limitations.

The [project plan](docs/plan/README.md) defines the executable milestones. The [repository plan](docs/plan/repository-and-maintenance.md) selects one Git monorepo and a modular Cargo workspace, without first-party submodules.

Use the [ordered implementation checklist](docs/plan/implementation-checklist.md)
to track the complete developer preview from bootstrap through release validation.
Completed setup, remaining work and deferred expansions are recorded separately.

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
