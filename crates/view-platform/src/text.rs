//! Optional shared-editor/native services. Windows clipboard access is explicit;
//! normal tests never read or overwrite the user's clipboard.

use crate::ImeEvent;
use view_text::{PlainEditor, Selection, TextError, TextIndex};

/// Feed a normalized winit IME event to the shared session exactly once.
pub fn dispatch_ime(
    editor: &mut PlainEditor,
    event: &ImeEvent,
    now_ms: u64,
) -> Result<(), TextError> {
    match event {
        ImeEvent::Enabled => Ok(()),
        ImeEvent::Disabled => {
            editor.cancel_composition();
            Ok(())
        }
        ImeEvent::Preedit(text, range) if text.is_empty() => {
            editor.cancel_composition();
            let _ = range;
            Ok(())
        }
        ImeEvent::Preedit(text, range) => {
            let (anchor, focus) = range.unwrap_or((text.len(), text.len()));
            editor.preedit(
                text,
                Selection {
                    anchor: TextIndex(anchor),
                    focus: TextIndex(focus),
                },
            )?;
            editor.set_composition_cursor_visible(range.is_some());
            Ok(())
        }
        ImeEvent::Commit(text) => editor.commit_composition(text, now_ms),
    }
}

/// Report the preedit or committed caret in window coordinates without guessing
/// a screen transform. Rendered/OS candidate placement still requires native E2E.
pub fn report_candidate(
    window: &winit::window::Window,
    editor: &PlainEditor,
    layout: &impl view_text::TextLayout,
    caret: view_text::Caret,
    control_origin: view_core::LogicalPoint,
    scale: view_core::ScaleFactor,
) -> Result<(), TextError> {
    if !control_origin.is_valid() {
        return Err(TextError::InvalidMetrics);
    }
    let mut rect = editor.candidate_rect(layout, caret)?;
    rect.origin.x += control_origin.x;
    rect.origin.y += control_origin.y;
    crate::ImeService::set_ime_cursor_area(window, rect, scale)
        .map_err(|_| TextError::InvalidMetrics)
}

#[cfg(windows)]
mod clipboard {
    use std::{marker::PhantomData, rc::Rc};
    use view_text::{Clipboard, TextError};
    use windows::Win32::{
        Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND},
        System::{
            DataExchange::{
                CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable,
                OpenClipboard, SetClipboardData,
            },
            Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock},
        },
    };
    const UNICODE_TEXT: u32 = 13;

    struct Open;
    impl Drop for Open {
        fn drop(&mut self) {
            // SAFETY: this guard exists only after OpenClipboard succeeds on this
            // thread; the !Send service never transfers the guard to another thread.
            let _ = unsafe { CloseClipboard() };
        }
    }
    /// Unicode Windows clipboard tied to a live owner window and its UI thread.
    /// Calls fail if another process has the clipboard open; retry is host policy.
    pub struct NativeClipboard<'a> {
        owner: HWND,
        _window: PhantomData<&'a winit::window::Window>,
        _thread: PhantomData<Rc<()>>,
    }
    impl<'a> NativeClipboard<'a> {
        /// Borrow a live winit window. Its lifetime and !Send marker confine use.
        pub fn new(window: &'a winit::window::Window) -> Result<Self, TextError> {
            use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
            match window
                .window_handle()
                .map_err(|_| TextError::Clipboard)?
                .as_raw()
            {
                RawWindowHandle::Win32(handle) => Ok(Self {
                    owner: HWND(handle.hwnd.get() as *mut _),
                    _window: PhantomData,
                    _thread: PhantomData,
                }),
                _ => Err(TextError::Unsupported),
            }
        }
        fn open(&self) -> Result<Open, TextError> {
            // SAFETY: owner is borrowed from a live window; service is !Send/!Sync.
            unsafe { OpenClipboard(Some(self.owner)) }.map_err(|_| TextError::Clipboard)?;
            Ok(Open)
        }
    }
    impl Clipboard for NativeClipboard<'_> {
        fn read(&mut self) -> Result<String, TextError> {
            let _guard = self.open()?;
            // SAFETY: clipboard remains open on this thread for all pointer use.
            unsafe {
                if IsClipboardFormatAvailable(UNICODE_TEXT).is_err() {
                    return Ok(String::new());
                }
                let handle = GetClipboardData(UNICODE_TEXT).map_err(|_| TextError::Clipboard)?;
                let memory = HGLOBAL(handle.0);
                let count = GlobalSize(memory) / 2;
                if count == 0 || count > isize::MAX as usize / 2 {
                    return Err(TextError::Clipboard);
                }
                let pointer = GlobalLock(memory).cast::<u16>();
                if pointer.is_null() {
                    return Err(TextError::Clipboard);
                }
                // The locked global allocation has count UTF-16 units; use only
                // initialized clipboard bytes, bounded by allocation and first NUL.
                let units = std::slice::from_raw_parts(pointer, count);
                let result = units
                    .iter()
                    .position(|&u| u == 0)
                    .ok_or(TextError::Clipboard)
                    .and_then(|end| {
                        String::from_utf16(&units[..end]).map_err(|_| TextError::Clipboard)
                    });
                let _ = GlobalUnlock(memory);
                result
            }
        }
        fn write(&mut self, text: &str) -> Result<(), TextError> {
            if text.contains('\0') {
                return Err(TextError::Clipboard);
            }
            let units: Vec<_> = text.encode_utf16().chain(std::iter::once(0)).collect();
            let bytes = units.len().checked_mul(2).ok_or(TextError::Clipboard)?;
            let _guard = self.open()?;
            // SAFETY: allocation is movable, exactly sized and exclusively owned
            // until SetClipboardData succeeds. Success transfers ownership to OS;
            // every failure frees it. No pointer survives unlocking/closing.
            unsafe {
                let memory = GlobalAlloc(GMEM_MOVEABLE, bytes).map_err(|_| TextError::Clipboard)?;
                let pointer = GlobalLock(memory).cast::<u16>();
                if pointer.is_null() {
                    let _ = GlobalFree(Some(memory));
                    return Err(TextError::Clipboard);
                }
                std::ptr::copy_nonoverlapping(units.as_ptr(), pointer, units.len());
                let _ = GlobalUnlock(memory);
                if EmptyClipboard().is_err()
                    || SetClipboardData(UNICODE_TEXT, Some(HANDLE(memory.0))).is_err()
                {
                    let _ = GlobalFree(Some(memory));
                    return Err(TextError::Clipboard);
                }
            }
            Ok(())
        }
    }
}
#[cfg(windows)]
pub use clipboard::NativeClipboard;
