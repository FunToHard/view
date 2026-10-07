# Source ledger

Checked during planning on 2026-10-07. These are primary references. They inform the proposals; they do not certify dependencies as bug-free, prove performance, or establish that version combinations have been tested together.

| Reference | Relevance | Follow-up before implementation |
| --- | --- | --- |
| [Flutter architecture](https://docs.flutter.dev/resources/architectural-overview) | Composition, immutable descriptions, persistent runtime objects | Translate ergonomics to Rust ownership; do not copy runtime assumptions |
| [wgpu Device](https://docs.rs/wgpu/latest/wgpu/struct.Device.html) | Device features/resources and backend boundary | Pin selected version and test host ownership |
| [wgpu Texture](https://docs.rs/wgpu/latest/wgpu/struct.Texture.html) | Texture metadata and resource lifetime | Validate formats/usages, sampling, retirement, and interop assumptions |
| [wgpu Surface](https://docs.rs/wgpu/latest/wgpu/struct.Surface.html) | Surface configuration/acquisition | Verify selected version's resize/error/recovery behavior |
| [wgpu ShaderSource](https://docs.rs/wgpu/latest/wgpu/enum.ShaderSource.html) | Shader input options including WGSL | Resolve Q-06 and choose the simplest supported toolchain |
| [Winit Windows event-loop extensions](https://docs.rs/winit/latest/winit/platform/windows/trait.EventLoopBuilderExtWindows.html) | Platform-specific integration points | Verify coverage of needed native message/window operations |
| [Winit control flow](https://docs.rs/winit/latest/winit/event_loop/enum.ControlFlow.html) | Waiting, timed wakeups, continuous loops | Reconcile host-owned mode and viewport pacing |
| [Microsoft DWM custom frames](https://learn.microsoft.com/en-us/windows/win32/dwm/customframe) | Documented frame integration and hit testing | Test current target Windows behavior, not just sample code |
| [Microsoft title-bar guidance](https://learn.microsoft.com/en-us/windows/apps/develop/title-bar) | Current platform customization guidance | Distinguish Win32 and Windows App SDK APIs/dependency costs |
| [COSMIC Text](https://github.com/pop-os/cosmic-text) | Rust text stack candidate | Complex scripts, font setup, editing and IME boundaries |
| [Taffy](https://docs.rs/taffy/latest/taffy/) | Layout algorithms and custom-tree integration | Compare with deliberately limited native constraint layout |
| [Glyphon](https://github.com/grovesNL/glyphon) | wgpu text integration candidate | Check matching wgpu/cosmic-text versions and cache ownership |
| [AccessKit](https://github.com/AccessKit/accesskit) | Semantic schema and native adapters | Test concrete widget/text/native integration support |
| [MCP Rust SDK](https://github.com/modelcontextprotocol/rust-sdk) | Optional Rust MCP transport implementation | Inspect dependency/features and supported protocol version |
| [Rust ABI](https://doc.rust-lang.org/reference/items/external-blocks.html#abi) | Rust ABI stability limits | Required if runtime binary plugins are pursued |
| [GitHub-hosted runners](https://docs.github.com/en/actions/reference/runners/github-hosted-runners) | Example hosted CI environments | Verify actual graphics/display availability and runner images |
| [Self-hosted runners](https://docs.github.com/en/actions/reference/runners/self-hosted-runners) | Example controlled hardware/session CI | Choose ownership, isolation, maintenance and cost policy |
| [wgpu backend overview](https://docs.rs/wgpu/latest/wgpu/) | WebGPU/WebGL2 and native graphics capability profiles | A WASM backend does not establish complete framework portability |
| [Winit web platform](https://docs.rs/winit/latest/winit/platform/web/index.html) | Canvas embedding, WASM target and coordinate caveats | Validate supported styling/input mapping if web is pursued |
| [MDN WebGPU](https://developer.mozilla.org/en-US/docs/Web/API/WebGPU_API) | Browser GPU capability and secure-context requirements | Detect actual browser/device capabilities; define target profile |
| [MDN Fetch](https://developer.mozilla.org/en-US/docs/Web/API/Fetch_API/Using_Fetch) | Browser request/cancellation/origin behavior | Keep transport policy distinct from UI state and revisions |
| [Rust panic hook](https://doc.rust-lang.org/std/panic/fn.set_hook.html) | Global panic reporting behavior | Explicit application ownership and reporting limits |
| [Rust catch_unwind](https://doc.rust-lang.org/std/panic/fn.catch_unwind.html) | Unwind-only panic boundary | Do not promise native-fault recovery or safe UI continuation |
| [WER local dumps](https://learn.microsoft.com/en-us/windows/win32/wer/collecting-user-mode-dumps) | Optional OS crash collection | Choose a supported arrangement; check custom-handler incompatibilities |
| [MiniDumpWriteDump](https://learn.microsoft.com/en-us/windows/win32/api/minidumpapiset/nf-minidumpapiset-minidumpwritedump) | Native dump collection and external-process recommendation | Reporter setup, symbols, permissions and failure handling |
| [MCP tools specification](https://modelcontextprotocol.io/specification/2026-07-28/server/tools) | Discoverable tool schemas/results | Application supplies real domain semantics and command policies |
| [Windows SendInput](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput) | OS mouse/keyboard injection and integrity/state constraints | Test on an explicitly provisioned desktop session |
| [Cargo publishing](https://doc.rust-lang.org/cargo/reference/publishing.html) | Ordinary community control distribution | Public API/version/feature compatibility and package checks |
| [Rustdoc tests](https://doc.rust-lang.org/rustdoc/write-documentation/documentation-tests.html) | Executable documentation examples | Separate future compile-tested examples from current API sketches |
| [Windows edit controls](https://learn.microsoft.com/en-us/windows/win32/controls/about-edit-controls) | Native text-control behavior reference | Define precise standard/specialized profiles rather than assume complete parity |
| [Edit-control text operations](https://learn.microsoft.com/en-us/windows/win32/controls/edit-controls-text-operations) | Selection/edit/clipboard operation reference | Test against explicit expected behavior and native fixtures |
| [Unicode segmentation](https://www.unicode.org/reports/tr29/) | Grapheme/word boundaries and tailoring | Pin text behavior data/version and define navigation/deletion policies |
| [Rust linkage](https://doc.rust-lang.org/reference/linkage.html) | Native executable/dynamic-library artifact options | Runtime loading is separate from JIT; define actual ABI |
| [Windows DLL usage](https://learn.microsoft.com/en-us/windows/win32/dlls/using-dynamic-link-libraries) | Native dynamic loading | Host lifetime, compatibility and trusted-code policy |
| [Wasmi](https://github.com/wasmi-labs/wasmi) | Rust WASM interpreter candidate without JIT | Evaluate ABI/features/resource limits and cost; no dependency chosen |
| [WIT reference](https://component-model.bytecodealliance.org/design/wit.html) | Component Model interface definitions | Do not assume every core-WASM runtime supports this model |
| [Cargo workspaces](https://doc.rust-lang.org/cargo/reference/workspaces.html) | Monorepo package/dependency configuration | Verify independent package builds and feature profiles |
| [Cargo resolver](https://doc.rust-lang.org/cargo/reference/resolver.html) | Resolver/version/feature behavior | Explicit workspace resolver and qualified toolchain |
| [Cargo SemVer](https://doc.rust-lang.org/cargo/reference/semver.html) | Public API compatibility | Coordinated releases, migrations and consumer checks |
| [Git submodules](https://git-scm.com/book/en/v2/Git-Tools-Submodules) | Alternative repository composition | Not selected for tightly coupled first-party packages |
| [WebView2 process model](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/process-model) | Shared environment/session/process ownership | Closing one panel cannot imply all shared processes exit |
| [WebView2 controller](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2controller) | Explicit Close and instance cleanup | Track references, completion and actual owned process lifetime |
| [WebView2 profile data](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/user-data-folder) | Stored browser data and session ownership | Never race active/shared profiles during ephemeral cleanup |
| [WebView2 suspend/resume](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2_3) | Suspension is different from hot retention | Do not silently suspend a KeepHot lease |
| [WebView2 distribution](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution) | Managed/runtime packaging options | Detect version/availability; do not assume installed runtime |
| [WebView2 Rust bindings](https://github.com/wravery/webview2-rs) | Selected optional COM binding family | Qualify compatible SDK/windows versions and native lifecycle |
| [lyon](https://github.com/nical/lyon) | Selected Rust path tessellation | Validate renderer feature/quality/resource requirements |
| [Unicode segmentation crate](https://github.com/unicode-rs/unicode-segmentation) | Selected editing-boundary implementation | Align Unicode/profile expectations and test tailoring |
| [bytemuck](https://docs.rs/bytemuck/latest/bytemuck/) | Selected GPU data-layout helper | Checked derives/layout tests instead of ad-hoc unsafe casts |

Dependency families are selected in revision 0.4, but no exact version set, lockfile, benchmark, build, or native experiment is established by this document. Latest-version links should be accompanied by pinned version references when implementation qualification and compatibility claims exist.
