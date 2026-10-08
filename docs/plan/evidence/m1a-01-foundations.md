# M1A-01 foundation contracts

Date: 2026-10-08. Local Windows x64, pinned Rust 1.98.1. This is foundation
coverage for P-01/P-03/Q-15 and the T-39 package boundary, not completion of
T-01–T-05 or RUN-01/RUN-02/RUN-05.

This is the historical M1A-01 evidence snapshot. The later
[M1A runtime report](m1a-runtime.md) supersedes implementation/deferred-crate
statements below; it does not alter what was tested in this first increment.

## Delivered behavior and boundaries

- [view-core](../../../crates/view-core/src/lib.rs) defines arena-qualified
  handles, nonzero generations, distinct window IDs, revisions and structured
  errors. It forbids unsafe code and has no dependencies or feature flags.
- Generation/revision advancement fails at u64 exhaustion. Slots must retire
  rather than wrap. Arena IDs must be unique and never reused within a session;
  an issuing runtime and arena storage remain M1A-02 work.
- Raw identity constructors do not validate occupancy, lifetime or ownership.
  Window IDs are framework values, not native handles. No global ID allocator,
  platform type, executor or backend is introduced.
- The [facade](../../../crates/view/src/lib.rs) re-exports these types; the
  [independent consumer](../../../tests/compatibility/facade-consumer/src/main.rs)
  compiles actual usage. Both lockfiles include only the new local core package;
  registry resolutions are unchanged. All packages remain unpublished.
- Per the staged-crate rule, view-testing will be introduced with its first
  harness implementation (M1A-11). No empty crate or speculative harness API is
  used as evidence. Foundation tests currently belong to core.

## Executed verification

`pwsh -File scripts/verify.ps1` passed:

- `cargo fmt --all -- --check`
- `cargo check --workspace --all-targets --locked`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --locked`
- `cargo doc --workspace --no-deps --locked`
- Isolated core tests with defaults disabled, facade minimal/all-current-feature
  compilation, independent consumer compilation/formatting, fixture hashes,
  dependency inventory and the noninteractive platform binding probe.

The new [foundation tests](../../../crates/view-core/tests/foundations.rs) cover
revision advancement/exhaustion, distinct arena/slot/incarnation identities and
generation exhaustion. A compile-fail doctest rejects passing an ordinary arena
handle where a window ID is required. Three new integration tests and one new
doctest passed; the six existing qualification tests also passed. These tests do
not allocate/reuse actual arena slots or reject stale dispatches (M1A-02).

`cargo tree --manifest-path tests/compatibility/facade-consumer/Cargo.toml --locked`
showed exactly three local packages: consumer → view → view-core, with zero
registry dependencies. The workspace qualification graph retains 124 registry
packages. Raw verification output is in ignored `target/verification/`.

Local Markdown links/anchors and fences in affected documents and
`git diff --check` were checked. No Linux/macOS execution, GPU, native window or
physical input test was performed for this change. Existing M0 infrastructure
gates remain open; CI workflow availability is not new execution evidence.
