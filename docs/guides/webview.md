# Optional WebView2 panels

`view-webview` contains portable lifecycle/bridge contracts. Its `native` feature
adds the Windows STA provider. The facade exports them as `view::webview` with
`webview` and enables the provider with `webview-native`. The default facade
still depends only on core. Linux/macOS CI is configured to execute provider
mocks; M6 execution evidence there is pending. Native hosting there is unsupported.

## Host integration

Keep one `Manager<WindowsProvider>` on the application's UI thread. Create it
after the event loop initializes, register retained `Arc<winit::window::Window>`
parents, and open a panel with a runtime-owned `Cancellation`, a placement,
page policy, profile and lifetime. Pump `tick` with monotonic elapsed time after
native callbacks return. Native callbacks queue observations and do not dispatch
application actions or start nested message loops.

`DisposeAfterUse` is the default. `finish` is app-declared work completion, not
navigation completion. `dispose` and shutdown revoke bridge authority, remove
event handlers, hide and Close the controller. Dedicated sessions remain counted
until their matching browser-group exit is observed. Ephemeral profile removal
runs on a worker after that receipt. Poll `receipt`; forget completed receipts
explicitly to free manager capacity. Drop is best effort and cannot certify
asynchronous cleanup.

`KeepHot` hides the existing controller, retaining page state without reloading
or calling TrySuspend. `reattach` accepts a new runtime owner lease and shows that
same controller. Hidden panels reject manager focus requests. Browser/OS
throttling still applies; use application services for critical background work.
Crashes invalidate a hot instance and its bridge; reattachment never silently
recreates it.

The manager bounds panel/closing counts, hot count, host-estimated retained bytes
and hot time. Estimates are accounting reservations, not browser working-set
measurements. Unmount conservatively reserves the whole configured hot-byte
budget. `eviction_candidates` is advisory: the host chooses disposal, and an
unexpired lease is never silently replaced. Failed allocations return a budget
error. The manager must remain alive and be pumped while cleanup is pending.

## Profiles and cleanup

`Profiles::new` creates a marked application-owned root, refusing an existing
unmarked directory. Dedicated ephemeral sessions get a fresh directory and
ownership marker. Sessions are never recreated in a closing ephemeral directory.
Cleanup validates root, direct-child identity, marker and absence of symlinks/
Windows reparse points before removal. This is an ownership guard, not a sandbox
against another process with write access to the same directory.

Persistent and explicitly shared profiles are host-provided absolute paths and
are never deleted. Disposing a shared panel reports `shared_retained`; it does
not wait for or kill sibling browser processes. Dedicated cleanup checks both
session identity and browser PID. Timeouts and filesystem failures remain
observable as pending/failed scopes; `retry_cleanup` requires a prior matching
exit receipt. Startup reconciliation lists validated orphan records but never
deletes based merely on a stored path or assumes a previous browser has exited.
The host must reconcile those records with independently established process
ownership; automatic crash-recovery deletion is not implemented.

If `Provider::start` returns a synchronous error, the provider contract requires
startup rollback with no process or callback retaining the supplied profile.
`Manager::open` then removes an owned ephemeral profile immediately; if removal
fails, both failures and the profile path are reported. Persistent/shared data is
never removed on this path.

## Page authority

`PagePolicy` permits exact HTTPS document URLs (a narrower permission than an
origin); `about:blank` is an explicit local-fixture opt-in. Defaults deny
navigation, downloads and developer tools. Popups and host-object injection are
disabled. Download permission is explicit host policy; the host remains
responsible for its application download workflow.

After a successful navigation observation the host may grant named bridge
commands. Messages require version, session, current navigation epoch, command and
typed arguments. Session and epoch are decimal JSON strings so JavaScript does
not round 64-bit identities. Source and main-frame identity come from the provider,
not page JSON. The native adapter subscribes only to top-level WebMessageReceived.
Before copying COM strings into Rust allocations, it enforces a 4 KiB source limit
and a 65,536-byte UTF-8 JSON-message limit and rejects malformed UTF-16.
Navigation attempts and disposal revoke grants. No filesystem, MCP, native add-on
or desktop-input capability is inherited by a page. Native message notices are
untrusted input: call `Bridge::decode<T>` before turning one into an app action;
use `#[serde(deny_unknown_fields)]` for strict app payloads.

The native `execute_script` method is an explicit trusted-host facility used by
the fixture, never an exposed page bridge operation. Normal application commands
should remain typed host actions.

## Composition and native coverage

| Capability | Contract |
| --- | --- |
| Rectangular placement | Logical client bounds converted at current DPI; parent movement notification |
| Resize/reparent | Explicit place operation; parent windows retained by provider |
| Native z-order | WebView2 child-window layer; arbitrary GPU overlays above web content unsupported |
| Transforms/masks | Rejected with Unsupported; no implied texture composition |
| Focus/keyboard/IME | Explicit focus enters native controller; browser handles its native text path |
| Accessibility | Native WebView2/UIA subtree; no duplicated framework semantic tree |
| Hidden retention | IsVisible(false), no manager focus; state remains live |
| Product certification | Physical keyboard, Japanese IME, native AT client and mixed-monitor gates require separate evidence |

## Qualification and running

Bindings: [`webview2-com = 0.39.1`](https://github.com/wravery/webview2-rs)
(MIT, MSRV 1.82), matching workspace
`windows 0.62.2`. The binding, sys and macro packages are three added registry
packages. `base64 0.22.1` (MIT OR Apache-2.0) adds one codec dependency for exact
matching of the native NavigateToString data URL; it grants no general data URL
navigation permission. This adds four registry packages to the existing root
qualification graph; a minimal consumer also acquires the reused winit/windows
dependency families when enabling native hosting. Browser
runtime installation is separate. They are Windows-targeted
optional dependencies, not a portable core requirement.

The adapter checks installed Evergreen availability and requires the environment
process-exit and download-policy interfaces during creation. Unsupported runtimes
return errors. The host should direct users to Microsoft's
[Evergreen installation/recovery instructions](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution);
the framework never downloads or installs a runtime automatically.

Run from the repository root:

```powershell
cargo run -p view-webview --features native --example panel -- --runtime
cargo run -p view-webview --features native --example panel -- --native-check
cargo run -p view-webview --features native --example panel -- --show
pwsh -File scripts/verify.ps1 -NativeWebview
```

The explicit native fixture uses CSP-restricted local HTML and dedicated test
profiles. It verifies both lifetimes, repeated disposal, shared siblings and hot
JavaScript state without physical input. It writes `target/verification/m6-native.txt`.
Ordinary CPU tests use mocks and do not open WebView2. Recorded execution and
remaining acceptance gates are in the [M6 evidence](../plan/evidence/m6-webview.md).

Contracts follow Microsoft's [STA threading model](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/threading-model)
and [browser-group exit semantics](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2environment5).
An exit receipt is stronger than synchronous controller Close; neither a mock nor
a Close call alone certifies that browser files are released.
