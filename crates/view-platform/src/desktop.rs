//! Explicit, owner-window-scoped desktop effects. Calling an adapter is an
//! application action; build, measure and paint must never call these functions.
use crate::PlatformError;
use std::path::PathBuf;
use winit::window::Window;

/// Single-selection filesystem dialog policy (no implicit file reads/writes).
#[derive(Clone, Copy, Debug)]
pub enum DialogKind {
    /// Pick an existing file.
    Open,
    /// Choose a destination, with native overwrite confirmation.
    Save,
    /// Pick an existing directory.
    Folder,
}
/// Explicit shell target, without command-line arguments or script execution APIs.
pub enum ShellTarget {
    /// Open an absolute filesystem path with its registered handler.
    File(PathBuf),
    /// Open an HTTPS URL with its registered handler.
    Https(String),
}
impl ShellTarget {
    /// Validate before entering a platform adapter. The app decides whether to
    /// offer this effect; validation is not a sandbox or trust decision.
    pub fn validate(&self) -> Result<String, PlatformError> {
        let value = match self {
            Self::File(path) if path.is_absolute() => path.to_str().map(str::to_owned),
            Self::Https(url)
                if url.starts_with("https://")
                    && url.len() > 8
                    && !url.chars().any(char::is_whitespace) =>
            {
                Some(url.clone())
            }
            _ => None,
        }
        .filter(|s| !s.contains('\0'));
        value.ok_or_else(|| PlatformError::UnsupportedCapability("invalid shell target".into()))
    }
}
/// Show a native owner-modal dialog. Cancellation returns `None`. Linux/macOS
/// currently return an explicit unsupported capability; no shell fallback.
pub fn file_dialog(
    window: &Window,
    kind: DialogKind,
    title: &str,
) -> Result<Option<PathBuf>, PlatformError> {
    if title.contains('\0') {
        return Err(PlatformError::UnsupportedCapability(
            "dialog title contains NUL".into(),
        ));
    }
    #[cfg(windows)]
    {
        native::file_dialog(window, kind, title)
    }
    #[cfg(not(windows))]
    {
        let _ = (window, kind, title);
        Err(PlatformError::UnsupportedCapability(
            "file dialogs on this platform".into(),
        ))
    }
}
/// Activate a validated target on behalf of the application. Does not invoke a
/// command interpreter, pass arguments, or return ownership of an external app.
pub fn activate(window: &Window, target: &ShellTarget) -> Result<(), PlatformError> {
    let value = target.validate()?;
    #[cfg(windows)]
    {
        native::activate(window, &value)
    }
    #[cfg(not(windows))]
    {
        let _ = (window, value);
        Err(PlatformError::UnsupportedCapability(
            "shell activation on this platform".into(),
        ))
    }
}
/// Native context menu at a physical screen position. Returns an index into the
/// caller-owned commands; the app performs the selected command after dismissal.
pub fn context_menu(
    window: &Window,
    screen: (i32, i32),
    commands: &[(&str, bool)],
) -> Result<Option<usize>, PlatformError> {
    if commands.len() > 1024 || commands.iter().any(|(label, _)| label.contains('\0')) {
        return Err(PlatformError::UnsupportedCapability(
            "invalid context menu".into(),
        ));
    }
    #[cfg(windows)]
    {
        native::context_menu(window, screen, commands)
    }
    #[cfg(not(windows))]
    {
        let _ = (window, screen, commands);
        Err(PlatformError::UnsupportedCapability(
            "native context menu on this platform".into(),
        ))
    }
}

