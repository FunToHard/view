# Setup and first application — view 0.1

This repository is an unpublished developer preview. Use the pinned Rust 1.98.1
toolchain (also the declared minimum), Cargo edition 2024 and PowerShell 7 for the
repository scripts. Install rustup, then run `rustup show` from the repository root.
On Windows x64 install Visual Studio C++ build tools and the Windows SDK for the
MSVC linker. Native windows require a desktop session; GPU examples require a
compatible hardware adapter and driver. The first product target is Windows 11.
Linux/macOS compilation and CPU tests are portability evidence, with current
coverage listed in the [preview matrix](../plan/preview-release.md).

Start without a native window or renderer:

```text
cargo run -p view --example minimal --locked
cargo run -p view-testing --example headless --locked
cargo run -p view --features controls --example form --locked
```

The source of each command is the example, rather than an API sketch copied from
the design documents. The [authoring reference](authoring.md) describes composition,
typed actions and retained/immediate ownership. `view` has no default features;
the minimal consumer resolves only `view` and `view-core`. Add `controls`,
`resources`, `wgpu`, `scene` or other documented features explicitly.

For an application outside this workspace, declare a path and compatible version:

```toml
[dependencies]
view = { path = "../view/crates/view", version = "0.1.0", default-features = false }
```

There is no published registry package claimed by this guide. The separate
`tests/compatibility/facade-consumer` project demonstrates public usage with its
own lockfile and feature resolution. Build it from the repository root:

```text
cargo test --manifest-path tests/compatibility/facade-consumer/Cargo.toml --locked --target-dir target/consumer
pwsh -File scripts/verify.ps1
pwsh -File scripts/qualify-release.ps1 -Profiles
```

The first two commands do not inject physical input. Some normal Windows checks
create hidden native windows. Physical mouse/keyboard testing has a separate
explicitly selected desktop procedure in the [automation guide](automation.md).
Additional dependencies and limitations are documented per
[feature profile](../plan/feature-profiles.md). Build outputs, reports and fixture
captures belong in `target/` or `work/`.

For native controls run `cargo run -p view-controls --features gallery --example
gallery --locked`. For the small multiline editor run `cargo run -p view-pad
--release --locked`. Text/IME/accessibility and performance gates are documented
in their evidence; launching an example does not certify them.
