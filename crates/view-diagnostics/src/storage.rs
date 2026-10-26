use crate::PreparedReport;
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

/// Healthy-operation local storage with explicit directory and count/byte limits.
/// Never call this from a panic hook: directory scans and sync may block.
/// Only regular `view-report-<20 decimal digits>.json` files participate in
/// retention. The host must exclusively own the directory and its ACLs.
pub struct ReportStore {
    directory: PathBuf,
    retained: usize,
    max_bytes: usize,
}
impl ReportStore {
    pub fn new(
        directory: impl Into<PathBuf>,
        retained: usize,
        max_bytes: usize,
    ) -> io::Result<Self> {
        if !(1..=1000).contains(&retained) || !(1..=1_048_576).contains(&max_bytes) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid report retention budget",
            ));
        }
        let directory = directory.into();
        fs::create_dir_all(&directory)?;
        Ok(Self {
            directory,
            retained,
            max_bytes,
        })
    }

    /// The host chooses a monotonic sequence; reuse is rejected without overwrite.
    /// Retention errors propagate even when the newly written report is durable.
    pub fn save(&self, sequence: u64, report: &PreparedReport) -> io::Result<PathBuf> {
        if report.bytes().len() > self.max_bytes {
            return Err(io::Error::other("report exceeds storage byte budget"));
        }
        let path = self
            .directory
            .join(format!("view-report-{sequence:020}.json"));
        let mut reports = Vec::new();
        for entry in fs::read_dir(&self.directory)? {
            let entry = entry?;
            if entry.file_type()?.is_file() && owned_name(&entry.path()) {
                if reports.len() == 1000 {
                    return Err(io::Error::other(
                        "report directory exceeds retention scan budget",
                    ));
                }
                reports.push(entry.path());
            }
        }
        reports.sort();
        if reports.last().is_some_and(|prior| prior >= &path) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "report sequence must increase",
            ));
        }
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)?;
        file.write_all(report.bytes())?;
        file.sync_all()?;
        reports.push(path.clone());
        let excess = reports.len().saturating_sub(self.retained);
        for path in reports.into_iter().take(excess) {
            fs::remove_file(path)?;
        }
        Ok(path)
    }
}
fn owned_name(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_prefix("view-report-"))
        .and_then(|name| name.strip_suffix(".json"))
        .is_some_and(|number| number.len() == 20 && number.bytes().all(|b| b.is_ascii_digit()))
}