#[cfg(windows)]
mod native {
    use super::*;
    use windows::{
        Win32::{
            Foundation::HWND,
            System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, CoTaskMemFree},
            UI::{Shell::*, WindowsAndMessaging::*},
        },
        core::{Interface, PCWSTR},
    };
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    fn owner(window: &Window) -> Result<HWND, PlatformError> {
        match window
            .window_handle()
            .map_err(|e| PlatformError::WindowCreation(e.to_string()))?
            .as_raw()
        {
            RawWindowHandle::Win32(h) => {
                let hwnd = HWND(h.hwnd.get() as *mut _);
                // SAFETY: the HWND is borrowed from a live winit window. These
                // queries have no pointer outputs and validate UI-thread affinity.
                let same_thread = unsafe {
                    GetWindowThreadProcessId(hwnd, None)
                        == windows::Win32::System::Threading::GetCurrentThreadId()
                };
                if !same_thread {
                    return Err(PlatformError::ThreadAffinityViolation);
                }
                Ok(hwnd)
            }
            _ => Err(PlatformError::UnsupportedCapability("Win32 owner".into())),
        }
    }
    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(Some(0)).collect()
    }
    fn error(e: windows::core::Error) -> PlatformError {
        PlatformError::EventLoop(e.to_string())
    }
    pub(super) fn file_dialog(
        window: &Window,
        kind: DialogKind,
        title: &str,
    ) -> Result<Option<PathBuf>, PlatformError> {
        let hwnd = owner(window)?;
        let _com = crate::ComGuard::new()?;
        let title = wide(title);
        // SAFETY: COM calls occur synchronously on the UI thread while its COM
        // guard and borrowed owner window live. All UTF-16 buffers are terminated
        // and outlive calls. Interface RAII releases before the apartment guard.
        unsafe {
            let dialog: IFileDialog = match kind {
                DialogKind::Save => CoCreateInstance::<_, IFileSaveDialog>(
                    &FileSaveDialog,
                    None,
                    CLSCTX_INPROC_SERVER,
                )
                .map_err(error)?
                .cast()
                .map_err(error)?,
                _ => CoCreateInstance::<_, IFileOpenDialog>(
                    &FileOpenDialog,
                    None,
                    CLSCTX_INPROC_SERVER,
                )
                .map_err(error)?
                .cast()
                .map_err(error)?,
            };
            let flags = dialog.GetOptions().map_err(error)?
                | FOS_FORCEFILESYSTEM
                | FOS_NOCHANGEDIR
                | match kind {
                    DialogKind::Open => FOS_FILEMUSTEXIST,
                    DialogKind::Save => FOS_OVERWRITEPROMPT,
                    DialogKind::Folder => FOS_PICKFOLDERS,
                };
            dialog.SetOptions(flags).map_err(error)?;
            dialog.SetTitle(PCWSTR(title.as_ptr())).map_err(error)?;
            if let Err(e) = dialog.Show(Some(hwnd)) {
                if e.code().0 as u32 == 0x800704c7 {
                    return Ok(None);
                }
                return Err(error(e));
            }
            let item = dialog.GetResult().map_err(error)?;
            let path = item.GetDisplayName(SIGDN_FILESYSPATH).map_err(error)?;
            let value = path.to_string();
            CoTaskMemFree(Some(path.0.cast()));
            Ok(Some(PathBuf::from(
                value.map_err(|e| PlatformError::EventLoop(e.to_string()))?,
            )))
        }
    }
    pub(super) fn activate(window: &Window, value: &str) -> Result<(), PlatformError> {
        let hwnd = owner(window)?;
        let value = wide(value);
        let verb = wide("open");
        // SAFETY: borrowed live owner and terminated buffers stay valid during
        // the synchronous shell call; no parameters or directory are supplied.
        let result = unsafe {
            ShellExecuteW(
                Some(hwnd),
                PCWSTR(verb.as_ptr()),
                PCWSTR(value.as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            )
        };
        if result.0 as isize <= 32 {
            Err(PlatformError::EventLoop(format!(
                "shell activation failed: {}",
                result.0 as isize
            )))
        } else {
            Ok(())
        }
    }
    struct Menu(HMENU);
    impl Drop for Menu {
        fn drop(&mut self) {
            // SAFETY: this uniquely owned menu is created by CreatePopupMenu and is
            // never transferred to a window. TrackPopupMenuEx has returned.
            let _ = unsafe { DestroyMenu(self.0) };
        }
    }
    pub(super) fn context_menu(
        window: &Window,
        screen: (i32, i32),
        commands: &[(&str, bool)],
    ) -> Result<Option<usize>, PlatformError> {
        let hwnd = owner(window)?;
        // SAFETY: new menu has one RAII owner; all string pointers remain live
        // during AppendMenuW, which copies labels. Synchronous tracking borrows
        // the live window and returns before menu destruction.
        unsafe {
            let menu = Menu(CreatePopupMenu().map_err(error)?);
            for (i, (label, enabled)) in commands.iter().enumerate() {
                let label = wide(label);
                AppendMenuW(
                    menu.0,
                    MF_STRING | if *enabled { MF_ENABLED } else { MF_GRAYED },
                    i + 1,
                    PCWSTR(label.as_ptr()),
                )
                .map_err(error)?;
            }
            let result = TrackPopupMenuEx(
                menu.0,
                (TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON).0,
                screen.0,
                screen.1,
                hwnd,
                None,
            )
            .0;
            Ok((result > 0)
                .then(|| result as usize - 1)
                .filter(|i| commands.get(*i).is_some_and(|(_, enabled)| *enabled)))
        }
    }
}
