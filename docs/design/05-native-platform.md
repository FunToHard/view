# Native platform integration

Status: selected native-platform baseline, including optional webview adapter. Revision: 0.4.

## Platform policy

Windows is the first supported product target. Cross-platform abstractions must expose platform capabilities and differences instead of approximating every system with the lowest common denominator.

Default windows use native decorations and native move/resize behavior. The renderer owns client content. Custom title-bar content is an explicit, capability-checked option using documented platform facilities. Removing decorations and simulating an entire window frame is not an acceptable default implementation shortcut.

Microsoft documents DWM frame integration and caption hit testing, including delegation to DwmDefWindowProc. This is evidence that native integration paths exist, not proof that every desired title-bar design works identically across Windows versions. [DWM custom frames](https://learn.microsoft.com/en-us/windows/win32/dwm/customframe)

## Adapter structure

Use a platform-neutral runtime with winit as the baseline window/event provider and targeted Rust Windows bindings for native features it does not expose. Qualify the adapter against native behavior tests before claiming support. A discovered blocker requires a recorded baseline change; it is not a reason for an undocumented workaround.

Winit exposes Windows-specific event-loop extension points; verify their exact coverage in the pinned version before relying on them. Never assume that an event-message hook alone provides a complete native window-procedure integration. [Winit Windows extensions](https://docs.rs/winit/latest/winit/platform/windows/trait.EventLoopBuilderExtWindows.html)

Platform calls, thread affinity, HWND lifetimes, COM initialization, callbacks, and any unsafe code are contained in audited adapter modules. A raw-handle API includes validity and thread rules. Supported advanced integration should not require users to patch private internals.

## Windows acceptance contract

| Area | Behavior to preserve and verify |
| --- | --- |
| Window frame | Caption controls, resize borders, maximize/restore, system menu, native dragging, snap behavior |
| Activation | Alt-Tab, focus restoration, owned/modal windows, minimize/restore, close requests |
| DPI | Per-monitor changes, logical/physical conversion, text scale, mixed-DPI movement |
| Input | Pointer/keyboard, capture loss, IME candidate positioning, composition, wheel/precision scrolling |
| Services | Clipboard, drag-and-drop, file dialogs, shell/file activation as scoped |
| Accessibility | Native accessibility bridge, focus, semantic actions, text/value exposure |
| Rendering lifecycle | Occlusion, resize, zero size, surface loss, sleep/resume, display change |

Optional title-bar content must still pass the applicable frame tests, including native caption affordances on supported OS versions. If the adapter cannot provide that behavior, report the limitation and use the native frame rather than silently substituting imitation controls.

Native widgets/child windows and GPU composition have separate z-order, clipping, focus, and accessibility concerns. Embedded web/native views need an explicit capability contract; arbitrary mixing is not assumed to be solved by obtaining an HWND.

The storefront proof uses native view components and does not require embedded web content. Optional rectangular Windows WebView2 panels are now selected independently, with [DisposeAfterUse and KeepHot](15-webview-lifecycle.md). Arbitrary transformed/native view composition remains deferred. OS input testing is an optional external driver with explicit session ownership and native restrictions, specified in [automation](06-automation-and-tooling.md).

## First-class advanced input

Creative applications need pressure/tilt where available, pointer capture, relative motion/cursor control for viewports, drag cancellation, and command routing. Discover these requirements early; their release priority depends on the selected proof application and hardware.

Text input is delivered through the platform text/IME path, not reconstructed from physical key presses. Scene shortcuts must not steal composition text or ordinary editing shortcuts from a focused field.

## Linux/macOS policy

Compile and execute core tests on actual Linux/macOS runners. Add adapter smoke and offscreen rendering tests where the environment supports them. A successful cross-compile is not a runtime test. A headless job is not evidence for window-manager, compositor, IME, or accessibility integration.

Keep explicit capability/coverage tables for each platform. A missing feature may return Unsupported; it must not corrupt unrelated functionality. Native support promotion requires platform-specific interactive validation beyond CI compilation.

The decision register selects Windows 11 x64 and GitHub Actions as delegated planning defaults. Actual runner images, display setup and graphics adapters are provisioning/qualification tasks in the project plan; no desktop or GPU availability is assumed from a runner label. Windows hosted build jobs do not substitute for the Windows 11 product validation session.

## Deferred web platform profile

If pursued, web targets browser services and a canvas, not the desktop adapter. Share models, identity, layout, semantics, and compatible graphics; adapt scheduling, fonts/assets, network, text input, storage, and accessibility. Do not expose HWNDs, global desktop input injection, or native crash dumps as portable facilities. Do not restrict native primitives to ensure browser compatibility. A WASM experiment and browser support matrix are decisions separate from initial Windows delivery; see [web portability](11-network-ui-and-web.md).

## Integration decision gate

Qualify the selected windowing adapter against native frame behavior; two windows and modal ownership; DPI transition; IME; accessibility; host-owned loop integration; and backend surface lifetime. Qualify title-bar extension separately if its deferred scope is reopened. Document unsupported cases and dependency cost; a blocker triggers a recorded decision amendment. This future experiment requires implementation authorization; no platform code is introduced by this design set.
