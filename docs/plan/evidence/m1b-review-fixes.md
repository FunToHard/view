# M1B review corrections — 2026-10-08

The review found ten actionable issues in the uncommitted M1B implementation.
Seven headless reproductions initially failed despite the existing suites passing.
This correction record supersedes the original report's blanket completion claim;
it certifies the fixes below, not every native acceptance scenario.

| Finding | Correction and evidence |
| --- | --- |
| Empty intersection became unrestricted | Empty clips retain a zero-area rectangle and reject all points |
| Parent layout origins omitted | Node-local bounds begin at zero; arranged origins enter the ancestor transform chain |
| Parent/child composition reversed | Child-local transformations apply before accumulated parent transformations |
| Scroll offsets ignored by geometry | Descendants share scroll translation and viewport clipping in layout and hit traversal |
| Keyboard shortcuts bypassed modal scope | Shortcut and focused targets require live same-window ownership, effective visibility and modal containment |
| Semantics exposed pending edits | Semantic records are stored in committed node snapshots, including values/actions/focus/effective visibility |
| Linux platform build relied on probe features | view-platform directly enables Linux X11/Wayland/dlopen features, matching the qualified probe stack |
| Native callbacks could fail with exit success | First callback failure terminates the loop and is returned from PlatformApp::run |
| Hidden ancestor children received focus | Focus traversal checks effective visibility through the owner chain |
| Frame origin used as client origin | Native inner_position supplies client origins at creation, movement and DPI change |

## Verification

`pwsh -File scripts/verify.ps1` passed locally on Windows x64, Rust 1.98.1:
formatting, all-target locked check, strict Clippy, workspace tests, rustdoc,
isolated package tests, independent consumer profiles, fixtures and inventory.
The workspace ran 71 unit/integration tests plus three compile-fail doctests;
the separate consumer ran one integration test.

The seven [public API regressions](../../../crates/view-core/tests/m1b_regressions.rs)
all pass. The Windows hidden inspectable app additionally compares its stored
client origin and client-zero screen conversion with the actual native inner
position. The [callback failure example](../../../crates/view-platform/examples/callback_failure.rs)
runs three separate hidden processes (ready/event/idle), each requiring the exact
injected error to return from `run`; swallowed errors make the fixture fail.

The CPU script executes native smoke only on Windows. Linux/macOS run the same
headless suites and isolated platform compilation/tests, with no implicit display
server requirement. This is not Linux/macOS native-window certification. Local
tests injected no keyboard/mouse input, and no rendered frame, mixed-monitor
movement or IME certification is claimed. M0 remains outside this change.

## CI

[CPU qualification run 37761330873](https://github.com/FunToHard/view/actions/runs/37761330873)
passed on Windows 2025, Ubuntu 24.04 and macOS 15 for implementation commit
`d7add724e0fd5363ceb9b23d90fcf7ef994e1b1e`. All three jobs ran the complete
verification script, including the seven regression tests and independent
consumer. Windows also passed the hidden inspectable app and all three callback
failure checks. [Runner environments](m1b-cpu-environments.json) preserve the
actual OS, architecture and compiler metadata.

The first run exposed an unconditional Windows COM test import under strict
Clippy on Linux/macOS. The import is now Windows-gated; the full local script
passed again before the successful CI run. This evidence covers CPU/headless
execution across all three platforms and hidden native Windows checks, with the
native/GPU limitations above unchanged.
