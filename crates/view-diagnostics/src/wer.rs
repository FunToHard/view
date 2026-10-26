use std::{io, path::PathBuf};

#[derive(Clone, Copy)]
pub enum DumpType {
    Mini,
    Full,
}

/// Reviewable per-executable WER setup. Generates a .reg document; never edits
/// registry/global exception policy, creates folders, or enables collection.
pub struct WerLocalDumps {
    executable: String,
    folder: PathBuf,
    count: u32,
    kind: DumpType,
}
impl WerLocalDumps {
    pub fn new(executable: &str, folder: PathBuf, count: u32, kind: DumpType) -> io::Result<Self> {
        if executable.is_empty()
            || executable.len() > 255
            || !executable.ends_with(".exe")
            || executable.chars().any(|c| {
                matches!(
                    c,
                    '\\' | '/' | '"' | '[' | ']' | ':' | '*' | '?' | '<' | '>' | '|'
                ) || c.is_control()
            })
            || !(1..=1000).contains(&count)
            || !folder.is_absolute()
            || folder
                .to_str()
                .is_none_or(|s| s.len() > 4096 || s.chars().any(|c| c == '"' || c.is_control()))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid per-executable WER configuration",
            ));
        }
        Ok(Self {
            executable: executable.into(),
            folder,
            count,
            kind,
        })
    }
    /// UTF-8 text; an administrator can review and import explicitly using an
    /// appropriate registry editor. Folder ACLs and compatibility remain host policy.
    pub fn registry_file(&self) -> String {
        // REG_EXPAND_SZ uses UTF-16LE hex(2) with a terminal NUL in .reg files.
        let folder = self
            .folder
            .to_str()
            .unwrap_or_default()
            .encode_utf16()
            .chain([0])
            .flat_map(u16::to_le_bytes)
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join(",");
        let kind = match self.kind {
            DumpType::Mini => 1,
            DumpType::Full => 2,
        };
        format!(
            "Windows Registry Editor Version 5.00\r\n\r\n[HKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Windows\\Windows Error Reporting\\LocalDumps\\{}]\r\n\"DumpFolder\"=hex(2):{folder}\r\n\"DumpCount\"=dword:{:08x}\r\n\"DumpType\"=dword:{kind:08x}\r\n",
            self.executable, self.count
        )
    }
}
