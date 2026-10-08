# M1B Geometry, Layout, Input, Semantics and Native Shell Evidence

Date: 2026-10-08. Executed locally on Windows 11 x64 with pinned Rust 1.98.1.
The canonical [checklist](../implementation-checklist.md) tracks milestone progress.
This is the initial implementation report. Subsequent review found integration
defects despite these tests passing; the [review corrections](m1b-review-fixes.md)
supersede its blanket completion claim and record new regression/native evidence.

## Implementation and Acceptance Mapping

| Item | Implemented Contract | Evidence |
| --- | --- | --- |
| **M1B-01** | Geometry value types (`LogicalPoint`, `LogicalSize`, `LogicalRect`, `PhysicalPoint`, `PhysicalSize`, `PhysicalRect`, `DocumentPoint`, `DocumentSize`, `DocumentRect`, `ScaleFactor`, `Transform2D`), finite validation (`is_valid`, `new_checked`), epsilon comparison rules (`EPSILON = 1e-5`); screen coordinates strictly isolated to the platform boundary | [`crates/view-core/src/geometry.rs`](../../../crates/view-core/src/geometry.rs); [geometry contracts](../../../crates/view-core/tests/geometry_contracts.rs) (4 passed) |
| **M1B-02** | Layout constraints (`Constraints`), pure measure/arrange, `PaddingLayout`, `FlexLayout` (Row/Column with `MainAxisAlignment` & `CrossAxisAlignment`), `StackLayout`, and `LayoutCache` | [`crates/view-core/src/layout.rs`](../../../crates/view-core/src/layout.rs); [layout contracts](../../../crates/view-core/tests/layout_contracts.rs) (5 passed) |
| **M1B-03** | `ClipChain`, `TransformChain`, reverse paint-order front-to-back hit traversal, custom hit contracts (`BoundingBoxHit`, `EllipseHit`, `PassthroughHit`) | [`crates/view-core/src/hit_test.rs`](../../../crates/view-core/src/hit_test.rs); [hit test contracts](../../../crates/view-core/tests/hit_test_contracts.rs) (3 passed) |
| **M1B-04** | Ordered `PointerEvent` / `KeyboardEvent`, hover/press/cancel state transitions, pointer capture routing, pre-input flush contract ensuring committed geometry before hit-testing | [`crates/view-core/src/input.rs`](../../../crates/view-core/src/input.rs); [input routing contracts](../../../crates/view-core/tests/input_routing_contracts.rs) (3 passed) |
| **M1B-05** | `FocusManager`, Tab/Shift-Tab cycling, window shortcuts, modal precedence trapping, focus restoration on modal exit and unmount | [`crates/view-core/src/focus.rs`](../../../crates/view-core/src/focus.rs); [focus contracts](../../../crates/view-core/tests/focus_contracts.rs) (3 passed) |
| **M1B-06** | `ScrollState`, boundary chaining policy (`ScrollChaining::Chain` vs `Clamp`), keyboard scrolling, `reveal_rect`, and virtualized item realization (`VirtualScroll`) | [`crates/view-core/src/scroll.rs`](../../../crates/view-core/src/scroll.rs); [scroll contracts](../../../crates/view-core/tests/scroll_contracts.rs) (4 passed) |
| **M1B-07** | `SemanticNode`, stable roles (`Role`), names, values, actions (`SemanticAction`), incremental semantic snapshots and diff updates (`SemanticUpdate`) | [`crates/view-core/src/semantics.rs`](../../../crates/view-core/src/semantics.rs); [semantic contracts](../../../crates/view-core/tests/semantic_contracts.rs) (3 passed) |
| **M1B-08** | Introduced `view-platform` crate with winit 0.30 and targeted Windows bindings; `ComGuard` thread-affine COM STA initialization/uninitialization; safe HWND handle extraction | [`crates/view-platform/src/com.rs`](../../../crates/view-platform/src/com.rs), [`window.rs`](../../../crates/view-platform/src/window.rs); [platform contracts](../../../crates/view-platform/tests/platform_contracts.rs) |
| **M1B-09** | Native decorated window lifecycle (`PlatformWindow`, `WindowLifecycle`: Opened, Active, Inactive, Minimized, CloseRequested, Destroyed), event loop pumping via `PlatformApp` | [`crates/view-platform/src/window.rs`](../../../crates/view-platform/src/window.rs), [`app.rs`](../../../crates/view-platform/src/app.rs) |
| **M1B-10** | `DpiState` conversions (logical, physical, text-scale), client to virtual desktop screen mapping supporting negative desktop coordinates (e.g. secondary monitor at `x = -1920`) | [`crates/view-platform/src/dpi.rs`](../../../crates/view-platform/src/dpi.rs); `test_dpi_conversions_and_negative_screen_coordinates` (passed) |
| **M1B-11** | Native event translation (`translate_window_event`), focus loss button cancellation (`cancel_held_buttons`), cursor shape service (`CursorService`), IME candidate cursor area (`ImeService`) | [`crates/view-platform/src/events.rs`](../../../crates/view-platform/src/events.rs), [`services.rs`](../../../crates/view-platform/src/services.rs); tests passed |
| **M1B-12** | Multi-window ownership (`PlatformShell`), modal hierarchies (`set_modal` with cycle detection), close requests preserving application documents, restoration snapshots (`WindowStateSnapshot`) | [`crates/view-platform/src/shell.rs`](../../../crates/view-platform/src/shell.rs); tests passed |
| **M1B-13** | Teardown cleanup on deactivation/removal; safe routing of late events targeting destroyed or non-existent native windows returning `None` without panic | [`crates/view-platform/src/shell.rs`](../../../crates/view-platform/src/shell.rs); `test_late_event_routing_safety` (passed) |
| **M1B-14** | Minimal inspectable native application demonstrating queued action execution, keyed identity, unmount, coherent tree & semantic snapshot, and idle quiescence | [`crates/view-platform/examples/inspectable_app.rs`](../../../crates/view-platform/examples/inspectable_app.rs); passed with `--run-hidden` |
| **M1B-15** | Review of all M1 contracts; execution of analytic layout, input, focus, scroll, semantic, and native shell test suites | Complete verification suite passed via `scripts/verify.ps1` |

