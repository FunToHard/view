# Expansion evaluation on the current Windows machine

Date: 2026-10-25. The owner explicitly included feasible Later-roadmap work.
This evaluation records implementation boundaries and local experiments; it does
not rename unavailable platform certification as a passed gate.

## Executed target experiment (F-11)

The installed pinned Rust toolchain includes `wasm32-unknown-unknown`,
`wasm32-wasip2`, `x86_64-pc-windows-msvc` and `x86_64-unknown-linux-musl`.
Both commands below passed on this machine:

```text
cargo check -p view --no-default-features --target wasm32-unknown-unknown --locked
cargo check -p view-core --target wasm32-wasip2 --locked
```

The minimal facade/core can compile as a WebAssembly capability subset. This is
not browser UI execution. The native shell, native clipboard/IME, desktop
automation, WebView2 and native-process add-ons do not acquire browser equivalents
from this compile result. A browser application needs an event-loop/DOM input
adapter, accessible text bridge, web graphics feature selection, async services
and its own T-20 fixtures. Rust's target documentation explicitly distinguishes
available language support from unsupported filesystem/thread operations:
[wasm32-unknown-unknown](https://doc.rust-lang.org/rustc/platform-support/wasm32-unknown-unknown.html).
Decision: retain a documented buildable core subset and defer full web delivery
until these adapters and browser execution exist; native capabilities remain intact.

## Runtime loaders and stateful reload (F-09/F-10)

The existing `view-addons` boundary serializes versioned JSON and runs a trusted
child executable. Core state is concrete Rust data behind owner-thread callbacks;
window, node and request identities include lifetime/generation information.
Passing these live objects across an independently compiled DLL would need a new
C ABI, allocator ownership, callback-thread and unload protocol. A WASM interpreter
would additionally need resource metering, host capability grants and callback
scheduling. Neither is implied by the current process backend or by compiling
core to WASM. No interpreter or unsafe DLL loader was added to the minimal graph.

Decision: keep the executable-process backend as the supported extension profile.
Any future C ABI must use opaque handles, fixed-width versioned records, explicit
create/drop functions and host-owned errors; never export Rust trait objects,
references or standard containers. Stateful code reload must first serialize
application data to a versioned format, cancel/revoke old callbacks and requests,
construct new runtime ownership and explicitly migrate that data. Existing
application checkpoint recovery and docking snapshots are useful data boundaries,
not live-code migration. Rust layout guarantees are specified by representation:
[Rust type layout](https://doc.rust-lang.org/reference/type-layout.html).

## Additional products and native composition (F-07/F-08/F-12)

Windows 10, ARM64/mobile, Linux and macOS native certification require their
actual environments. Cross-compilation on this machine cannot close native
input/accessibility/distribution gates. The current Windows window retains native
decorations; custom title-bar content remains contingent on the full native
caption/snap/system-menu/DPI/AT contract. Arbitrary external D3D/Vulkan/Metal
resources and transformed child HWND composition require explicit resource
ownership/fence/format tests and backend-specific adapters. Existing same-device
wgpu embedding remains supported. These requirements are still open, not silently
implemented by exposing a raw pointer or replacing native decoration.

## Crash helpers and optional architecture changes (F-13/F-14)

The new independent failure supervisor can detect/kill/reap owned disposable
children and retain evidence. It does not collect production hang dumps or
monitor arbitrary applications. The selected diagnostic integration generates
reviewable per-executable WER setup without changing the registry. Microsoft
documents compatibility restrictions for custom crash reporters:
[WER LocalDumps](https://learn.microsoft.com/en-us/windows/win32/wer/collecting-user-mode-dumps).
Decision: keep WER host-controlled; a production helper would need an isolated
process protocol, target identity validation, ACL policy, crash-loop bounds and
separate collection qualification before being selected.

Current measured resize/edit CPU time is dominated by surface acquisition and
submission. This does not establish a benefit for a mandatory DSL, macro expansion,
render thread, damage redraw or typed branch specialization. Retain ordinary Rust
builders and the existing scheduling contract. Grid/CSS layout and other optional
authoring extensions need their own requested layout semantics and comparative
fixtures; no new maintenance cost is justified by this measurement alone.

These evaluation outcomes leave the corresponding backend/product implementations
explicitly unsupported. The active feasible expansion implementations (passwords,
docking data/runtime integration, large documents, image effects/export and scene
assets/materials) have separate code and execution evidence.
