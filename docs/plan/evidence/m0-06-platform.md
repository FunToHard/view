# M0-06 Platform Dependency Qualification Evidence (Windows)

Date: 2026-10-07

Target: `x86_64-pc-windows-msvc` (Windows 11 x64)

Toolchain Baseline: `rustc 1.98.1 (48a229cea 2026-09-01)` / `cargo 1.98.1 (797e8a9bc 2026-08-05)`

Status: Passed local qualification probe (compile and layout verification)

---

## 1. Scope and Strategy

In accordance with `AGENTS.md`, `docs/design/05-native-platform.md`, and `docs/design/09-decisions-and-questions.md`:
- The public facade crate [view](../../../crates/view/Cargo.toml) remains dependency-free. Core runtime packages will be implemented in M1.
- Windows windowing and OS bindings are qualified inside a dedicated development package: [platform-probe](../../../crates/platform-probe/Cargo.toml), publish=false.
- Selected dependency minimum versions/features are centralized in workspace.dependencies; Cargo.lock fixes the actual resolved versions. The manifest uses compatible ranges, not exact requirements.
- Platform dependencies in `crates/platform-probe` are target-gated under `[target.'cfg(windows)'.dependencies]`, and source files are guarded with `#[cfg(windows)]`. On non-Windows hosts, the package compiles cleanly and reports that the Windows-specific probe was skipped.
- Win32 and DWM API contracts are validated via compile-time typed function pointer references and struct layout/alignment checks. **No runtime calls with invalid/null `HWND` or fake parameters are made to the OS.**
- Native OS window behavior (caption dragging, snap layouts, Alt-Tab activation, DPI scaling) remains untested at this stage; it will be qualified during Phase 2 native shell integration.

---

## 2. Upstream Dependency Audit

### 2.1. `winit`

- **Resolved Version**: `0.30.13` (manifest declared minimum: `0.30.13`).
- **Upstream Repository**: <https://github.com/rust-windowing/winit>
- **Crates.io**: <https://crates.io/crates/winit/0.30.13>
- **License**: `Apache-2.0` (verified from downloaded crate manifest).
- **MSRV**: `1.70.0` (verified from manifest `rust-version = "1.70.0"`; toolchain is `1.98.1`).
- **Maintenance Evidence**: [upstream commit snapshot](upstream-maintenance.json) records the primary repository's observed revision/date. The selected stable 0.30 API is shared by AccessKit's winit adapter. Recent commits are activity evidence, not proof of correctness or a release maintenance guarantee.
- **Features & Target Resolution**:
  - Configured with `default-features = false, features = ["rwh_06"]`.
  - Upstream manifest gates X11 (`x11rb`) and Wayland (`sctk`, `wayland-client`) dependencies behind `cfg(all(unix, not(target_os = "android"), ...))`. On Windows (`x86_64-pc-windows-msvc`), Cargo's target resolution ignores these Unix dependencies entirely. Disabling default features further prevents pulling unnecessary optional crates across cross-compilation configurations.
  - Activates `raw-window-handle` 0.6 interoperability.
- **Direct Dependencies on Windows**:
  - `bitflags v2.13.2`
  - `cursor-icon v1.2.0`
  - `dpi v0.1.2`
  - `raw-window-handle v0.6.2`
  - `smol_str v0.2.2`
  - `tracing v0.1.44`
  - `unicode-segmentation v1.13.3` (gated to `cfg(target_os = "windows")` in winit manifest)
  - `windows-sys v0.52.0` (gated to `cfg(target_os = "windows")` in winit manifest)
- **Known Risks**:
  - `winit`'s event loop seizes main-thread ownership in standard GUI mode. In non-GUI worker threads or headless test fixtures, `EventLoopBuilderExtWindows::with_any_thread(true)` must be explicitly enabled.

### 2.2. `windows` (Official Microsoft Bindings)

- **Resolved Version**: `0.62.2` (manifest declared minimum: `0.62.2`).
- **Upstream Repository**: <https://github.com/microsoft/windows-rs>
- **Crates.io**: <https://crates.io/crates/windows/0.62.2>
- **License**: `MIT OR Apache-2.0` (verified from downloaded crate manifest; matches view license).
- **MSRV**: `1.82` (verified from manifest `rust-version = "1.82"`; toolchain is `1.98.1`).
- **Maintenance Evidence**: official Microsoft repository and [observed upstream revision](upstream-maintenance.json). No specific update cadence or absence of bugs is inferred.
- **Selected Features** (strictly minimized):
  - `Win32_Foundation`: `HWND`, `LRESULT`, `WPARAM`, `LPARAM`, `RECT`.
  - `Win32_UI_WindowsAndMessaging`: Window procedure function `DefWindowProcW`.
  - `Win32_Graphics_Dwm`: DWM APIs `DwmDefWindowProc`, `DwmExtendFrameIntoClientArea`, `DwmSetWindowAttribute`, and constant `DWMWA_USE_IMMERSIVE_DARK_MODE`.
  - `Win32_UI_HiDpi`: `GetDpiForWindow`.
  - `Win32_UI_Controls`: System structure `MARGINS`.