## Verification Suite Execution

The repository verification script `pwsh -File scripts/verify.ps1` executed:

```text
1. Fixture manifest verification: 2 fixtures passed.
2. cargo fmt --all -- --check: passed.
3. cargo check --workspace --all-targets --locked: passed.
4. cargo clippy --workspace --all-targets --locked -- -D warnings: passed (0 warnings).
5. cargo test --workspace --locked: passed (64 tests across workspace).
   - view-core: 44 tests (geometry, layout, hit-test, input, focus, scroll, semantics, runtime, foundations, arena) + 3 doctests.
   - view-platform: 8 contract tests.
   - view-testing: 3 driver tests.
   - platform-probe & dependency-probe: 6 tests.
6. cargo doc --workspace --no-deps --locked: passed.
7. cargo test -p view-core --no-default-features --locked: passed.
8. cargo test -p view-testing --no-default-features --locked: passed.
9. cargo test -p view-platform --locked: passed (8 passed).
10. cargo run -p view-testing --example headless --locked: passed (model=5, builds=2, commits=2, idle_polls=100).
11. cargo run -p view-platform --example inspectable_app --locked -- --run-hidden: passed.
    - [1/5] Queued action executed once: counter=1, committed=Some(Revision(3))
    - [2/5] Keyed identity and reconciliation: found keyed nodes item-a and item-b
    - [3/5] Unmount and structural reconciliation: item-b cleanly unmounted
    - [4/5] Coherent tree, semantic and platform window snapshots verified
    - [5/5] Idle confirmed: 0 redundant rebuilds or commits across 10 flushes
12. cargo check -p view (--no-default-features and --all-features): passed.
13. Independent facade consumer (tests/compatibility/facade-consumer): check, run, test (--features harness), and fmt passed.
14. Dependency inventory: 124 registry packages qualified.
15. platform-probe: Win32/DWM signatures, type layout, and EventLoopBuilder passed.
```

## Architectural Review and Safety Invariants

1. **Dependency Boundary**:
   - `view-core` remains strictly independent of backend, platform, and graphics crates (`#![forbid(unsafe_code)]`, zero platform dependencies).
   - Platform integration (`winit`, `windows`) is isolated inside `view-platform`.
   - Screen coordinates exist exclusively within `view-platform`'s `DpiState` conversions. Core works strictly in logical DIPs, physical pixels, and document coordinates.
2. **Unsafe and COM Isolation**:
   - `CoInitializeEx` and `CoUninitialize` are encapsulated in `ComGuard` with documented safety invariants and thread-affinity verification on Drop.
   - HWND extraction uses `winit::raw_window_handle::RawWindowHandle::Win32` with verified invariants.
   - All unsafe blocks adhere to workspace lints (`undocumented_unsafe_blocks = "deny"`).
3. **Decoupled Application State & Restoration**:
   - OS close requests (`WindowEvent::CloseRequested`) do not destroy application document state. The shell marks `WindowLifecycle::CloseRequested`, allowing application code to intercept, confirm with users, take restoration snapshots (`WindowStateSnapshot`), or gracefully exit.
4. **Late Event and Modal Safety**:
   - Events targeting closed/destroyed windows return `None` safely without panicking.
   - Modal child windows block parent pointer and keyboard input while permitting non-input lifecycle events.

## Limitations and Open Coverage

- **Platform Coverage**: Verified on Windows 11 x64 (primary product target). Non-Windows platforms (Linux/macOS) are abstracted via winit and compile without Win32 features, but native desktop frame certification on Linux/macOS requires dedicated runners.
- **Physical Desktop Input**: In automated test runs, desktop mouse/keyboard input was never seized (`--run-hidden` smoke execution). Physical foreground input certification remains scheduled for Phase 8 (M5).
- **Text Shaping and Rendering**: Text rendering and IME candidate window rendering certification remain open for M2/M3.
