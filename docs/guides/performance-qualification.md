# Reproducible Windows performance qualification

The hardware DX12 probe measures a hidden 640×360 form with one focused text
field and two buttons, expanding to 839×559 during resize. It uses the actual
GPU renderer, SDR output, default rendering budgets, 128 retained text layouts,
system fonts, 1,000 resize requests and 500 insert/delete pairs. The application
returns to its original text. Every iteration checks the retained renderer and
text cache limits. A CPU or unidentified adapter is rejected.

The selected workload definitions are version 1:

| Workload | Fixed input / extent | Quality and measurement environment |
| --- | --- | --- |
| Hardware form resize/edit | Field and two buttons, 640×360 through 839×559, 1,000 requests of each kind | Release, hidden hardware DX12, SDR, default budgets; sans-serif 16 px / 22 px line height and recorded system-font inventory |
| Virtual hierarchy | 100,000 keyed rows, 800×600 viewport, 24 px row height, 25 realized rows, 1,000 positions | CPU correctness; no GPU or input-latency claim |
| Multi-window edit/undo | Public editor/storefront/IDE document, three runtime windows, 1,000 cycles | CPU correctness and retained node/action invariants; example fixes its layout |
| Background import | 1,000 4 KiB assets; channel capacity four, cache eight / 32 KiB | CPU correctness with stale/current request pairs |
| Sparse tiled document | 4,096×4,096 RGBA8, 1,000 pixel edit/undo pairs | CPU document operations, at most one 16×16 tile/history item; no rendering |
| Analytic scene | Five indexed quads (ten triangles), then one lit triangle; 64×64, 128×64, 129×65 targets | Hardware correctness, single sample, sRGB color / Depth32Float, no textures/fonts; unlit and directional metallic-roughness profiles |

Scene input and camera are fixed by the compiled `view-wgpu` scene example (FOV
π/2, near 1, far 10); fixtures use original OBJ geometry. UI proof fonts use the
licensed Noto Sans fixture checked by the fixture manifest. Hardware/driver and
binary/font-inventory identifiers for the measured form are recorded in
[executed evidence](../plan/evidence/m8-performance.md). Correctness runs may
overlap compilation. A dedicated quiet-host environment and reviewed baselines
are required before enforcing comparative performance results.

```text
pwsh -NoProfile -File scripts/measure-gpu-memory.ps1 -Runs 3
cargo run --release -p view-application-proofs --locked -- --stress
```

Memory reports distinguish private commitment from working set. Each process is
sampled every 200 ms and held for three seconds at quiet stages. The five final
samples supply each stage's median; peak columns are sampled peaks, not a claim
to observe every allocation. External process sampling adds overhead. The
environment includes CPU/GPU/driver, OS, binary hash and system-font inventory
fingerprint. Do not compare runs from different hardware/driver/font environments.

Each resize/edit distribution reports first, p50, p95, p99 and maximum elapsed
time. `renderer_prepare` covers renderer preparation. `surface_submit` includes
surface acquisition, retry waits and submit/present calls. `cpu_total` also
includes control layout/shaping and the workload operation. `*_warm` discards
the first 100 iterations. These are CPU elapsed measurements through the present
call; they do not measure GPU completion, compositor scanout or physical-key to
visible-pixel latency. No timestamps from those absent domains are invented.

The CPU stress proof separately exercises 100,000 virtual rows with 25 realized,
1,000 edits/undos across the three-window document, 1,000 idle flushes and 1,000
bounded background imports with stale request rejection. A 4,096×4,096 sparse
image workload performs 1,000 pixel transactions and undoes each, retaining at
most one tile/history item and returning to zero allocated tiles. It checks unchanged
idle build/layout/commit counters, bounded queue/cache counts and final document
state. It measures each workload independently and does not call headless polls
OS wakeups. The image document and scene examples cover their own contracts.

To compare a candidate, first preserve a reviewed run's entire output directory
at a separate baseline path under `target/`. Then use:

```text
pwsh -NoProfile -File scripts/measure-gpu-memory.ps1 -Runs 3 -Baseline target/verification/baseline -RegressionFactor 1.5
```

Comparison rejects mismatched environments and missing timing series, and checks
median-across-run p50/p95/p99 values against the explicit factor. A baseline may
not be the output directory. The factor is an application/runner policy, not a
universal frame-rate guarantee. Establish it with repeated quiet-host evidence;
concurrent builds, power/thermal state and foreground applications can affect
tail latency. No fixed 30 MiB memory promise is enforced: the owner allows a
memory compromise while requiring hardware rendering.

CPU, GPU and selected-desktop correctness qualification has a separate runner:

```text
pwsh -NoProfile -File scripts/qualify-windows.ps1 -Cpu -Gpu -NativeWebview
pwsh -NoProfile -File scripts/qualify-windows.ps1 -SelectedDesktop
```

The second invocation physically operates only the guarded test windows on the
explicitly selected unlocked desktop. F12 stops injected input. The runner
records per-case exit status, bounded timeout and logs under
`target/verification/windows-matrix/`. Every invocation writes explicit uncovered
entries for unexecuted input/hardware/other-OS scenarios. A passed requested
subset does not close the full preview acceptance gates.
