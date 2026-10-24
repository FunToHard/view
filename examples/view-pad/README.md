# View Pad

View Pad is a small native text editor example built on the `view` workspace.
It uses the retained runtime and `TextField` multiline editing engine, with a
hardware-wgpu surface and native window decorations.

Run it with:

```powershell
cargo run -p view-pad --release
cargo run -p view-pad --release -- path\to\notes.txt
```

The toolbar and keyboard provide New, Open, Save and Save As. Editing includes
multiline wrapping, tabs, selection, undo/redo, caret reveal, wheel scrolling,
IME composition and the platform clipboard. Ctrl+N/O/S and Ctrl+Shift+S are
supported; on macOS the command key is accepted for these shortcuts. Opening
or closing a changed document offers Save, Discard and Cancel. Files are read
and written as UTF-8 with LF newlines.

Windows uses View's owner-modal native file dialogs and Unicode clipboard.
Other platforms can open a path supplied on the command line, edit and save it;
the current platform adapter reports native file dialogs/clipboard as
unsupported there. The demo opts into wgpu's `MemoryUsage` allocation hint
while retaining hardware rendering. This is a memory-oriented example policy,
not a changed framework default.

This is an application proof, not a full Notepad replacement. It currently has
one document per window, no Find/Replace, print, tabs, encoding chooser, or
extended file format handling. Windows native IME/accessibility behavior and
interactive GPU performance still require their separately provisioned native
acceptance runs.
