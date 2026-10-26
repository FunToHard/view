# M8 diagnostics and application recovery evidence

Scope: M8A-01 through M8A-05 / P-16 and Q-18 / acceptance T-21. Executed on
Windows x64, 2026-10-25. This is implementation/verification evidence, not another
progress board. See the [canonical checklist](../implementation-checklist.md) and
[host integration guide](../../guides/diagnostics.md).

The optional `view-diagnostics` package implements a versioned bounded envelope,
independent revision streams, capability/build/symbol identifiers, runtime trace
ingestion, host redaction before serialization, explicit storage/retention, and
best-effort immutable prepared panic reports. It never installs global hooks,
uploads, walks an unhealthy UI tree, or catches/resumes UI panics. Its explicit
composition API preserves the supplied host hook. Core/facade dependencies remain
unchanged. Existing serde/serde_json versions are reused; Windows bindings already
qualified by the workspace are used only by the disposable failure executable.
No new third-party registry package was selected. Library code forbids unsafe;
the isolated Windows fixture documents both per-process WER UI flags and the
pointer-free, noncontinuable native exception call.

M8A-03 is implemented as reviewed configuration/guidance, as allowed by its task:
validated per-executable WER .reg generation with explicit administrator/ACL and
collector-compatibility guidance. Healthy PE CodeView parsing reports actual PDB
GUID+age without leaking an embedded source path. Synthetic valid/malformed and
the current Windows executable are checked. No registry changes were made and no
WER dump collection, matching-PDB debugging session, arbitrary native fault,
production watchdog or other-OS symbol support is claimed.

The application-specific disposable proof owns its checkpoint schema and restore
decision. Two completed revisions are synced/published during healthy operation;
the incomplete third write is left for validation. Separate healthy launches
reject corrupted/unsupported later records, offer revision 2, then restore only
under the application's explicit accepted-restore policy. This proves process
termination recovery, not universal application recovery or power-loss durability.

Executed focused verification: all-feature Clippy with warnings denied, all-feature
tests (including the independent failure supervisor), and all-feature rustdoc
passed on Windows. The core workspace check/test/doc pass also includes the
default library profile without Windows failure-fixture dependencies. The broader
integration script passed in `target/verification/m8-final.log`.

## Disposable failure observations

The supervisor owns each child's readiness deadline, exit observation, timeout,
kill/reap guard and retained stdout/stderr. It never drives desktop input or uses
personal browser profiles. All four cases restore the same application checkpoint
on a fresh healthy process; none resumes a failed UI tree.

| Mode | Observed Windows exit | Prepared panic report | Host hook | Supervisor |
| --- | --- | --- | --- | --- |
| Panic | 101 | Redacted envelope with document revision 2 and actual PDB identity | Preserved | Child exited |
| Abort | 0xC0000409 / -1073740791 | None, as expected for explicit abort | Not invoked | Child exited |
| Native SEH exception | 0xE0425649 / -532523447 | None, as expected | Not invoked | Child exited |
| Hang | Killed, exit 1 | None | Not invoked | 300 ms post-readiness deadline, killed and reaped |

Example executed manifest:
`target/diagnostics-contracts/11860-1792903881640791000-manifest/manifest.json`.
Each case directory retains `supervisor.json`, healthy context, panic output,
stdout/stderr, completed and partial checkpoints, offered recovery and restored
text. `wer_dump_collection` explicitly says not configured/qualified.

Linux/macOS execution, WER collection, real fatal hardware/driver faults,
allocation/stack exhaustion and production hang dump capture remain uncovered.
No physical E2E, benchmark result, OS dump or telemetry delivery is inferred from
these tests.
