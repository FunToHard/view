# Trusted native-process add-ons

`view-addons` implements protocol 1 over dedicated child-process stdio. Enable
`view/addons` for the facade, or use the crate directly. `addons-render` on the
facade / `render` on the crate enables the existing wgpu control painter. Core
and the default facade acquire no add-on, process, text-shaping or GPU dependency.
The optional add-on crate reuses serde/serde_json and the standard control/text
engines; rendering reuses view-wgpu. There are no new registry dependencies.

## Trust, installation and updates

An add-on is a **trusted executable with the user's actual OS privileges**. API
grants are not a filesystem, network, memory or CPU sandbox. It inherits the
host environment and current directory. There is no shell interpretation, DLL
loading, VM, JIT, automatic download, installer or global command registration.
Windows child creation uses the no-console-window flag. Diagnostics use stderr,
never the protocol's stdout, and the host retains at most 8 KiB of diagnostics.

The application selects an absolute executable path and validates a `Manifest`
with extension/version identity, protocol, arguments, components, slots, scopes
and command names. `Addon::install` registers that manifest in memory; it does
not copy files or imply a signature/trust decision. The app owns persistent
settings and its installation UI. To update, deactivate the old instance, wait
for its child and workers to exit, remove it and install the new manifest. There
is no stateful hot-reload or native Rust ABI compatibility promise. Protocol 1
requires an exact match. Unsupported required component versions fail before
launch; optional components may select their explicit registered fallback.

The process manager owns the direct child and its dedicated pipes. Cancellation
terminates that child and observes exit off the UI thread. It does not claim to
contain or terminate arbitrary descendants. Trusted guests must not leave
descendants holding inherited pipes. `ProcessReceipt` distinguishes child exit
from remaining pipe/monitor workers; restart refuses while either remains live.
No receipt silently promotes incomplete cleanup to success.

## Host integration and lifecycle

Use an `Addon` on the same thread as the existing application `Runtime`. Supply
retained runtime parents for each offered `Slot` and a runtime-owned cancellation
lease. Slot parents remain app-owned; the add-on owns only the namespaced roots
mounted beneath them. `activate` starts negotiation; `poll` drains bounded
messages, invokes app services, applies coherent UI batches and checks deadlines.
Pass monotonic elapsed time and commit the runtime after each turn. Pipe reading,
writing, diagnostics and process reaping run on bounded worker channels; the UI
thread never waits for a guest response.

States are Installed, Negotiating, Active, Deactivating, Failed, Disabled and
Removed. Deactivation, disable, failure and owner cancellation revoke commands
and pending work, unmount the contributed runtime subtrees, clear control focus /
composition / capture ownership and cancel the child. Keep pumping/rechecking the
receipt until exit and zero workers. `failure()` provides a host-owned diagnostic
state: the app displays this in its own status/error chrome, outside guest UI.
Explicit `deactivate`/`remove` while the runtime is available is required. Dropping
the process handle cancels the child but cannot mutate a separately owned runtime
or guarantee asynchronous completion.

Each activation gets a new generation. Only the initial Hello uses generation
zero; subsequent guest messages must carry the Welcome generation. A stale
generation cannot mutate a replacement instance. Commands discovered for
automation also carry the generation and expire on deactivation/restart.

## Wire contract, bounds and permissions

Frames contain a four-byte little-endian byte length followed by exactly one
UTF-8 JSON object. Zero/oversized lengths fail before payload allocation; partial,
malformed, unknown-field and incompatible messages fail the session. Maximum
frame size is 256 KiB. Message types and guest builders are public Rust types;
borrowed closures, raw runtime handles and Rust object layouts never cross IPC.

Defaults per instance: 128 UI nodes, 128 KiB guest UI descriptions, 32 messages
in each pipe queue, 32 queued events plus 32 outstanding events, 256 incoming
messages per host-clock second, five-second startup and five-second event
deadlines. Properties are bounded to 4 KiB. Configurable limits have hard caps.
Text controls additionally use bounded shared-editor history and a 4096-grapheme
local value limit. These are protocol/control budgets, not OS process memory or
CPU limits. An oversized host service result cannot be sent as an unbounded frame.

