# Controls expansion contracts — 2026-10-25

2026-10-26 integration review: the physical work-area query now requires a
per-monitor-DPI-aware caller and inspects that context without changing it.
The final hidden docking proof passed this guard and the full restoration cycle;
log: `target/verification/m8-docking-final.log`. This does not certify physical
cross-monitor DPI transitions or native activation/IME.

This implementation adds the bounded F-01 password-state contract and F-02
docking model/native lifecycle fixture in `view-controls`. It does not certify
either standard-control profile or complete native docking interaction.

## Password field

- `PasswordField` masks display by default, has an explicit reveal switch, and
  omits its secret from semantic values. Application submission uses the
  deliberately named `expose_value` accessor.
- Disabled mode clears focus and reveal state and rejects value mutation.
- The API does not promise zeroization, locked memory, or secret-safe cloning.
  The shared editor masks grapheme clusters and translates caret/pointer indices.
  Clipboard copy/cut are denied; reveal retains semantic-value suppression.

## Dock model

- `DockLayout` validates unique window/stack/tab IDs, active stack members, split
  ratios, and finite positive window bounds before accepting or restoring a
  snapshot.
- Tab moves and tear-off are transactional: invalid requests leave the previous
  snapshot unchanged. Snapshots have a validated versioned binary codec with
  bounded input. Runtime reconciliation suspends inactive panels and revokes
  focus/capture; the host delivers the returned cancellation actions.
- Typed `DockCommand` planning returns a validated `DockTransition` and explicit
  cross-window panel migrations before host effects. Stale commits are rejected.
  Hosts stage fallible window/mount creation first, cancel/deliver old capture,
  unmount the old panel and commit on their UI thread. Application document,
  caret and revision state remain outside the disposable UI mount. Empty source
  stacks/windows retain stable IDs; closing them is an explicit host policy.
- `restore_in_work_areas` chooses the greatest saved-window overlap, otherwise
  the nearest available area center, and clamps size/origin to that work area.
  Minimum sizes are capped by the work-area size. Native snapshots store outer
  position and size in physical virtual-desktop pixels, including negative
  coordinates. Logical client sizes must be converted by hosts. The fixture
  subtracts measured native decorations when setting client size, and verifies
  actual outer bounds against restored bounds.
- `view-platform::monitor::work_area_at` queries Windows taskbar-excluded monitor
  bounds. Other platforms explicitly report unsupported. Its Win32 pointer and
  borrowed-handle safety invariants are documented at the adapter calls. It adds
  the Graphics/GDI binding feature to the existing pinned Windows dependency;
  no package/version or core dependency is added. The optional `docking` example
  profile enables the shell/work-area adapter without gallery accessibility,
  clipboard/text services, GPU or desktop-service features.
- Runtime reconciliation resolves every handle's live window ownership and
  validates missing/stale/wrong-window/overlapping panel roots before effects.
  `Runtime::node_window` provides current generation-checked ownership rather
  than relying on an older committed snapshot. Cancellation order is stable.

## Native docking fixture and remaining boundary

Run from the repository root:

```text
cargo run -p view-controls --example docking --features docking --locked -- --verify-hidden
```

The hidden fixture opens one native decorated window, plans a tab tear-off,
stages a second native window and new runtime mount, cancels logical capture,
revokes the old mount's task lease, transfers runtime focus and preserves the
application's unsaved document/caret/revision. It writes the validated binary
layout to `target/docking-layout.vdk`, closes both native/logical owners,
decodes/restores the layout, recreates two distinct native windows, verifies
their actual physical outer bounds and application state, restores focus to the
application-selected active panel, and verifies final runtime cleanup. All
native windows are hidden and inactive; it never seizes physical input.

The fixture's focus checks concern the runtime's focus model. OS foreground
activation, real OS capture release, IME session migration, keyboard/accessible
tab presentation and pointer drag/drop hit-testing are still uncovered. There
is no rendered docking surface or native tab-drag UX in this fixture. Calling
`DockLayout::tear_off` alone only changes data; native creation remains an
explicit host step. Full F-02 docking is therefore still open.
Negative-coordinate and removed-monitor restoration are synthetic CPU cases;
the native fixture uses the actual monitor nearest `[0, 0]`. No physical
multi-monitor removal, cross-DPI move or alternate-platform work-area result is
claimed.

## Verification

Focused Windows verification for the docking increment:

- `cargo test -p view-controls --test password_docking --locked`: 12 passed,
  covering atomic/stale commands, codec truncations, duplicate generated stack
  IDs, negative/removed monitors and malformed work areas, plus reconciliation
  prevalidation and the existing password tests.
- `cargo test -p view-core --test node_ownership --locked`: 1 passed, covering
  live, removed, recycled and foreign node ownership queries.
- `cargo test -p view-platform --lib monitor::tests --locked`: 1 passed,
  covering checked negative/overflow bounds. The actual monitor query runs only
  in the explicit hidden fixture so ordinary CPU/headless tests do not require
  an attached monitor.
- `cargo run -p view-controls --example docking --features docking --locked --
  --verify-hidden`: passed the actual work-area query and two-native-window
  tear-off/persistence/restoration/cleanup assertions on this Windows host.
- `cargo clippy -p view-controls --all-targets --features docking --locked --
  -D warnings`: passed in the standalone docking profile.
- `cargo tree -p view-controls --features docking -e normal --prefix none`:
  the profile includes `view-platform` and no `view-wgpu`, AccessKit or WebView
  package. `cargo fmt --all -- --check`: passed.

Formatting and the remaining workspace gates are delegated to the parent
integration run. No Linux/macOS execution is claimed.
