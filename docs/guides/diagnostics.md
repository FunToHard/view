# Host-owned diagnostics and document recovery

`view-diagnostics` is an optional package. The facade/core do not depend on it.
It reuses the qualified serde/serde_json family and has no telemetry transport.
Windows bindings are used only by its disposable failure fixture. The library
forbids unsafe code; the fixture isolates its documented native exception calls.

## Healthy-operation reports

Create `Diagnostics` with application/build/symbol/framework/platform/backend
identifiers chosen by the host. Set capabilities and the independent document,
committed, presented and device-generation revisions. Drain `Runtime::drain_trace`
into `Diagnostics::trace` on the owner thread while healthy. Core never calls a
reporter and has no dependency back to diagnostics.

`Limits` bounds event count, UTF-8 bytes per text field and serialized report
bytes. Hard ceilings are 256 events, 512 bytes per string, 64 capabilities and
1 MiB per report; defaults are 64 events, 256 bytes and 64 KiB. Oldest events are
evicted and counted; zero events disables retention. A serialization budget
failure returns an error without returning partial bytes. Operations should be
stable names such as `document.checkpoint.complete`, never user document text.

`prepare` invokes the host redactor for every textual field, including identifiers
and capabilities, before producing immutable bytes. The redactor sees already
bounded strings; use field/operation allowlists rather than assuming full input
text remains available. Returning `None` removes capabilities/events or blanks an
identity field. Its output is bounded again. Host callbacks and the original
caller-supplied values are outside the library's allocation guarantees.

`ReportStore` is an explicit healthy-operation file sink. The host selects an
exclusively owned directory/ACLs, a monotonically increasing report sequence, a
retained count (1–1,000) and a byte limit (at most 1 MiB). Writes use create-new,
sync and bounded retention. Only regular `view-report-<20 digits>.json` files are
retired; unrelated files and symlinks are excluded. More than 1,000 existing owned
reports is an actionable scan-budget error. A write/retention failure propagates;
the host must decide whether to retry, remove partial output or warn. No scan or
retention runs implicitly on import, mount, panic, or startup.

## Panic ownership

Implement `Reporter` using host-owned, preopened resources. Its panic-time
callback must be short, must not panic, acquire application locks, call back into
`PanicReporter::update`, walk the UI tree, read GPU textures, or upload data.
Filesystem I/O remains best effort and can block at the OS; this does not promise
reliable reporting for allocation failure, stack exhaustion or a damaged process.

Prepare/redact while healthy, then create `PanicReporter`. Healthy turns may
publish a replacement immutable report with `update`. `emit` uses nonblocking
snapshot access; contention, poisoning and reporter errors produce `false`.
Neither construction nor composition installs a process-global hook.

The application explicitly owns serialized startup and the global lifetime:

```rust,no_run
use std::sync::Arc;
use view_diagnostics::{Diagnostics, Identity, Limits, PanicReporter, Reporter,
    chain_panic_hook};

fn install_at_application_startup(sink: Arc<dyn Reporter>) -> std::io::Result<PanicReporter> {
    let collector = Diagnostics::new(Identity::default(), Limits::default());
    let prepared = collector.prepare(|_, value| Some(value.into()))?;
    let reporter = PanicReporter::new(prepared, sink);
    let previous = std::panic::take_hook();
    std::panic::set_hook(chain_panic_hook(previous, reporter.clone()));
    Ok(reporter)
}
```

The prior hook runs after best-effort prepared reporting, preserving host policy.
Hosts that already own a hook can instead call `emit` inside that hook without
replacing it. There is no automatic restoration guard: another library could
replace the global hook afterward, so restoration is the application's serialized
shutdown decision. The prepared report intentionally omits panic payloads and
source paths; the original host hook controls its own sensitive output.

