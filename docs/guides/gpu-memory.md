# GPU memory measurement

The selected approach remains hardware GPU rendering. The owner allows a revised
memory budget if measurements justify it; software rendering is not an option
for meeting the target. Working set and private commitment are separate metrics.

On a Windows desktop, run:

```powershell
pwsh -File scripts/measure-gpu-memory.ps1 -Runs 3
```

This builds the release `view-controls` memory_probe example and launches each
run in a fresh process with a hidden, inactive native window. No physical input
is injected. A CPU adapter is rejected. The example uses the same DX12 context,
renderer, text backend and control behavior APIs as the framework, with their
default limits/budgets. No cache trimming or working-set manipulation is used.

The cumulative stages are process baseline, native window, instance/adapter/raw
surface, GPU device, configured swapchain, renderer/clear, system-font discovery,
CPU text shaping, GPU text, and a focused textbox with two buttons. It then
executes 1,000 native size requests and rendered frames, followed by 1,000 edit
operations (500 insert/backspace pairs), checking that the original text returns.
After GPU completion it measures retained memory, then drops GPU/text/control
owners and measures remaining process memory with the native window still alive.

Each quiet stage holds for three seconds. Explicit transition markers separate
next-stage initialization; transition samples remain in raw CSV but are excluded
from stage summaries. The external PowerShell sampler reads
private bytes and working set every 200 ms. The summary reports the median of
the final five samples per stage, plus sampled stage peaks; raw CSV also includes
the OS process peak working set. Short-lived private-memory peaks between samples
can be missed. Run logs identify the adapter and validation outcome; environment
metadata records the binary hash, OS, profile and sampling settings. Outputs are
ignored under policy-specific subfolders of `target/verification/gpu-memory/`
and are overwritten when that policy is rerun.

Interpret stage differences as cumulative observed increments, not exact live
allocation sizes: drivers/allocators can retain memory, defer allocation or share
pages. Working set includes resident shared code and changes under system memory
pressure; private bytes are process commitment, not physical RAM or GPU VRAM.
GPU local/nonlocal memory is not collected by this sampler. Do not add these
metrics together or treat glyph-atlas reservation counters as measured memory.

The form is a renderer/control probe, not a full application certification. It
does not install accessibility/IME services or application runtime/document state.
The hidden window and synchronous fixture do not reproduce visible compositor
behavior, physical input timing, or an interactive event loop under load. Record
those additions separately before adopting a production budget. Compare repeated
release runs on the same GPU/driver and with similar background load; do not use
these absolute values as portable CI thresholds.

## Local baseline — 2026-10-24

Three fresh release processes passed on Windows build 26300, NVIDIA GeForce
RTX 3050 6GB Laptop GPU, driver 32.0.16.1714, DX12. Each completed 1,000 native
size requests and 500 insert/backspace pairs without reported GPU validation
errors. No compilation ran concurrently with sampling. Values below are ranges
across the three per-stage tail medians, in MiB:

| Cumulative stage | Working set | Private commitment |
| --- | ---: | ---: |
| Process | 7.40–8.22 | 1.43–1.48 |
| Native window | 13.41–14.06 | 2.23–2.26 |
| Adapter and raw surface | 61.81–62.55 | 70.65–71.37 |
| GPU device | 157.37–157.94 | 317.91–319.00 |
| Configured swapchain | 158.12–158.71 | 317.62–318.69 |
| Renderer and clear | 166.04–166.66 | 323.11–323.57 |
| System fonts | 166.07–166.88 | 322.71–323.71 |
| CPU shaped text | 166.74–167.57 | 322.78–323.71 |
| GPU rendered text | 169.23–169.73 | 324.77–325.23 |
| Textbox and two buttons, idle | 169.95–170.42 | 325.46–325.91 |
| After 1,000 resizes | 177.65–178.43 | 333.10–333.95 |
| After 1,000 edits | 179.20–179.78 | 334.63–335.34 |
| After dropping GPU/control owners, window alive | 118.43–119.29 | 272.85–274.32 |

The dominant observed increase occurs in graphics initialization, before text
and controls. This establishes the cost of this configuration, not an inherent
minimum for every hardware-GPU implementation. Residual process commitment after
dropping owners does not by itself identify a leak; driver/runtime allocations
can remain until process exit, and allocation-stack analysis is a separate step.

The earlier provisional 200 MiB working-set / 384 MiB private-commitment budget
is withdrawn. It was proposed before separating device allocation policy and did
not describe a product target. The policy comparison below is the current result;
no cross-machine budget or release gate has been selected. Full application
services, visible presentation and other GPUs/drivers need their own measurements.
Hardware rendering remains required; software rendering is not used to reduce
these numbers.

