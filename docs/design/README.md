# Design index

Revision: 0.4 — 2026-10-07. Status: selected design and project planning baseline; provenance of delegated platform/license/CI defaults is tracked in the decision register.

Bootstrap update: the owner subsequently authorized project initialization. The Git repository, initial Cargo workspace/facade, toolchain, licenses and contributor guidance now exist. Historical planning-only notes below describe the design revision, not a prohibition on the authorized setup. See the [backlog evidence](../plan/initial-backlog.md).

The name is **view**; the tagline is **The devil's framework**. Requirements explicitly supplied by the project owner are distinguished from proposed engineering choices throughout this set.

| Document | Purpose |
| --- | --- |
| [Product charter](01-product-charter.md) | Requirements, intended applications, scope, and performance meaning |
| [Runtime architecture](02-runtime-architecture.md) | Retained/immediate integration, ownership, scheduling, and lifecycle |
| [Rust authoring API](03-rust-authoring-api.md) | Flutter-like composition and examples to review before implementation |
| [2D/3D rendering](04-rendering-and-interop.md) | Composition, custom rendering, resource ownership, and embedding |
| [Native platform integration](05-native-platform.md) | Windows behavior and portable platform boundaries |
| [Automation and developer tools](06-automation-and-tooling.md) | E2E, inspection, deterministic tests, optional MCP, and iteration |
| [Feature discovery](07-feature-discovery.md) | Capability inventory, priorities, extension contracts, and dependency review |
| [Delivery and validation](08-delivery-and-validation.md) | Milestones, CI/CD coverage, acceptance scenarios, and performance evidence |
| [Decision register](09-decisions-and-questions.md) | Accepted requirements, selected decisions, deferred scope, and revision procedure |
| [Source ledger](10-sources.md) | Primary references, evidence boundaries, and follow-up verification |
| [Network-driven UI and web portability](11-network-ui-and-web.md) | Async branching, native storefront proof, and deferred WASM capability profile |
| [Diagnostics and crash policy](12-diagnostics-and-crash-policy.md) | Optional host-owned reporting, native faults, and healthy-operation recovery |
| [Controls and conformance](13-controls-and-conformance.md) | Community control crates, reusable behavior engines, and completeness profiles |
| [Runtime add-ons](14-runtime-addons.md) | Word-style custom UI, AOT extension routes, host contracts, and lifecycle |
| [Webview lifecycle](15-webview-lifecycle.md) | DisposeAfterUse and KeepHot, process/profile ownership, and cleanup evidence |
| [Project plan](../plan/README.md) | Executable milestones, work packages, release scope and acceptance gates |
| [Repository and maintenance](../plan/repository-and-maintenance.md) | Git monorepo, Cargo workspace, dependencies, compatibility and releases |
| [Initial backlog](../plan/initial-backlog.md) | Ordered M0/M1 work items, dependencies and completion evidence |
| [Implementation checklist](../plan/implementation-checklist.md) | Sequential preview implementation, current progress, acceptance gates and deferred roadmap |
| [M0 qualification evidence](../plan/evidence/m0-qualification.md) | Executed checks, selected dependency stack and remaining gates |
| [Package profiles](../plan/feature-profiles.md) | Independent/minimal facade and development qualification commands |
| [Runner contract](../plan/runner-contract.md) | CPU/GPU/interactive environment requirements and provisioning status |

## How to read status

- **Requirement:** explicitly requested behavior or constraint. Recorded as R-* in the decision register.
- **Selected:** architecture adopted in revision 0.4 under the owner's delegation. Recorded as P-* with resolutions in Q-*.
- **Delegated default:** an engineering choice made under the owner's request to finalize and continue; not an explicit questionnaire answer.
- **Historical proposal:** earlier rationale or an illustrative API sketch; the revision 0.4 decision baseline determines the current choice.
- **Deferred:** outside the initial implementation scope, with any necessary boundary designed now.

Descriptions using “must” define baseline contracts. All code snippets remain API sketches, not compiled examples or existing functionality. Dependency qualification, runner provisioning and measurements happen during implementation; selecting the design does not fabricate that evidence.

## Review order

1. Read the decision baseline for the final choices and their provenance.
2. Read the webview contract for the two lifetime policies and exact cleanup scope.
3. Use the project plan and repository plan to prepare M0/M1 implementation.
4. Treat the detailed design documents as contracts and acceptance references, not a second competing plan.

The [decision baseline](09-decisions-and-questions.md) closes the architectural questions: no mandatory ECS/reactive library, native process add-ons, canonical Rust builders, an optional basic scene module, selected text/graphics dependencies, and one monorepo/workspace. Deliberately deferred capabilities are outside the initial release rather than undecided implementation choices.

Revision 0.2 records positive owner feedback on the design criteria and adds proposals for network-driven branches, a native storefront proof, deferred web portability, crash diagnostics, application-integrated MCP, and optional physical hit-test/scroll/keyboard E2E. The owner clarified that no browser shell is intended. This revision does not expand release support or authorize implementation automatically.

Revision 0.3 adds a low-friction authoring/discovery contract for readers and models without view-specific training, first-class community control crates, versioned standard-control behavior profiles, and a concrete Word-style runtime add-on use case. The runtime backend remains open; AOT native processes, interpreted WASM, and trusted DLLs have different tradeoffs. No implementation is introduced.

Revision 0.4 resolves those technical alternatives, adds developer-selected webview disposal/hot retention, and establishes the monorepo and milestone plan. Earlier revision notes above are historical. No Git repository, Cargo manifests, implementation, CI configuration, or remote resource was created by this documentation update.