Hooks run before panic unwinding/aborting. An explicit `process::abort`, native
fault, kill or hang may never call a Rust panic hook. This package never catches
a UI panic or resumes an inconsistent tree. See the Rust documentation for
[global hook ownership](https://doc.rust-lang.org/std/panic/fn.set_hook.html) and
[the limits of catch_unwind](https://doc.rust-lang.org/std/panic/fn.catch_unwind.html).

## Windows WER, symbols and report handling

`WerLocalDumps` produces reviewable per-executable `.reg` text with a validated
executable basename, absolute folder, count and mini/full dump choice. `DumpFolder`
uses the documented REG_EXPAND_SZ type, encoded as UTF-16LE `hex(2)`. Save the
generated text with an encoding supported by the chosen registry editor, review
it, and have the application administrator explicitly import it. The API never
changes the registry, creates a dump folder or enables collection. Do not use a
global LocalDumps setting for the test executable.

The host must provision writable folder ACLs, choose retention/access rules,
decide consent/report disclosure and later remove its per-executable settings
when appropriate. Dumps can contain secrets beyond structured-event redaction.
Microsoft documents that local dumps require administrator setup, are incompatible
with applications doing custom crash reporting, and are not collected when
automatic postmortem debugging is configured. Check compatibility with the actual
host collector before enabling them. Local dump controls are independent of other
WER reporting settings. See [Microsoft's collection contract](https://learn.microsoft.com/en-us/windows/win32/wer/collecting-user-mode-dumps).

During healthy startup, `read_pdb_identity(current_exe)` can obtain a PE32/PE32+
CodeView RSDS GUID+age. `symbol_key` uses the conventional Windows GUID byte order.
Parsing uses bounded reads; malformed/unsupported input is an error and absence
of an RSDS record is `None`. No embedded developer PDB pathname is returned. Linux
and macOS symbol formats are unqualified. Add this identity and an application
release/build identifier to `Identity`; retain the exact EXE/DLL/PDB files and
their hashes together in the release artifact manifest. A human build label
alone does not prove PDB compatibility. Symbols must match binary GUID+age when
debugging; the diagnostic library does not find or upload them.

On the next healthy launch, the host enumerates its owned dump/report directory,
associates reports with release/symbol manifests, enforces retention, and offers
its recovery/report UI. No external upload or consent decision is provided by
this package. WER dump generation was not enabled or qualified in the local
fixture. A missing dump remains an uncovered/failed collection gate; it never
means that a process survived. Hangs need an external supervisor/explicit dump
capture; custom production watchdog/dump helpers remain deferred. Native driver
faults, hardware failures, stack exhaustion, OOM and every abort configuration
are not certified by the representative fixture below.

## Application checkpoint proof and disposable supervisor

Run from the repository root:

```powershell
cargo test -p view-diagnostics --all-features --locked -- --nocapture
```

The independent integration-test supervisor launches `diagnostic-fixture` in a
dedicated directory under `target/diagnostics-contracts/`. It waits for a healthy
readiness marker, retains stdout/stderr, polls child status with a deadline, and
kills/reaps a deliberate hang. A drop guard also kills/reaps a child if an
assertion fails. It never uses physical desktop input, personal profiles, network
uploads, registry setup or an in-process catch around native failure. Its flags
suppress WER UI only in the disposable child process.

Four Windows cases run: unwind panic, explicit abort, a noncontinuable
application-defined Windows SEH exception, and hang. The native exception uses
`RaiseException` with no pointer payload and no handler that resumes Rust frames;
it does not manufacture Rust undefined behavior or claim to reproduce every
access violation. [Microsoft documents the native exception dispatch](https://learn.microsoft.com/en-us/windows/win32/api/errhandlingapi/nf-errhandlingapi-raiseexception).
Non-Windows runs omit the Windows native-fault case explicitly.

Before installing any hook, the fixture application writes two completed
checkpoints using its own versioned JSON document format, checksum, sync and
same-directory publication. It leaves an incomplete third write. No crash
callback serializes a document. A fresh healthy process validates schema,
filename/revision agreement, byte/text bounds and checksum, rejects malformed or
unsupported later records and presents revision 2 as a recovery candidate.
Only an explicit application restore choice writes the recovered text. The test
checks both the offered candidate and accepted restore separately.

This proves recovery after process failure for the example's schema, independently
of crash report availability. It is not a universal framework document format,
an authenticated journal, a migration engine, or a power-loss durability promise
for directory metadata. The application owns journal retention and future schema
migrations. Per-case `supervisor.json`, logs, reports and recovery artifacts remain
under `target/`; the canonical checklist records executed platform coverage.
