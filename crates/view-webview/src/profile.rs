use crate::{Error, Result, SessionId};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Default)]
pub enum Profile {
    #[default]
    EphemeralDedicated,
    /// Caller-owned persistent data. Never deleted by this adapter.
    Persistent(PathBuf),
    /// Explicit shared process/profile ownership. Never deleted or killed.
    Shared(PathBuf),
}
#[derive(Clone, Debug)]
pub struct ProfileLease {
    pub(crate) path: PathBuf,
    pub(crate) root: PathBuf,
    pub(crate) session: SessionId,
    pub(crate) ephemeral: bool,
    pub(crate) shared: bool,
}
impl ProfileLease {
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn ephemeral(&self) -> bool {
        self.ephemeral
    }
    pub fn shared(&self) -> bool {
        self.shared
    }
    pub fn session(&self) -> SessionId {
        self.session
    }
}
pub struct Profiles {
    root: PathBuf,
}
impl Profiles {
    /// Use a dedicated application directory, never a personal browser profile.
    pub fn new(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref();
        if root.exists() {
            no_links(root)?;
            if fs::read_to_string(root.join(".view-webview-root"))
                .ok()
                .as_deref()
                != Some("view-webview-v1")
            {
                return Err(Error::Invalid(
                    "existing directory is not an owned webview root".into(),
                ));
            }
        } else {
            fs::create_dir_all(root)?;
            fs::write(root.join(".view-webview-root"), "view-webview-v1")?;
        }
        Ok(Self {
            root: fs::canonicalize(root)?,
        })
    }
    pub(crate) fn allocate(&self, session: SessionId, profile: &Profile) -> Result<ProfileLease> {
        let (path, ephemeral, shared) = match profile {
            Profile::EphemeralDedicated => {
                let path = self.root.join(format!("session-{}", session.0));
                fs::create_dir(&path)?;
                fs::write(path.join(".view-owner"), session.0.to_string())?;
                (path, true, false)
            }
            Profile::Persistent(path) | Profile::Shared(path) => {
                if !path.is_absolute() {
                    return Err(Error::Invalid("profile path must be absolute".into()));
                }
                fs::create_dir_all(path)?;
                (
                    fs::canonicalize(path)?,
                    false,
                    matches!(profile, Profile::Shared(_)),
                )
            }
        };
        Ok(ProfileLease {
            path,
            root: self.root.clone(),
            session,
            ephemeral,
            shared,
        })
    }
    /// Startup reconciliation is conservative: discovery is not proof of browser exit.
    /// Return validated orphan records for host diagnostics; never delete them on startup.
    pub fn pending_from_previous_run(&self) -> Result<Vec<PathBuf>> {
        let mut pending = vec![];
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            if entry.file_name().to_string_lossy().starts_with("session-") {
                no_links(&entry.path())?;
                let marker = entry.path().join(".view-owner");
                no_links(&marker)?;
                let owner = fs::read_to_string(marker)?;
                if owner.parse::<u64>().is_ok()
                    && entry.file_name().to_string_lossy() == format!("session-{owner}")
                {
                    pending.push(entry.path());
                }
            }
        }
        Ok(pending)
    }
}
fn no_links(path: &Path) -> Result<()> {
    let meta = fs::symlink_metadata(path)?;
    if meta.file_type().is_symlink() {
        return Err(Error::Denied);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return Err(Error::Denied);
        }
    }
    Ok(())
}
fn inspect_tree(path: &Path) -> Result<()> {
    no_links(path)?;
    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            inspect_tree(&entry?.path())?;
        }
    }
    Ok(())
}
/// Called only after the matching provider process-group exit receipt.
/// Never follows profile links or accepts a path recovered from untrusted metadata.
pub(crate) fn remove_owned(lease: ProfileLease) -> Result<()> {
    if !lease.ephemeral || lease.shared {
        return Err(Error::Denied);
    }
    no_links(&lease.root)?;
    let canonical = fs::canonicalize(&lease.path)?;
    if canonical.parent() != Some(lease.root.as_path())
        || canonical.file_name()
            != Some(std::ffi::OsStr::new(&format!(
                "session-{}",
                lease.session.0
            )))
        || fs::read_to_string(canonical.join(".view-owner"))? != lease.session.0.to_string()
    {
        return Err(Error::Denied);
    }
    inspect_tree(&canonical)?;
    fs::remove_dir_all(canonical)?;
    Ok(())
}