The manifest's requested scopes must be a subset of explicit host grants.
UI rendering does not grant document reads, document writes, automation, MCP,
desktop input, filesystem APIs or arbitrary app commands. Host action registries
contain typed argument validators; unknown actions fail before application
dispatch. Guests must send strictly increasing nonzero document request IDs.
Duplicate/reordered IDs cannot replay document effects; delivery loss does not
promise exactly-once effects across restarts. Apps retain their own transaction
receipts if that stronger contract is needed.

## Host-owned UI and controls

The initial subset is flat, keyed label/button/single-line text rows in offered
side-panel, toolbar and menu surfaces. Rows are ordered by key, 280 logical units
wide, 44 high with a 48-unit pitch. These are host-rendered surfaces, not automatic
Win32 native menu registration. Names include extension ID, generation and key.
The guest cannot choose an arbitrary runtime parent or impersonate host chrome.

Snapshots and Put/Remove patches carry the current base revision. Stale UI
batches return Resync with the host revision; invalid schemas, duplicate keys,
unknown controls and excessive trees are rejected before applying the batch.
Stable nodes retain their runtime identity and editor session. `Registry`
accepts host-compiled schema decoders that map to the shipped control kinds.
Optional fallback uses that decoder, not a guest-supplied native implementation.
New control behavior requires a host build/adapter; naming a crate or DLL cannot
install code through a UI patch.

`MountedUi` uses normal core nodes, semantics, modal focus and capture cleanup.
Buttons reuse `Pressable`; text uses the normal `TextField` editor, selection,
history, clipboard and composition behavior. The host routes committed hit/key
events through the normal runtime and calls `Addon::input`; pointer positions
use window-logical coordinates consistent with the prepared control geometry.
The host retains clipboard, native IME candidate publication and physical-input
routing. `control` exposes the retained engine for those platform adapters.
Keyboard focus must belong to the text node before text input is accepted.

Local editing completes before IPC. Unsent text values coalesce by key while
button/command activations keep separate queue entries. Queue saturation returns
an explicit limit error so required actions can be retried. Guest validation is
asynchronous, tagged with event and value revision; expired/older validation is
ignored. A guest echo of its previous value cannot overwrite local edits or
preedit. Actual external replacements require the current value revision and no
active composition. Hosts show `TextField::invalid` in their chosen decoration;
guest validation never silently rolls back committed text.

The optional painter uses these same sessions and the existing font/renderer
stack. It preserves accumulated clips and supports translated rectangular slots;
nontranslation transforms return an explicit incompatibility. Hosts needing more
composition can supply their own adapter over the public control engines. Host
theme, DPI, layout policy, renderer submission and window ownership stay local.
Inherited standard-control native certification gaps remain gaps; add-ons do not
turn headless IME tests into physical Japanese input or native AT evidence.

## Document commands and automation

Implement `Application` in the application. Snapshot data is an owned JSON value
with the application's revision. A registered edit receives typed/validated
arguments and a revision precondition. The application must check the revision,
selection/range and permissions before committing one atomic, undoable document
transaction. No document type or unrestricted mutable UI/document reference is
provided to a guest. Host service methods must be short UI-thread operations;
long guest computation uses events and their cancellation/deadlines.

`commands()` discovers only active, manifest-declared commands explicitly
allowlisted by the host for automation and granted the Automation scope. The host
may register those descriptors with its existing CLI/MCP app command dispatcher.
`invoke` checks current generation and authorization again. No MCP server or
desktop-input capability is started implicitly.

## Review proof and verification

The [review-addon binary](../../crates/view-addons/src/bin/review-addon.rs) includes
an actual native guest and a host-owned Word-like plain-document proof. It mounts
a suggestion list/label, note field and side/menu/toolbar actions, requests a
snapshot, accepts a suggestion, checks the document revision and expected range,
commits one undo record, restores the original text and observes full cleanup.
It is not a Word file-format implementation or a rich-text editor.

```powershell
cargo run -p view-addons --bin review-addon --locked -- --proof
cargo run -p view-addons --features render --bin review-addon --locked -- --gpu dx12
cargo test -p view-addons --all-features --locked
pwsh -File scripts/verify.ps1
```

GPU execution is explicit and writes `target/verification/m7-review.bmp`. Default
tests launch owned local fixture children but no window, browser, GPU or physical
input. Windows execution and platform limitations are recorded in
[M7 evidence](../plan/evidence/m7-addons.md).
