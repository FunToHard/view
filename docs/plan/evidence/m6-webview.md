# M6 optional webview implementation and evidence

Date: 2026-10-22. Local Windows x64. Status is tracked only in the
[implementation checklist](../implementation-checklist.md). No M6 remote CI,
Linux/macOS execution or physical-input certification is inferred here.

## Implementation mapping

| Items | Implementation and evidence |
| --- | --- |
| M6-01 | Optional Windows-targeted webview2-com 0.39.1 / windows 0.62.2 adapter; installed Evergreen probe and explicit unsupported errors/recovery route |
| M6-02 | Rectangular logical-to-physical bounds, retained parent HWND, UI-thread check, resize/reparent/movement, native focus entry; transforms/masks rejected. Native keyboard/IME/UIA certification remains open |
| M6-03–04 | Runtime cancellation owner, unique session/profile, explicit shared/persistent opt-in, exact document/main-frame/session/epoch/command typed bridge; navigation revocation and deny-default download/devtools/popup policy |
| M6-05–07 | Owner-thread event removal/Close, stale script-result rejection, matching whole-group exit, asynchronous guarded profile deletion, scope receipts, bounded manager/closing capacity, timeout/retry and conservative startup orphan discovery |
| M6-08–09 | Same-controller hide/reattach, retained JavaScript state, explicit count/estimated-byte/time leases and eviction candidates; no silent recreation or deliberate suspension |
| M6-10 | CPU races/locks/replacement tests and native repeated disposal/shared sibling/hot-state fixture with measured owned reference counts |
| M6-11 | CPU hot-budget and placement checks implemented; physical keyboard, Japanese IME, scroll, mixed-monitor DPI/reattachment and Linux/macOS mock execution still required |
| M6-12 | Explicit passive `panel` example, lifetime receipts/counts and native composition matrix in the [guide](../../guides/webview.md) |

## Executed native evidence

The explicitly invoked passive fixture used installed WebView2 Evergreen
**155.0.4283.62**, CSP-restricted local HTML and owned profiles below
`target/webview-native/`. It passed both policies, typed page-to-host
bridge delivery, hot JavaScript state value **42**, shared sibling state **7**,
three repeated dedicated create/dispose cycles and final shutdown.
Each repeated cycle returned to **(2 controllers, 2 environments, 0 pending
creations)**. Final owned counts were **(0, 0, 0)** with matching browser-group
exit and ephemeral-profile removal receipts. `passed=true failure=None` is
recorded in `target/verification/m6-native.txt`; final callback observations are in
`m6-native-final-run.log`. These are resource counts, not a browser memory plateau or
performance benchmark. Persistent/shared profile directories were retained.

Initial failed runs exposed NavigateToString's exact data URL and asynchronous
script reply ordering. The implementation now allows only the specific expected
host HTML data URL and keys replies by panel. Those failures were not counted
as passing evidence. Abandoned run profiles remain available for conservative
reconciliation; no process was assumed exited merely to erase its files.

The final native rerun includes parent-thread validation, stricter orphan record
validation, cleanup event-order fixes, panel-specific focus tracking and event
queue protection against page traffic evicting lifecycle receipts. An earlier
rerun was not executed because automatic approval review reached a usage limit;
review subsequently succeeded and the final authorized passive run passed. The
passive fixture does not certify physical focus traversal or IME behavior.

## Verification and limits

CPU tests cover repeated matched disposal, wrong session/PID, shared/persistent
ownership, hot count/bytes/TTL/focus denial, unmount/crash, profile marker
replacement, real Windows file lock and retry, typed source/frame/epoch authority,
creation cancellation, late failure/duplicate exit, crash-exit-before-dispose,
recreate-before-cleanup and shutdown. A native-adapter unit test floods the event
queue and verifies that Ready/Exited receipts survive. They do not open a browser.

The first full verification passed required workspace formatting, check, Clippy,
tests and documentation, then failed all-feature example Clippy on a complex
reply type. The alias was factored and the full Windows `scripts/verify.ps1`
run then passed (exit 0), including required workspace checks, supported feature
profiles, hidden native shell checks, independent consumers and dependency
inventory. Logs are under `target/verification/m6-full*.log`.

The final M6-specific formatting, all-target/all-feature Clippy, tests and docs
passed after the last implementation edits: eight lifecycle tests and one
native-adapter event-queue unit test. The passive WebView2 fixture also passed
after those edits. Documentation links/fences and `git diff --check` passed.
The independent consumer additionally enables `webview-native` to check the
Windows provider outside workspace feature unification; its updated lockfile
includes the previously absent winit/windows platform family. The locked native
consumer check and all-feature consumer tests both passed after this addition.

The binding's upstream Windows COM package uses the workspace's windows 0.62
family and declares MSRV 1.82 / MIT. Its sys/macro crates are isolated behind the
Windows native feature. Base64 0.22.1 is MIT OR Apache-2.0 and used only for exact
NavigateToString matching. No browser executable is bundled. Source maintenance
and API references are linked in the [guide](../../guides/webview.md); the
dependency inventory records selected licenses/features rather than asserting
these packages are bug-free.

Startup reconciliation is diagnostic, not automatic crash-recovery deletion.
A failed creation without a confirmed process identity can retain an orphan
profile; it is not safe to infer process exit from an error. Hot byte accounting
uses host reservations, not process working-set measurements. Drop cannot pump
asynchronous shutdown. M6 remains short of full native input/accessibility and
cross-platform acceptance until those named gates are executed.
