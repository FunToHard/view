# Working on view

view (The devil's framework) is a Rust native UI framework. Windows 11 x64 is the
first product target; Linux/macOS receive CI validation with explicitly recorded
coverage. This file applies to the whole repository.

## Read before changing architecture

- Start with `docs/design/README.md` and `docs/design/09-decisions-and-questions.md`.
- Follow the milestones in `docs/plan/README.md` and tasks in
  `docs/plan/initial-backlog.md`.
- Track completion in `docs/plan/implementation-checklist.md`. Check an item only
  with implementation/verification evidence; preserve stable IDs and distinguish
  missing platform coverage from a passed gate. Avoid duplicate progress boards.
- Read the affected subsystem document before implementing it. Design examples
  are sketches until compiled; do not treat their exact signatures as existing APIs.
- The bootstrap is authorized and initialized. Continue implementation within the
  user's requested scope; do not reinterpret historical planning-only statements
  as a requirement to ask permission for each routine edit.
- Record architecture changes with their reason, affected contracts, and
  acceptance evidence. Keep the decision register and related documents consistent.

## Architecture boundaries

- Use this monorepo and Cargo workspace; no first-party Git submodules. Add crates
  when implementation or a dependency/platform boundary needs them, not per widget.
- Keep one identity, ownership, input, semantics and lifecycle model for retained
  and immediate UI. Build/measure remain pure; input effects execute once.
- Keep application documents/scenes independent of the UI tree. Support
  demand-driven UI alongside independently scheduled 2D/3D viewport rendering.
- Use ordinary Rust builders, typed actions and explicit state/revisions. Avoid a
  mandatory DSL, global mutable model, ECS, HTTP stack, VM or browser runtime.
- Keep platform/wgpu/tool dependencies out of core. Optional automation, webview
  and add-on modules must not become mandatory core dependencies.
- Preserve native window behavior. Isolate FFI, thread affinity and unsafe code
  in platform/backend adapters with documented safety and ownership invariants.
- Standard controls use shared behavior engines and declared conformance profiles;
  styling must not remove expected editing, focus, accessibility or IME behavior.
- Respect webview disposal/hot-retention ownership contracts. Native child-process
  add-ons are trusted processes, not automatically sandboxed executables.

## Dependencies and Rust

- Use the pinned toolchain, edition 2024 and workspace resolver 3.
- Centralize new dependency versions/features in workspace dependencies. Explain
  purpose, maintenance evidence, transitive cost, target support and licensing.
- Qualify compatible graphics/text versions together. Do not call a dependency
  bug-free or select one solely because it is newest.
- Commit Cargo.lock. Keep packages unpublished until naming/release checks are
  complete. No native Rust ABI stability is promised.
- The public facade forbids unsafe code. Any adapter that needs unsafe code must
  explain each safety invariant and obey the workspace lints.

## Verification

Run from the repository root for Rust changes:

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo doc --workspace --no-deps --locked
```

Add meaningful behavior tests for new runtime/control contracts. Do not add tests
that merely mirror trivial implementation or claim empty test suites prove UI
behavior. Once features/packages exist, test supported profiles and independent
consumers as well as the workspace to catch feature-unification mistakes.

For documentation-only changes, check local links, fences and affected decision/
requirement/test references. Record what actually ran. A Windows check does not
prove Linux/macOS execution; CPU/headless tests do not prove GPU/native E2E.

Physical mouse/keyboard E2E requires an explicitly selected desktop test session,
foreground checks and held-input cleanup. Normal builds/tests must not seize input.
Keep generated artifacts in target/ or work/ and never use personal browser
profiles as lifecycle test fixtures.

## Change delivery

- Keep changes focused; preserve unrelated user work. Update examples, contracts
  and acceptance scenarios together when behavior changes.
- Prefer short-lived branches and atomic cross-crate changes; no permanent
  platform branches. Use normal Cargo dependencies for community controls.
- Report changed behavior, verification, and material gaps without fabricated
  benchmarks, native support or CI results.
- Local setup does not authorize pushing, publishing packages/releases, uploading
  diagnostics, or creating external infrastructure. Follow the user's scope for
  those operations; do not introduce extra approval gates for routine local work.
