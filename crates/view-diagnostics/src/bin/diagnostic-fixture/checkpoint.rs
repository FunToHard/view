//! Application-specific example, deliberately outside the framework API.
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    path::Path,
};

#[derive(Serialize, Deserialize)]
pub struct Document {
    pub schema: u32,
    pub revision: u64,
    pub text: String,
    checksum: u64,
}
#[derive(Serialize)]
pub struct Recovery {
    pub candidate: Option<Document>,
    pub rejected: usize,
}

// Integrity check for the proof's bounded text, not authentication/encryption.
fn checksum(revision: u64, text: &str) -> u64 {
    revision
        .to_le_bytes()
        .into_iter()
        .chain(text.bytes())
        .fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
        })
}
pub fn save(directory: &Path, revision: u64, text: &str) -> io::Result<()> {
    if text.len() > 4096 {
        return Err(io::Error::other("document checkpoint exceeds app budget"));
    }
    let document = Document {
        schema: 1,
        revision,
        text: text.into(),
        checksum: checksum(revision, text),
    };
    let pending = directory.join(format!("checkpoint-{revision:020}.partial"));
    let committed = directory.join(format!("checkpoint-{revision:020}.json"));
    if committed.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "checkpoint revision already exists",
        ));
    }
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&pending)?;
    serde_json::to_writer(&mut file, &document)?;
    file.flush()?;
    file.sync_all()?;
    drop(file);
    // Same-directory publication after syncing bytes. This proves process-crash
    // recovery, not portable power-loss durability of directory metadata.
    fs::rename(pending, committed)?;
    Ok(())
}
pub fn recover(directory: &Path) -> io::Result<Recovery> {
    let mut candidate: Option<Document> = None;
    let mut rejected = 0;
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(revision) = name
            .to_str()
            .and_then(|s| s.strip_prefix("checkpoint-"))
            .and_then(|s| s.strip_suffix(".json"))
            .filter(|s| s.len() == 20 && s.bytes().all(|b| b.is_ascii_digit()))
            .and_then(|s| s.parse::<u64>().ok())
        else {
            continue;
        };
        if !entry.file_type()?.is_file() {
            continue;
        }
        let mut bytes = vec![];
        fs::File::open(entry.path())?
            .take(8193)
            .read_to_end(&mut bytes)?;
        let document = serde_json::from_slice::<Document>(&bytes);
        match document {
            Ok(document)
                if bytes.len() <= 8192
                    && document.schema == 1
                    && document.revision == revision
                    && document.text.len() <= 4096
                    && document.checksum == checksum(revision, &document.text) =>
            {
                if candidate
                    .as_ref()
                    .is_none_or(|prior| prior.revision < document.revision)
                {
                    candidate = Some(document);
                }
            }
            _ => rejected += 1,
        }
    }
    Ok(Recovery {
        candidate,
        rejected,
    })
}
