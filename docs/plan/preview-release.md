# Developer preview compatibility and qualification

The coordinated workspace version is **0.1.0**, edition 2024, with Rust **1.98.1**
as the pinned compiler and declared minimum. Every package remains `publish=false`.
This document is a compatibility/reproduction index, not a second progress board;
completion remains in the [canonical checklist](implementation-checklist.md).

Pre-1.0 breaking Rust API changes increment the minor version; patches preserve
documented contracts. No native Rust ABI stability is promised. The add-on and
automation wire protocols negotiate version 1 independently of Cargo versions;
unsupported protocol/schema versions are rejected, and document revisions/epochs
are validated by the host. Webview bridge grants are session/epoch/document scoped;
session and epoch wire values use decimal strings. Keep protocol compatibility
changes and consumer updates in the same reviewed change.

## Platform and feature matrix

| Surface | Windows 11 x64 | Linux/macOS | Evidence boundary |
| --- | --- | --- | --- |
| Minimal runtime and harness | CPU implemented/tested | Earlier executed CPU evidence | [M1](evidence/m1a-runtime.md); current revision needs current OS execution |
| Text and controls | CPU/render implementation; physical IME/AT gaps | Earlier CPU/build evidence; native gaps | [M2A](evidence/m2a-text-editing.md), [M2B](evidence/m2b-controls.md) |
| wgpu 2D/scene/embedding | Local hardware DX12 evidence | GPU/native execution open | [M3A](evidence/m3a-renderer.md), [M3B](evidence/m3b-completion.md) |
| Application/Markdown proofs | CPU plus explicitly recorded native/GPU evidence | Current native proof coverage open | [M4](evidence/m4-authoring-proofs.md) |
| CLI/MCP | CPU subprocess/dispatcher evidence | Current revision execution required | [M5](evidence/m5-automation.md) |
| Physical input | Explicit selected session only; uncovered cases remain | No initial parity claim | [runner contract](runner-contract.md) |
| Webview | Optional Windows WebView2 adapter/runtime | Portable ownership/bridge contracts; no native adapter | [M6](evidence/m6-webview.md) |
| Native-process add-ons | Optional trusted AOT child process | Portable protocol; native/platform gates tracked | [M7](evidence/m7-addons.md) |
| Diagnostics | Optional host-owned reporting | Portable reports; OS collectors target-specific | [crash policy](../design/12-diagnostics-and-crash-policy.md) |

CPU compilation, a hidden window, a native screenshot and a physical interaction
are different evidence. Existing Linux/macOS logs qualify their recorded revision,
not later changes. GPU performance uses hardware and named workloads; software
rendering is excluded by the owner's memory-budget clarification.

## Local package qualification

```text
pwsh -File scripts/qualify-release.ps1
pwsh -File scripts/qualify-release.ps1 -Profiles
```

The offline audit enforces coordinated metadata, sibling path-plus-version
declarations, disabled publication, dependency-free core, core-only harness and
the independent minimal consumer's exact dependency graph. It derives dependency
order including optional normal/build dependencies, inspects each Cargo package
file list, checks embedded first-party licenses and rejects generated/private
artifact paths. It packages and verifies the dependency-free core twice and
compares archive SHA-256. Reports are written to `target/verification/release/`.
`-Profiles` checks every workspace package with minimal/full features and runs
each independent consumer feature separately, plus minimal and combined profiles.
The rest of the [normal verification suite](feature-profiles.md) remains required.

Package lists are content dry runs; they do not verify registry-dependent archives.
The repeatable core archive is evidence for those bytes in this working tree,
not a clean release candidate. The facade name `view` was occupied in the
2026-10-25 registry audit and requires an owner naming decision. Registry dependency
resolution, all-package archive verification, third-party attribution review,
vulnerability/maintenance review and clean reviewed-revision reproduction remain
release gates. `-RequirePublishable` deliberately fails while the known release
blockers remain. It never enables publishing or changes package manifests.

All package directories embed the two first-party license texts. Third-party
dependencies retain their own licenses; generate the selected dependency inventory
with `scripts/dependency-inventory.ps1`, and review fixture attribution in the
[owned fixture manifest](../../tests/fixtures/README.md). An SPDX expression and a
passing compilation alone do not constitute a legal or vulnerability audit.

## Guide and proof index

| Contract | Versioned compiled source / reproduction | Guide |
| --- | --- | --- |
| Setup/architecture | `view-testing --example headless`; runtime contract tests | [setup](../guides/setup.md), [runtime](../guides/headless-runtime.md) |
| Rust authoring | Eight `view` examples run by `verify.ps1` | [authoring](../guides/authoring.md) |
| Standard/community controls | Gallery headless mode and separate Markdown consumer | [controls](../guides/controls.md), [control SDK](../guides/community-controls.md) |
| Text/IME | `view-text` suites; separate native prerequisites | [text](../guides/text-editing.md) |
| Rendering/canvas/scene | Explicit offscreen/scene examples with `dx12` | [rendering](../guides/rendering.md), [canvas](../guides/canvas.md), [scene](../guides/scene.md) |
| Host embedding | `view-wgpu --features scene --example embedded -- dx12` | [embedding](../guides/embedding.md) |
| Network UI | `view --features resources --example async_branch`; application proofs | [network UI](../guides/network-ui.md) |
| Automation | `view-tooling --all-features` tests; explicit desktop script | [automation](../guides/automation.md) |
| Webview | `verify.ps1 -NativeWebview` using owned fixtures | [webview](../guides/webview.md) |
| Add-ons | `view-addons --bin review-addon -- --proof` | [add-ons](../guides/addons.md) |

Use full Cargo commands from the linked guides; table cells abbreviate package
invocations. Native/GPU modes are opt-in. The onboarding pack is compile-tested;
independent new-reader/model learning evidence for T-27 remains a distinct gate.
Requirement/capability/scenario mappings are in the
[traceability audit](preview-traceability.md). Current local packaging evidence is
in [M8 packaging](evidence/m8-packaging.md).