- **Direct & Transitive Dependency Costs on Windows**:
  - Direct dependencies in the downloaded windows manifest: windows-core, windows-collections, windows-future and windows-numerics.
  - Transitive support includes windows-result/strings/link/threading and proc-macro dependencies such as windows-implement/interface, syn, quote and proc-macro2.
  - Reproduce the isolated graph with `cargo tree -p platform-probe --locked --target x86_64-pc-windows-msvc`. The later graphics probe expands the workspace graph; current counts and duplicate families are recorded separately in [M0 qualification](m0-qualification.md).

---

## 3. Qualification Probe Implementation

Package: [platform-probe](../../../crates/platform-probe/Cargo.toml), publish=false.

### Tested Contracts:
1. **Raw Window Handle Interoperability**:
   - `raw_handle_to_hwnd`: Pure arithmetic extraction of typed `HWND` from `winit::raw_window_handle::RawWindowHandle::Win32`.
   - `hwnd_to_raw_handle`: Pure arithmetic reconstruction of `Win32WindowHandle` from `HWND`.
   - Validated via unit test round-trip using mock non-zero address (no OS calls).
2. **Compile-Time Win32 / DWM Typed Signatures (Uncalled)**:
   - `assert_win32_signatures_compile`: Type-checks the Rust binding function signatures for DefWindowProcW, DwmDefWindowProc, DwmExtendFrameIntoClientArea, DwmSetWindowAttribute and GetDpiForWindow without invoking native functions. This is not independent certification of the generated FFI calling convention.
3. **Data Structure Layout & Constant Validation**:
   - `check_layout_and_constants`: Validates byte sizes and alignments against Win32 ABI:
     - `MARGINS`: size = 16 bytes, align = 4 bytes.
     - `RECT`: size = 16 bytes.
     - `HWND`: size = pointer size (8 bytes on x86_64).
     - `DWMWA_USE_IMMERSIVE_DARK_MODE.0 == 20`.
4. **winit EventLoopBuilder Windows Extensions**:
   - `check_winit_event_loop_builder`: Probes `builder.with_any_thread(true)` from `EventLoopBuilderExtWindows`.
5. **Multi-OS CFG Isolation**:
   - `crates/platform-probe/Cargo.toml` gates platform dependencies to `cfg(windows)`.
   - `src/lib.rs` and `src/main.rs` isolate Windows implementations behind `#[cfg(windows)]`. On non-Windows hosts, probe stubs explicitly report that the Windows probe was skipped, exiting with status code 2.

---

## 4. Verification Commands and Results

All commands executed from repository root `F:\DEV\ui\view`:

| Command | Exit Code | Result | Notes |
|---|---|---|---|
| `cargo fmt --all -- --check` | 0 | **PASS** | Formatted per workspace rules |
| `cargo check --workspace --all-targets --locked` | 0 | **PASS** | Zero errors, zero warnings |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 0 | **PASS** | Zero lints, workspace rules satisfied |
| `cargo test --workspace --locked` | 0 | **PASS** | 4 unit tests passed in `platform-probe` |
| `cargo doc --workspace --no-deps --locked` | 0 | **PASS** | Documentation builds cleanly |
| `cargo run -p platform-probe` | 0 | **PASS** | Standalone binary reports 3/3 checks passed |

### Test Execution Output:
```text
running 4 tests
test tests::test_compile_time_win32_signatures ... ok
test tests::test_event_loop_builder_with_any_thread ... ok
test tests::test_handle_conversion_round_trip ... ok
test tests::test_platform_type_layout_and_constants ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### Standalone Executable Output:
```text
============================================================
view - M0-06 Platform Dependency Qualification Probe (Windows)
============================================================
Host OS:           windows
Target Arch:       x86_64
Recorded baseline: winit 0.30.13, windows 0.62.2, rustc 1.98.1
(Refer to Cargo.lock and `rustc --version` for active metadata)
------------------------------------------------------------
[PASS] Win32 / DWM typed API function signature verification
[PASS] Win32 / DWM type layout and constants check
[PASS] winit Windows EventLoopBuilder extension check
------------------------------------------------------------
[SUCCESS] Platform probe completed all verification checks.
Note: Native OS window lifecycle behavior remains untested.
```

---

## 5. How to Rerun

```powershell
# Run the platform qualification probe
cargo run -p platform-probe

# Run all qualification tests
cargo test -p platform-probe --locked
```

---

## 6. Limitations and Exclusions

- **Compile and Layout Only**: This probe establishes that `winit 0.30.13` and `windows 0.62.2` build cleanly under Rust 1.98.1 with verified struct layouts and uncalled type signatures.
- **Native OS Window Behavior Untested**: No native OS window was created. DWM frame hit-testing, Snap Layout integration, caption buttons, Alt-Tab window switching, DPI change event delivery, and message pumps remain untested. These will be qualified in M1 Phase 2 (`view-platform` native shell).
- **Public Facade Untouched**: `crates/view` remains an empty facade crate without third-party dependencies.
