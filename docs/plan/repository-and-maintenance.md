# Repository and long-term maintenance plan

Revision: 0.4. Status: selected architecture with initial bootstrap completed. Git, workspace/toolchain files, lockfile, licenses, guidance and the public facade crate exist. The full directory tree below remains a staged plan; other crates are added with implementation.

## Repository decision

Use one Git monorepo and one Cargo workspace for first-party framework crates, examples, tests, tools, and documentation. Do not use Git submodules for first-party components. Cargo workspace package/dependency inheritance, a shared repository lockfile, and coordinated commands support coherent changes across the framework. [Cargo workspaces](https://doc.rust-lang.org/cargo/reference/workspaces.html)

Submodules track separate repositories and commits and introduce a separate synchronization/update workflow. That is useful for genuinely independent repositories, but it is unnecessary overhead for tightly coupled runtime, backend, controls, and conformance changes. [Git submodules](https://git-scm.com/book/en/v2/Git-Tools-Submodules)

Community controls stay in independent repositories/packages. Third-party libraries use Cargo dependencies; a necessary fork is pinned with a reason and an upstream/removal plan. Do not copy dependency source into the monorepo or introduce submodules merely to avoid version management.

## Planned workspace

```text
view/
  Cargo.toml                 # virtual workspace; created in M0 implementation
  Cargo.lock                 # committed for this repo's tools/examples/tests
  rust-toolchain.toml         # qualified stable compiler, recorded MSRV
  crates/
    view/                    # supported public facade and feature selection
    view-core/               # identity, transactions, layout contracts, input, semantics
    view-text/               # text services and shared edit/session backend
    view-controls/           # standard controls, behavior profiles and composition
    view-wgpu/               # render providers, GPU resources, composition
    view-platform/           # winit and target-specific native adapters
    view-testing/            # deterministic harness and conformance support
    view-automation/         # command/query model and app command registration
    view-webview/            # optional Windows WebView2 adapter
    view-addons/             # optional process protocol and host-rendered contributions
  tools/
    view-cli/                # inspect/test/capture commands
    view-mcp/                # optional MCP adapter
    xtask/                   # repository checks/release orchestration, when useful
  examples/
    gallery/
    editor-workspace/
    storefront/
    host-embedding/
    community-markdown/
    addon-review-panel/
  tests/
    fixtures/                # owned fonts/assets/data and documented licenses
    native-e2e/
    graphics/
    compatibility/
  docs/
    design/
    plan/
    guides/
    adr/
```

Create crates as their milestone starts, not as empty placeholders. The first executable foundation needs core, platform, facade, and a harness; text/controls/rendering arrive in the next increments. Keep related implementation modules inside these crates until a dependency, compilation, platform, or release boundary justifies another package. Do not create a crate for every widget.

Package names shown are workspace identifiers; verify registry availability before publication. The framework identity remains view even if public package naming needs a reservation-compatible prefix. That check is a release task, not permission to reserve/publish anything now.

## Dependency direction

Core depends on Rust std and narrowly justified utilities, never platform windows, wgpu, HTTP, MCP, a webview, or an add-on runtime. Text supplies framework-owned measurement/edit contracts. Controls consume core/text services. The wgpu adapter owns backend-specific types and compatible text-rendering integration; internal adapter interop can couple selected backend versions without leaking them into every widget API.

Platform consumes runtime contracts and exposes native services. The facade assembles defaults; it does not become a second implementation. Testing/automation use supported observation/action interfaces. Optional webview/add-on/MCP packages cannot become reverse dependencies of core. A headless host can select core/text/testing without native windows or GPU device creation.

No import cycles. Prefer moving a small shared contract downward over creating a universal service locator or utility crate. Keep APIs private until needed by an actual consumer; expose supported backend/extension contracts intentionally.

## Workspace and version policy

- Use Rust edition 2024 and an explicit resolver 3 in the virtual workspace, subject to the selected qualified compiler meeting dependency requirements.
- Pin one qualified stable toolchain for repository CI; publish an MSRV at bootstrap and raise it through a documented release note.
- Centralize dependency versions/features with workspace inheritance. Backend families must use compatible wgpu/text versions.
- Commit Cargo.lock for repeatable repository builds. Downstream libraries still resolve declared version ranges; a lockfile is not a substitute for correct ranges.
- Test minimal/default/selected-full feature profiles and public consumer crates; do not assume workspace-wide feature unification proves independent package correctness.
- Use path plus version declarations for sibling packages intended for publication, and package checks that verify dependencies are publishable without repository-only paths.

Cargo's resolver and feature behavior need explicit workspace configuration and feature-profile testing. [Cargo dependency resolution](https://doc.rust-lang.org/cargo/reference/resolver.html)

## Release policy

Use MIT OR Apache-2.0 for first-party framework code. Both license texts and consistent SPDX workspace/package metadata were added during bootstrap; track future third-party dependencies, fonts, images and other fixtures under their actual licenses. No third-party attribution audit is claimed by this setup. GitHub Actions is the initial CI provider; keep contributor commands runnable locally and other job contracts portable.

Keep the public facade/core/controls/text/backend adapter family on a coordinated version train initially. Internal packages remain publish=false until an external consumer requires them. Independently installed add-ons negotiate a protocol version separate from Rust crate versions. Do not promise native Rust ABI stability.

For pre-1.0 releases, breaking public changes increment the minor version; patches preserve the documented contract. Avoid gratuitous breaking changes even before 1.0. Provide migration notes, compatibility tests, changelog entries, and deprecations where practical. Follow Cargo SemVer considerations for public types/features as well as functions. [Cargo SemVer](https://doc.rust-lang.org/cargo/reference/semver.html)

Publication proceeds in dependency order from a reviewed release commit with reproducible artifacts and passed applicable gates. CI builds artifacts automatically; publishing packages, signed installers, or releases is an explicit release operation. No automatic publishing is authorized by this plan.

## Maintenance workflow

Use short-lived branches and small reviewed changes against the main integration branch. No permanent platform branches and no long-lived v-next branch by default. A cross-crate API change and its examples/tests land together.

Each change states user-visible behavior, compatibility impact, tests, and any new dependency/feature cost. Platform, rendering, controls, and tooling ownership are review responsibilities; they do not require separate repositories. Architecture changes update a numbered decision/ADR and associated acceptance evidence.

An upgrade is a deliberate change: inspect transitive/features impact, run affected conformance/graphics/native suites, and avoid upgrading every unrelated library in the same patch. A discovered upstream bug gets a minimized reproduction and a tracked workaround with a removal condition. Do not market a dependency as bug-free.

Keep documentation runnable where appropriate, profiles versioned, and fixture licenses recorded. Review unsafe/native code at its explicit boundary. Community contribution guidance must show how to run the relevant tests without owning every GPU or OS; the CI matrix records remaining platform evidence.

## Repository alternatives rejected for this stage

| Alternative | Why not selected |
| --- | --- |
| First-party polyrepo | Makes atomic API/test migrations and version synchronization harder at this stage |
| First-party Git submodules | Adds checkout/update coordination without independent ownership need |
| Single giant crate | Forces optional backend/platform/tool dependencies together and blurs public contracts |
| One crate per component | Adds packaging/version/review overhead without useful boundaries |
| Vendored dependencies as the default | Transfers update/maintenance burden without demonstrated benefit |

Revisit repository separation only when there are independently owned products or a concrete release/access requirement. Do not split solely because the codebase becomes large.
