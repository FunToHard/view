# Contributing to view

The repository is at the bootstrap stage. Read [AGENTS.md](AGENTS.md), the
[design index](docs/design/README.md), and the [initial backlog](docs/plan/initial-backlog.md)
before beginning implementation.

## Local setup

Install Rust through rustup and the native linker/build prerequisites for your OS.
On Windows, use the MSVC toolchain with Visual Studio C++ build tools and a Windows
SDK. Running Cargo in this directory selects Rust 1.98.1 with rustfmt and Clippy.
This is the bootstrap compiler and declared minimum; the eventual dependency
stack still needs qualification.

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo doc --workspace --no-deps --locked
```

The unpublished facade has no runtime API or third-party dependencies. The separate
platform and dependency probes qualify native bindings and compatible graphics/text
types without becoming application APIs. Run `pwsh -File scripts/verify.ps1` for
the full CPU suite, fixture validation and independent consumer check. An opt-in
GPU check adds `-Gpu -Backend dx12` (or vulkan/metal on the appropriate host).
Reports are written under `target/verification/`; these commands never inject input.

See [feature profiles](docs/plan/feature-profiles.md) and the
[runner contract](docs/plan/runner-contract.md). Hosted CI configuration is not
proof that remote jobs have run. Native UI, text rendering and E2E remain separate
milestone gates even when qualification probes pass.

Add packages when needed, inherit workspace metadata/lints, and keep dependency
direction consistent with the [repository plan](docs/plan/repository-and-maintenance.md).
Commit the shared lockfile. Package names are not reserved on crates.io.

Keep pull requests focused and state behavior, tests and limitations. Amend design
decisions when changing an architectural contract. Use owned, licensed fixtures;
never commit credentials, personal profiles, local traces or build artifacts.

First-party code is available under MIT OR Apache-2.0. Preserve the actual licenses
and attribution of third-party material.
