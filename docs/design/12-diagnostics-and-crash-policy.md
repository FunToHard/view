# Diagnostics and crash policy

Status: revision 0.4 selected P-16 baseline: optional diagnostics/panic integration and compatible Windows WER collection. Custom dump helpers/watchdogs are deferred.

## Recommendation

Ship a first-party, optional diagnostics integration with clear application ownership. Include structured runtime errors, bounded breadcrumbs, build/resource identifiers, and a reporter interface. Do not silently install process-global crash behavior when a library is imported or a view is mounted.

The host application chooses whether to install panic hooks, enable OS dump collection or a separate reporter, retain reports, show recovery UI, and send telemetry. An engine embedding view may already own these mechanisms; composing with it is part of the supported integration.

## Distinguish failure classes

| Failure | Expected response | What is not promised |
| --- | --- | --- |
| Network/validation/asset error | Typed result and normal application error state | Treating expected failure as a crash |
| GPU validation/device loss | Diagnostics and supported device recovery path | Guaranteed recovery from every driver fault |
| Background task panic | Task failure/cancellation under explicit owner policy | Unconditional process recovery |
| UI/runtime panic | Record available context; host decides termination/restart | Continuing a possibly inconsistent tree |
| Native fault/abort | OS or external crash collector where enabled | Rust catch_unwind handling every process failure |
| Allocation failure/stack exhaustion | Best-effort minimal evidence | Allocation-heavy reporting or reliable in-process UI |
| Hang | Optional out-of-process heartbeat/watchdog diagnostics | Declaring every slow GPU/task to be a crash |
| Browser/WASM trap | Host/browser diagnostic path and reload policy | Native Windows dump machinery in the browser |

Rust panic hooks are global and run before the panic runtime. A hook must be explicitly installed, compose with the host's reporting policy, and avoid recursively panicking. [Rust set_hook](https://doc.rust-lang.org/std/panic/fn.set_hook.html)

catch_unwind only catches unwinding panics, not aborting panics or arbitrary native faults. It is not a general application error-handling mechanism. Catching at a deliberately isolated task boundary does not establish that the UI's invariants are safe after a panic. [Rust catch_unwind](https://doc.rust-lang.org/std/panic/fn.catch_unwind.html)

## Proposed diagnostic envelope

Reports can include application/build ID, view version, platform/backend, device generation, last commit/frame IDs, recent operation names, bounded scheduling/error events, and provider identifiers. Source symbols/PDBs and matching build identifiers belong in the release artifact policy.

Keep breadcrumbs while the application is healthy. A crash callback must not walk the entire tree, acquire arbitrary application locks, read back GPU textures, call a network API, or attempt to display a complex view dialog. Reporting buffers have explicit limits and failure behavior. Full process dumps may contain sensitive data beyond what structured-event redaction controls.

Provide a reporter trait/registration API without requiring one telemetry vendor. Payload detail, redaction, file paths, retention, upload, and consent are application policy. No automatic external upload is part of the default proposal.

## Windows collection options

Select optional OS-managed Windows Error Reporting local dumps for the initial collection integration, where the host's crash policy is compatible. Its documentation notes incompatibility with applications using their own custom crash reporting; do not stack incompatible handlers. The separate-helper option below is retained as future design rationale, not initial scope. [WER local dumps](https://learn.microsoft.com/en-us/windows/win32/wer/collecting-user-mode-dumps)

Microsoft recommends calling MiniDumpWriteDump from a separate process where possible because invoking it inside an unstable target may deadlock. A helper still needs correct process permissions, architecture support, setup, symbol management, and timeout behavior; it does not guarantee a dump for every termination. [MiniDumpWriteDump](https://learn.microsoft.com/en-us/windows/win32/api/minidumpapiset/nf-minidumpapiset-minidumpwritedump)

Do not replace native exception handling wholesale with a bespoke mechanism just to minimize dependencies. Keep native bindings and optional collection machinery outside the portable core. Browser-engine child failures are provider failures that may be restartable independently; host-process faults follow the application's crash policy.

## Document recovery is a separate feature

Unsaved creative work is protected by application-owned journaling/checkpoints written during healthy operation. A crash handler cannot be responsible for serializing an entire document at the moment of failure. Recovery validation and the user's restore choice occur on a subsequent healthy launch.

Schema/version migration and durable-write semantics are the document layer's contract. view can expose lifecycle and command hooks but cannot promise recovery of arbitrary application state.

## Test plan

Use disposable child processes to trigger an unwind panic, abort, representative native fault where appropriate, and a deliberate hang. Verify collection/timeout behavior, report structure, host-hook integration, and symbol association. Confirm that no test resumes a poisoned UI tree or uploads reports by default.

Test GPU errors separately from process crashes. Test browser/WASM diagnostic forwarding separately from native dumps. A missing dump is a reportable collection failure, not evidence that the application survived. The CI supervisor retains the exit status and available logs even when the crash collector fails.
