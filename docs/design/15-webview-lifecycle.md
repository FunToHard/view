# Webview panels and resource lifecycle

Revision: 0.4. Status: selected design baseline under the owner's request to finalize decisions. Implementation and measured cleanup guarantees remain to be verified.

## Developer-selected policy

view will provide an optional Windows webview panel adapter using WebView2. Native view controls remain the primary UI; a webview is an opt-in panel provider. This is independent of deferred WASM/web delivery and does not change the native storefront proof into a browser application.

Two lifetime policies are first-class:

| Policy | On application-declared work completion / panel release | On reuse |
| --- | --- | --- |
| DisposeAfterUse (default) | Close the webview and retire all resources owned by that panel/session under its declared ownership policy | Create a fresh instance; restore only explicitly persisted application state |
| KeepHot | Detach/hide from visible UI while retaining the loaded instance and its state under an application-owned lease | Reattach/show the retained instance without deliberate reload |

The application decides what “work is done” means. Completion is explicit; finishing navigation or downloading an asset is not an implicit instruction to destroy the panel. A normal UI unmount follows its selected policy. App shutdown, explicit dispose, or failure invalidates a hot lease.

Proposed API sketch, not implemented:

```rust
WebPanel::new(url)
    .lifetime(WebPanelLifetime::DisposeAfterUse)
    .profile(WebProfile::EphemeralDedicated)

WebPanel::new(url)
    .lifetime(WebPanelLifetime::KeepHot)
    .profile(WebProfile::Persistent("help"))
```

Lifetime and stored-data retention are independent settings. A persistent login/profile is never erased merely because a panel is disposed. A caller requiring cleanup of panel data as well as execution resources selects an exclusively owned ephemeral profile.

## Ownership and cleanup meaning

Track panel/controller ownership, environment/process-group ownership, profile ownership, and persistent application data separately. Default ephemeral panels use an exclusively owned user-data directory and session, so unrelated panels do not share their process group. Shared environments/profiles require explicit opt-in and have weaker per-panel process-release guarantees.

WebView2 can share browser processes across views associated with the same user-data folder; creating another environment object with the same configuration does not necessarily isolate the process group. [WebView2 process model](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/process-model)

Disposal must:

1. Mark the instance closing and revoke its native/automation bridge authority.
2. Cancel owned requests/tasks, invalidate callbacks by generation, and resolve focus, capture, IME, and native view placement.
3. Remove subscriptions/native event registrations and stop accepting new panel work.
4. Explicitly close the controller and release owned COM, host, graphics, stream, bridge, and application references on the correct thread.
5. For an exclusively owned session, observe the corresponding browser-process exit and ensure no new lease/session has replaced it.
6. Delete the exclusive ephemeral profile only when safe, or report pending/failed cleanup. Preserve explicitly persistent profiles.
7. Produce an observable cleanup result with completed and pending scopes.

WebView2's controller Close breaks the instance lifecycle; its documented browser shutdown depends on whether other webviews still use the browser instance. Close is synchronous but does not imply that every associated process/data file has already disappeared, and it does not trigger beforeunload. The host must resolve any application-level save/confirmation flow before closing. [Controller Close](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2controller)

The user-data folder contains state such as cookies and cached resources. Profile deletion must follow the relevant process/session lifecycle rather than racing active users of that directory. [User-data management](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/user-data-folder)

Define cleanup outcomes such as PanelReleased, SessionExited, EphemeralProfileRemoved, SharedSessionRetained, and CleanupPending/Failed. “All cleaned” is emitted only for the owned scopes actually confirmed. Runtime installation files, global OS caches, and explicitly persistent/shared data are not panel-owned cleanup targets.

Cleanup is asynchronous at the framework level; never block the UI thread awaiting process exit. Bound outstanding closing sessions and use timeouts plus diagnostic receipts. A hung process or locked folder is a reported failure, not a silent leak or successful disposal. Never terminate a shared browser group to clean up one panel. Any exceptional termination of an exclusively owned group is a separately declared host policy and recorded as abnormal shutdown.

Explicit finish/dispose supports awaiting cleanup. Drop/unmount schedules best-effort cleanup but cannot promise asynchronous work completed before returning. Pending cleanup can be reconciled on next healthy startup using validated ownership records; do not delete arbitrary directories based solely on a stored string.

## KeepHot contract

KeepHot retains the webview rather than destroying or deliberately suspending it. It remains an owned, countable resource with a live lifecycle; it does not mean an invisible orphan. A hot panel is excluded from visible focus/accessibility hit targets until reattached, while bridge operations allowed by the application can continue.

Background execution remains subject to browser/OS throttling and process failure. KeepHot does not guarantee real-time JavaScript, uninterrupted networking, or an immortal renderer. Critical background work should use application services rather than rely on a hidden page running at a fixed cadence.

Hot retention has configured count/memory/time budgets and an application eviction policy. If the caller requires an unexpired hot lease, the manager must request/notify eviction or reject additional allocations rather than silently replacing it with a cold instance. Instrument retained instances, resource use, last activity, and readiness. Report a crashed/recreated hot instance explicitly so the app can restore state.

Suspended retention may be added as a separate policy later. It is not an alias for KeepHot: WebView2's TrySuspend is best effort and pauses activities such as script timers/animations. [WebView2 suspension](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2_3)

## Platform and bridge boundaries

The initial adapter hosts rectangular native panels using documented Windows facilities. Native child-view z-order, focus, DPI, keyboard/IME handoff, and accessibility are acceptance requirements. Arbitrary transformed/masked GPU composition or overlaying every kind of view widget over native web content is not promised by the first adapter. Unsupported composition requests fail with a capability explanation.

Use a dedicated optional crate with WebView2 COM bindings. Prefer the managed Evergreen runtime for the initial distribution path; detect missing/incompatible runtime and provide a documented host installation/recovery route. A fixed runtime is a later packaging option. No runtime is bundled, downloaded, or installed by this planning work. [WebView2 distribution](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution)

Bridge commands are typed, origin/frame/session checked, and limited to app-granted operations. Page navigation can revoke bridge permissions. Web content does not acquire application MCP, filesystem, native plugin, or desktop input authority merely by being hosted. The app decides navigation/download permissions and whether developer tools are exposed.

## Acceptance evidence

- Repeated create/use/dispose cycles release host references, subscriptions, native views, and owned graphics resources; measured allocations do not grow without bound after warmup.
- A dedicated ephemeral session reports process exit and safe data-folder cleanup; failure/timeout is visible.
- Disposing one shared-session panel leaves siblings functional and reports shared retention honestly.
- KeepHot preserves a test page's local state across hide/show, avoids intentional reload/suspension, and obeys configured budgets.
- Hot process failure, late callbacks, application shutdown, profile locks, and recreate-during-cleanup races have deterministic handling.
- Native E2E covers focus/IME transfer, keyboard shortcuts, scrolling, DPI changes, and reattachment.
- A mock provider tests lifecycle state machines on other OS runners; it does not claim native WebView2 coverage there.

The project plan makes both lifetime policies part of the optional webview module's release gate. Webview lifecycle tests use owned local fixtures and explicit profile directories; no browser user profile is a test cleanup target.
