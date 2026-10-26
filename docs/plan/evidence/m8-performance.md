# Windows workload evidence — 2026-10-25

Implementation and commands: [performance qualification](../../guides/performance-qualification.md).
This report supports M8A-08 through M8A-11 without claiming the full workload,
physical-input or platform matrix complete.

Three release hardware DX12 runs completed with the default performance memory
policy on AMD Ryzen 5 7235HS and NVIDIA RTX 3050 6GB Laptop GPU, driver
32.0.16.1714, OS build 10.0.26300. The binary SHA-256 was
`3D3BE3A98F92B5680069764C8EA5F123045B4ACA4750689026554A738FBE1775`.
Each run performed 1,000 resize requests and 1,000 alternating insert/delete
operations. All cache-limit checks passed and the final text was unchanged.

| Warm CPU elapsed series | Median of run p50 | Median of run p95 | Median of run p99 |
| --- | ---: | ---: | ---: |
| Resize total | 10.00 ms | 14.75 ms | 29.33 ms |
| Resize renderer preparation | 0.71 ms | 1.25 ms | 1.66 ms |
| Resize surface acquisition/submission | 8.65 ms | 12.43 ms | 27.49 ms |
| Edit total | 10.01 ms | 10.48 ms | 11.62 ms |
| Edit renderer preparation | 0.34 ms | 0.60 ms | 0.85 ms |
| Edit surface acquisition/submission | 9.54 ms | 10.02 ms | 11.28 ms |

These samples indicate that the surface path dominates this fixture's elapsed
CPU time. They do not identify GPU execution time or visible input latency.
Some development activity overlapped the first run; this is exploratory evidence,
not a dedicated-machine regression certification or a reason to add speculative
render threads.

Median private commitment was 319.68 MiB at idle controls, 327.18 MiB after
resizing and 328.62 MiB after editing. Median working sets were 156.45, 164.22
and 165.65 MiB respectively. Device creation alone accounted for 311.57 MiB
median private commitment. This confirms the earlier distinction between device
overhead and control payload; it does not prove every driver allocation is freed
immediately. Default allocator hints were retained, and no software renderer was
used. Alternative allocator policies remain explicit probe variants.

Raw samples, stage logs, timing distributions and environment are in
`target/verification/gpu-memory/default/`. They are generated local artifacts.
The new CPU stress workload and optional baseline comparator have separate
execution receipts; results must not be inferred from their presence.

Still unqualified: GPU timestamp timing, visible input latency, OS wakeups,
allocator event counts, per-frame upload volumes, hot/cold WebView costs,
sleep/display removal, sustained real multiwindow/canvas GPU pressure and a
dedicated quiet-host numeric regression budget. Other-OS GPU coverage requires
those machines. These omissions keep composite M8A gates open.
