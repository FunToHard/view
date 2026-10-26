# Changelog

## 0.1.0 — unreleased

Unpublished developer preview; package names and distribution are unqualified.
The workspace uses Rust 1.98.1 / edition 2024 and MIT OR Apache-2.0 for first-party
code. The canonical [implementation checklist](docs/plan/implementation-checklist.md)
records implementation and outstanding acceptance gates.

- Shared retained/declarative/immediate ownership, actions, revisions, layout,
  focus, input and committed semantics through the safe `view` facade.
- Optional text/shaping, shared control behaviors, native Windows shell and
  accessibility adapters, wgpu 2D/scene rendering and host embedding.
- Popup-owned select, bounded tables, shared-editor password entry, and docking
  transitions with a hidden native restoration proof.
- Application-owned styled document transactions, measured pagination and
  conservative disjoint rebasing; sparse SDR masks/blur/export and scene assets,
  translation animation and directional metallic-roughness shading.
- Bounded host-owned diagnostics, composed panic reports, supervised failure
  recovery and explicit WER configuration guidance.
- Bounded async resources and remote sections, editor/storefront/IDE proofs,
  public Markdown package and compile-tested authoring examples.
- Optional automation/CLI/MCP, Windows WebView2 ownership policies and trusted
  process add-on protocol; each has separate lifecycle and platform gates.
- Offline local packaging/graph/content qualification and independent feature
  consumers, with generated evidence kept under `target/verification/`.
- Correct extended navigation-key injection on Windows; guarded native tests
  observe Home/End and modified selection instead of keypad variants.

No previous published version exists, so there is no published-version migration
or deprecation removal in this preview. Design-document signatures are historical
sketches; migrate prototype callers to the compiled 0.1 examples and the
[authoring reference](docs/guides/authoring.md). Optional features must be selected
explicitly. Rust crate version compatibility and version-1 wire protocol
negotiation are separate contracts. See the
[platform/feature matrix](docs/plan/preview-release.md) for limits; this entry does
not certify native input, accessibility, multi-OS GPU or release readiness.
