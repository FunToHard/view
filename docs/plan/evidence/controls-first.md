# Controls-first implementation — 2026-10-23

Owner-authorized priority change: develop UI controls/gallery before remaining
framework milestones. Existing unchecked framework and release gates stay open;
no platform, IME or accessibility coverage is reclassified as passed.

## UI-01: horizontal tab list and gallery

- Added public TabList/TabOption in view-controls, reusing RadioGroup selection.
  Stable unique keys, required enabled selection, atomic rejection of disabled/
  missing keys, horizontal automatic activation, Home/End and focus gating.
- Tab stays with the host focus manager; vertical arrows and modified chords are
  not consumed. Key-up does not reactivate. No hidden global state or dependency.
- Added selected tab, tab-list and tab-panel AccessKit role mapping through the
  existing custom-role extension; checked state is not substituted for selection.
- Extended the existing gallery with Overview/Keyboard/Coverage panels and a
  disabled Later tab. Existing button/text/checkbox/radio/range/list/tree states
  remain present. Gallery size is now 900x740 logical pixels.
- The host retains one descriptive panel and derives its displayed/semantic text
  from the selection. This is not a general dynamic panel lifecycle or docking
  implementation. Native tab-to-panel relationships remain uncovered.

## Executed acceptance

- view-controls with gallery/all targets: 13 contract tests, 2 accessibility tests
  and 2 gallery tests passed. New cases cover disabled/wrapped selection, focus and
  modifier gating, Home/End, invalid construction, selected semantics, panel change
  and translated disabled/tab/panel roles.
- DX12 offscreen gallery passed at 100/150/200% scaling with existing selection/
  preedit/caret pixel assertions. The 100% BMP was visually inspected; tabs and
  panel text are visible and do not overlap other controls.
- Hidden native gallery passed: surface rendering, AccessKit installation/update,
  IME candidate geometry service call and teardown. This is not native client or
  physical input certification.
- Full scripts/verify.ps1 log completed through the final successful platform probe: formatting, workspace check/clippy/tests/docs, supported feature profiles and independent facade consumers. Changed documentation links/fences and git diff whitespace checks also passed.

Local artifacts: target/controls-first-verification.log,
target/controls-gallery-gpu.log, target/controls-gallery-hidden.log,
target/verification/gallery-1.bmp (also 1.5 and 2 scaling captures).
GPU: Windows, NVIDIA RTX 3050 6GB Laptop, DX12, driver 32.0.16.1714.
No new Linux/macOS native/GPU execution, remote CI or release qualification claimed.

Run commands and the exact tab contract: [controls guide](../../guides/controls.md).
Canonical remaining work: [implementation checklist](../implementation-checklist.md).
