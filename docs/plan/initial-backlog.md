# Initial implementation backlog

Revision: 0.4. Status: BOOT-01 repository setup is complete locally; broader M0/M1 work remains planned. This backlog decomposes M0/M1 from the [project plan](README.md); later milestones retain their acceptance gates there.

## Bootstrap evidence — 2026-10-07

- Git initialized on main, origin set to https://github.com/FunToHard/view.git; no commit or push performed during setup.
- Cargo workspace initialized with the unpublished view facade only; core/platform/testing packages will be added when implementation needs them.
- Rust 1.98.1 pinned and declared as the bootstrap minimum; edition 2024, resolver 3, shared lints, lockfile, both license texts and contributor/agent guidance added.
- Local Windows formatting, check, Clippy, test and rustdoc commands passed. The scaffold has zero tests and no runtime API; this is not behavioral conformance evidence.
- No third-party dependency family, independent package consumer, clean-checkout build, Linux/macOS runner, GPU or native window has been qualified yet. BOOT-01's clean-checkout evidence remains open until a committed checkout exists; BOOT-02 through BOOT-04 and all RUN items remain planned.

## Work items

Track current completion in the [ordered implementation checklist](implementation-checklist.md).
This document retains BOOT/RUN task definitions and the bootstrap evidence snapshot;
do not maintain a second independent completion status here.

| ID | Deliverable | Depends on | Completion evidence |
| --- | --- | --- | --- |
| BOOT-01 | Initialize Git/Cargo workspace with facade/core/platform/testing packages as needed; toolchain, edition/resolver, license files and contributor commands | Start implementation phase | Clean-checkout format/check/test works; metadata matches the selected baseline; no empty optional packages |
| BOOT-02 | Qualify dependency versions, features, compiler/MSRV and target support; record backend/text compatibility | BOOT-01 | Reproducible locked build, dependency inventory and standalone consumer checks; record blockers rather than silently substitute libraries |
| BOOT-03 | Establish GitHub Actions CPU jobs on all three OSes and artifact retention | BOOT-02; repository/runner availability | Logs show tests actually executed per OS; missing native/GPU coverage explicitly marked |
| BOOT-04 | Establish owned fixture manifest and required Windows 11 interactive/GPU runner specification | BOOT-01 | Fixture provenance recorded; environment requirements and provisioning status visible; no unprovisioned hardware reported as tested |
| RUN-01 | Implement generational identity, mount/unmount ownership and typed action queue | BOOT-02 | Stale handles cannot act on reused slots; disposed owners cannot publish late updates; event ordering is deterministic |
| RUN-02 | Implement revisions, dirty propagation, pure build/measure boundaries and demand scheduling | RUN-01 | One action produces coherent observation; repeated measure cannot repeat an effect; idle runtime has no continuous rebuild loop |
| RUN-03 | Implement minimal constraints/layout, focus and input routing against committed geometry | RUN-02 | Fixed analytic geometry fixtures cover targeting and focus; later geometry-sensitive events see required committed changes |
| RUN-04 | Integrate native decorated Windows window and adapter lifecycle | BOOT-02; RUN-01 | Windows 11 open/resize/close smoke with correct thread/lifetime handling; capability limits reported |
| RUN-05 | Add controlled clock, tree/semantic snapshots and deterministic test driver | RUN-01 and RUN-02; grows with RUN-03 | Runnable identity/lifecycle/action/layout scenarios; inspector observes committed revision, not partially updated state |
| RUN-06 | Combine native shell and harness into a minimal inspectable application | RUN-03 through RUN-05 | Keyed identity/focus, queued action, snapshot, unmount and idle behavior demonstrated; native and headless evidence distinguished |

BOOT-02 qualifies the initial runtime/platform dependencies directly. Small disposable compatibility probes may qualify the selected graphics/text family without implementing M2/M3; keep their findings and remove scratch probes when superseded by real integration tests. Full conformance remains in the owning milestone. BOOT-03 can run core tests while BOOT-04 tracks hardware provisioning, but absent required native evidence keeps its gate open.

## Ordering and review

Use short-lived branches per bounded work item. Land core contracts and their meaningful tests together; no permanent platform branches. One maintainer can execute this sequence without parallel development. Review RUN-06 before expanding the public API into controls, graphics, webviews or add-ons.

T-01 through T-05 apply progressively at the layers implemented here. T-39 package/feature checks begin in BOOT-02 and grow with the workspace. These IDs refer to [validation scenarios](../design/08-delivery-and-validation.md); completing an early subset does not certify the entire final scenario.

Track each item as planned, in progress, blocked with a concrete dependency, or complete with evidence links. Assign people and estimates when staffing is known. Setup created the local main branch but no external issues, cloud resources or remote commits.