The corrected probe emits explicit transition markers. Earlier exploratory
samples mixed next-stage initialization into prior stage names and are superseded
by this baseline. In particular, a preliminary 63 MiB window-only observation
included graphics initialization; the corrected window-only measurement is
13–14 MiB. The historical baseline binary SHA-256 was
`C2CDBACEDCF3E132CD92C0DFCDF83FF8D7C478F2E3763D2D0307EC3523A97066`; policy
instrumentation later overwrote its raw files. The summarized measurements remain
above; current raw samples and binary identities are in each policy subfolder.

## DX12 allocator policy investigation — 2026-10-24

wgpu 30.0.1 exposes `DeviceDescriptor::memory_hints`. Its DX12 backend maps
`Performance` (the default) to 128–256 MiB device and 64–128 MiB host suballocation
blocks. `MemoryUsage` maps to 8–64 MiB device and 4–32 MiB host blocks. A manual
device range of 16–64 MiB maps to 8–32 MiB host blocks. These configure allocator
block sizing; they are not guaranteed allocations at device creation. The probe
now accepts `default`, `usage`, and `manual16` as its second argument and prints
wgpu's live allocation and reserved block report. For example:

```powershell
pwsh -File scripts/measure-gpu-memory.ps1 -Runs 3 -Policy usage
```

Three fresh-process runs per policy used the same hidden DX12 RTX 3050 6GB Laptop
GPU, driver 32.0.16.1714, release probe, 1,000 resize requests/frames and 1,000
text edit/render iterations. Values are medians/ranges in MiB; allocator reserved
bytes are wgpu reports, not process memory:

| Policy | Device stage private / working set | Idle controls private / working set | After workload private / working set | wgpu reserved: device → after edits |
| --- | ---: | ---: | ---: | ---: |
| Default `Performance` | 317.2 / 157.4 | 324.8 / 170.0 | 334.0 / 179.2 | 192 → 192 |
| `MemoryUsage` | 138.8 / 97.3 | 146.7 / 110.0 | 252.1 / 119.5 | 12 → 108 |
| Manual 16–64 MiB | 151.1 / 101.4 | 158.8 / 114.0 | 232.0 / 123.3 | 24 → 88 |

At device creation, live wgpu allocations were only 0.69 MiB for all policies;
reserved blocks differed substantially. At idle they were 2.50 MiB. After the
edit/render workload each policy reported 65.1 MiB live in 1,032 GPU allocations,
while reserved blocks differed as shown. The allocation names include renderer
vertex/index resources and glyphon resources. This is allocator capacity, not
proof of a framework leak: the probe submits many frames before one final GPU
wait, and those frames can keep resources alive. A realistic interactive cadence
and resource-lifetime investigation are still needed.

From the adapter/surface stage to device creation, private commitment rose by
about 247 MiB (default), 68 MiB (`MemoryUsage`), and 80 MiB (manual16). Subtracting
wgpu's reported reserved blocks leaves roughly 55–56 MiB of process commitment
not represented by that report. This remainder combines wgpu, DX12, Windows and
driver state; this probe cannot attribute it to the NVIDIA driver alone. Working
set rose by about 96, 36 and 40 MiB, respectively.

One additional run per policy after instrumenting workload duration measured
resize request/render loops at 9.99, 10.61 and 10.09 seconds (default, usage,
manual16), and edit/render loops at 10.02, 9.99 and 10.01 seconds. This small,
single-run sample is diagnostic only, not evidence of equivalent performance.
It did not measure GPU throughput or frame latency under an interactive event
loop.

Conclusion: the large initial private-commit jump is strongly associated with
wgpu's default DX12 suballocation block sizes: smaller policies reduce device-stage
private commitment by about 166–179 MiB in this setup without switching rendering
backends. Manual 16–64 MiB had the lowest post-workload commitment; `MemoryUsage`
had the lowest idle commitment. These results do not justify changing the
framework's default yet: measure frame latency, realistic resource churn, and
integrated/low-memory hardware before selecting the default or adding a public
policy setting. They do show that the old initial-memory figure was substantially
allocator-reservation policy, not bytes needed by the textbox and buttons.

Verification passed: release build and all three native probes; workspace
fmt/check/clippy/tests/docs; controls all-feature clippy/tests; documentation
links/fences and diff whitespace checks. Existing generated test PDBs caused
LNK1285 errors on the first workspace-test attempts; deleting only the named
PDBs under target/debug/deps and rebuilding resolved them. Logs are retained as
`target/gpu-memory-probe-verification*.log`,
`target/gpu-memory-probe-test-retry4.log` and
`target/gpu-memory-probe-final-checks.log`. This adds local Windows evidence only.
