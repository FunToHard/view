# M5 automation implementation and evidence

Date: 2026-10-21. Windows x64 local execution. The canonical status is the
[implementation checklist](../implementation-checklist.md); no remote CI or
Linux/macOS runtime execution is inferred from these results.

## Implementation mapping

| Items | Implementation and evidence |
| --- | --- |
| M5-01–03 | `view-automation`: protocol v1, session/generational targets, scopes, exact selectors, coherent snapshots, bounded predicates, controlled clock and explicit presentation boundaries; geometry/stale/ambiguity/timeout tests |
| M5-04–05 | Typed schemas, owner-thread harness actions, request retention/deduplication, bounded workers with progress/cancellation and terminal results; Rust and subprocess tests exercise undo and retry |
| M5-06–07 | Explicit local stdio attachment and optional rmcp stdio; bounded framing and separate diagnostics; actual MCP initialize/list/call subprocess negotiation, discovery/error/edit/undo/background-result parity |
| M5-08–09 | Public snapshot HTML inspector with bounds/clip/verified-hit overlays and why-updated trace; explicit capture receipts, bounded bundles and fixture-boundary replay; automation-enabled DX12 capture proof passed |
| M5-10–12 | Opt-in Windows driver, cross-process ownership, foreground/process checks, F12 stop, deadline watchdog, held-input cleanup, current DPI/screen conversion; host realization callback is labeled semantic setup |
| M5-13–14 | Analytic clipped/offscreen/occluded/virtual/reused-target and fractional-DPI tests; physical single-desktop editor/storefront/IDE and modal flow; broader hardware/control matrix remains incomplete |
| M5-15 | Named en-US physical path passed. Named Japanese native composition runner implemented; Japanese input method unavailable on this desktop, explicitly uncovered |
| M5-16 | Rust/local CLI/actual MCP parity and separately labeled physical selection/gizmo/edit/undo workflow; actual GPU output/capture uses the existing proof renderer |
| M5-17–18 | Passive mode, simulated interruption/focus loss/timeout and owned-input cleanup tests; real restricted-input failure retained. Complete native AT, mixed-monitor and standard-control certification remains open |

## Executed native evidence

The user explicitly selected this desktop for native tests. Ordinary builds/tests
never seize input. The first restricted process reached the foreground test window
but `SendInput` rejected insertion; `m5-native-restricted.txt` retains the failure.
The same authorized test ran outside the execution sandbox with all guards intact.

The first unrestricted workflow exposed an ordering failure: wheel delivery used
the old search-field cursor position before the move was observed by the app.
The runner now requires observed pointer movement before the wheel group. The
failure is retained in `m5-native-wheel-failure.txt`; it was not counted as a pass.

The corrected run passed physical hierarchy selection, gizmo drag to X=0.5,
Ctrl+Z restoration, Right Arrow edit, Tab/reverse Tab, Escape, en-US key-to-text,
selection/backspace, separately labeled Unicode insertion, asynchronous catalog,
wheel-driven virtual rows, IDE edit/undo and modal Tab containment/Escape focus
restoration. Active target keyboard layout was `0x4090409`. The wheel event was
observed at logical `(370,330)`, delta `(0,-24)`, consuming `(0,24)`; the test did
not assign a scroll offset. Final native report: `target/verification/m5-native.txt`.

The named Japanese command exited nonzero before input with an uncovered receipt
(`m5-japanese-ime.txt`). Installed layouts were `04090409`, `f0a90445`, `40094009`;
the user-language query reported en-US, en-IN and bn-IN. No Japanese IME was installed
or activated. Candidate rectangle publication is implemented; this is not native
preedit/conversion/commit evidence. Linux/macOS native, mixed-monitor hardware,
precision touchpad, real native AT client and full control-profile certification
remain uncovered. These dependent gates must not be checked as passed.

## Verification

Focused core/automation/tooling tests, all-feature Clippy and independent automation
consumer checks passed during implementation. The actual rmcp process test covers
negotiation, discovery, edit, duplicate request, stale undo, successful undo and
background results. Analytic geometry expectations are independent constants;
they do not call the locator to calculate expected positions. The disabled-subtree
regression checks semantics, focus removal, capture cancellation and no click-through.
Final automation/tooling run passed 13 behavior tests, including the actual MCP
subprocess exchange, inspector hit-point/escaping checks, oversized worker result
rejection and replacement of an existing capture at capacity. Request and response
bytes both count toward retention; oversized input is rejected before dispatch.

The three-window automation GPU proof passed on NVIDIA GeForce RTX 3050 6GB Laptop
GPU, DX12, driver 32.0.16.1714. Each actual readback contains 800 × 600 × 4 bytes;
receipts report `offscreen_readback`, content revision and measured capture cost.
This does not claim compositor observation. Artifacts are in `target/verification/`:
`m5-gpu-run.log`, `m5-gpu-bundle.json` and `m5-inspector.html`.

The full feature/independent-consumer run passed its behavior checks and then found
an unformatted new independent-consumer test. That formatting was corrected and
the independent workspace's formatting check passed. The original nonzero run is
retained in `m5-full.log`; remaining verification commands are recorded separately
in `m5-verification-tail.log`. Final workspace checks are in
`m5-final-workspace.log`; final automation/MCP tests in `m5-final-tooling.log`.
The final required workspace fmt, all-target check, Clippy with warnings denied,
tests and docs all passed. The resumed verification tail exited successfully:
dependency inventory, platform probe, hidden application surfaces, DX12 dependency/
offscreen/scene/embedded/gallery proofs and hidden gallery/surface lifecycle checks.
The gallery verified AccessKit installation/update and candidate geometry service
calls, explicitly without a native accessibility client or IME composition claim.
Local links and balanced fences in the eight affected M5 documents passed, as did
`git diff --check`. Generated evidence remains local and is not committed or uploaded.

Open acceptance items remain explicit: M5-13 needs the full native movement/mixed-DPI
matrix; M5-14 needs the remaining page-navigation/control flows; M5-15 needs an
installed and selected Japanese IME; M5-17 needs complete native closure/interruption
coverage; M5-18 needs real native AT and full standard-control profile observations.
The current mock cleanup and headless geometry tests do not substitute for those runs.

## Dependencies and decisions

Core/facade minimal dependency trees remain `view -> view-core`. Serialization
is confined to optional automation; async/transport dependencies to optional
tooling MCP. No HTTP/TLS/auth-server features are enabled. New first-party packages
are unpublished MIT OR Apache-2.0 packages. Lockfiles are updated together.

rmcp is pinned to 3.6.0 (Apache-2.0, declared MSRV 1.88), qualified on the pinned
Rust 1.98.1 toolchain with `server,transport-io` and default features disabled.
It is the maintained [official Rust SDK](https://github.com/modelcontextprotocol/rust-sdk).
Qualification uses compilation and a real stdio process exchange, not a claim of
bug-free behavior or selection solely by release recency. Selected resolved versions
include serde 1.0.229 (MIT OR Apache-2.0), serde_json 1.0.154 (MIT OR Apache-2.0) and
tokio 1.53.2 (MIT). The initial resolution added 26 registry packages for optional
tooling; Cargo metadata/inventory records actual target-specific cost and licenses.
These libraries support portable CPU tooling; this increment executed Windows only.

Native insertion obeys the documented [SendInput restrictions](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput).
The public runtime gains capture/geometry diagnostics and effective enabled state
because locators and native controls require consistent input eligibility.
Disabled content remains visually occluding; it is not made transparent to clicks.
The inspector consumes the same public protocol, not a privileged tree API.

Contracts and runnable commands: [automation guide](../../guides/automation.md).
