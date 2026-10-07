# M0 runner contract and provisioning status

Updated 2026-10-07. Specifications are not executed coverage. The canonical
checklist remains the progress record; this document records required environments.

| Environment | Contract | Current evidence |
| --- | --- | --- |
| Hosted CPU | GitHub Actions windows-2025, ubuntu-24.04, macos-15; pinned Rust; pwsh; locked checks and independent consumer | All three passed [run 37658180508](https://github.com/FunToHard/view/actions/runs/37658180508); actual environments in [M0 evidence](evidence/m0-qualification.md#hosted-cpu-execution) |
| Windows GPU | Windows 11 x64; DX12-capable adapter and driver; optional separate Vulkan qualification | Local RTX 3050 detected; execution result in M0 evidence |
| Linux GPU | Linux native host, Vulkan loader and compatible device/driver; pwsh and Rust/linker | No registered runner or machine supplied |
| macOS GPU | Native macOS host with Metal device, Rust/linker and pwsh | No registered runner or machine supplied |
| Windows interactive | Dedicated unlocked Windows 11 x64 desktop; en-US keyboard and Japanese IME; named DPI/monitor configuration | Not provisioned/certified; no input injection run |

GitHub read-only runner query returned zero self-hosted runners on 2026-10-07.
The repository is public; a private personal desktop must not run untrusted PR
code. CPU jobs run on disposable hosted environments. The GPU workflow is manual
and targets labels `self-hosted`, `view-gpu`, and `windows`/`linux`/`macos`. Register
only owned, isolated machines, using a least-privilege runner account and reviewed
revisions. Registration tokens are never stored in this repository. Provisioning
new paid infrastructure is a separate owner choice.

Before dispatching a GPU job, verify runner availability and record OS, GPU,
driver/backend, power state, toolchain and fixture manifest. `scripts/verify.ps1
-Gpu -Backend dx12` (or vulkan/metal) returns failure on unavailable adapters,
timeouts or incorrect readback. Adapter device type is recorded; software results
are correctness evidence only, and unknown type is not assumed to be hardware.
The present compute/readback check does not certify the future 2D renderer or
native presentation. Add rendering fixtures in M3.

Interactive qualification is serialized per desktop. Require a stop mechanism,
foreground target validation, bounded operations and cleanup of only runner-held
keys/buttons. Record resolution, physical/logical DPI, display topology, input
method and installed IME version. Tests must not change the user's active language
or seize desktop input as part of ordinary `cargo test` or CPU CI. Add that driver
in M5 and run the native shell acceptance flow from M1. Until then M0-13 and the
dependent final gate stay open.

Hosted Windows build images are not evidence of Windows 11 shell behavior. A
Windows cross-compile is not Linux/macOS execution. Missing machines, credentials,
IME or adapters are reported as blocked/uncovered, never substituted with a green
skip in a required gate. See [validation contracts](../design/08-delivery-and-validation.md).
