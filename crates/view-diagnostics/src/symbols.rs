use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    path::Path,
};

/// CodeView RSDS identity of a Windows executable's matching PDB. The embedded
/// developer filesystem path is deliberately never returned or reported.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PdbIdentity {
    pub guid: [u8; 16],
    pub age: u32,
}
impl PdbIdentity {
    /// Conventional symbol-server GUID+age key (GUID fields use Windows order).
    pub fn symbol_key(&self) -> String {
        let g = self.guid;
        format!(
            "{:08X}{:04X}{:04X}{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}{:X}",
            u32::from_le_bytes([g[0], g[1], g[2], g[3]]),
            u16::from_le_bytes([g[4], g[5]]),
            u16::from_le_bytes([g[6], g[7]]),
            g[8],
            g[9],
            g[10],
            g[11],
            g[12],
            g[13],
            g[14],
            g[15],
            self.age
        )
    }
}
fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "invalid or unsupported PE debug data",
    )
}
fn u16_at(bytes: &[u8], offset: usize) -> io::Result<u16> {
    Ok(u16::from_le_bytes(
        bytes
            .get(offset..offset + 2)
            .ok_or_else(invalid)?
            .try_into()
            .map_err(|_| invalid())?,
    ))
}
fn u32_at(bytes: &[u8], offset: usize) -> io::Result<u32> {
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..offset + 4)
            .ok_or_else(invalid)?
            .try_into()
            .map_err(|_| invalid())?,
    ))
}
fn read_at(file: &mut File, length: u64, offset: u64, count: usize) -> io::Result<Vec<u8>> {
    if count > 4096
        || offset
            .checked_add(count as u64)
            .is_none_or(|end| end > length)
    {
        return Err(invalid());
    }
    file.seek(SeekFrom::Start(offset))?;
    let mut bytes = vec![0; count];
    file.read_exact(&mut bytes)?;
    Ok(bytes)
}

/// Inspect a PE32/PE32+ executable while healthy, with bounded reads and no native
/// calls. `None` means no RSDS record; malformed input is an error. This does not
/// load executable code, locate/upload a PDB, or prove that a PDB file is retained.
pub fn read_pdb_identity(path: &Path) -> io::Result<Option<PdbIdentity>> {
    let mut file = File::open(path)?;
    let length = file.metadata()?.len();
    let dos = read_at(&mut file, length, 0, 64)?;
    if &dos[..2] != b"MZ" {
        return Err(invalid());
    }
    let pe_offset = u64::from(u32_at(&dos, 60)?);
    let pe = read_at(&mut file, length, pe_offset, 24)?;
    if &pe[..4] != b"PE\0\0" {
        return Err(invalid());
    }
    let sections = usize::from(u16_at(&pe, 6)?);
    if sections > 96 {
        return Err(invalid());
    }
    let optional_size = usize::from(u16_at(&pe, 20)?);
    let optional = read_at(&mut file, length, pe_offset + 24, optional_size)?;
    let directory_start = match u16_at(&optional, 0)? {
        0x10b => 96,
        0x20b => 112,
        _ => return Err(invalid()),
    };
    if u32_at(&optional, directory_start - 4)? <= 6 {
        return Ok(None);
    }
    let debug_rva = u32_at(&optional, directory_start + 6 * 8)?;
    let debug_size = u32_at(&optional, directory_start + 6 * 8 + 4)?;
    if debug_rva == 0 || debug_size == 0 {
        return Ok(None);
    }
    if debug_size > 28 * 128 || !debug_size.is_multiple_of(28) {
        return Err(invalid());
    }
    let section_bytes = read_at(
        &mut file,
        length,
        pe_offset + 24 + optional_size as u64,
        sections * 40,
    )?;
    let mut debug_offset = None;
    for section in section_bytes.as_chunks::<40>().0 {
        let virtual_address = u32_at(section, 12)?;
        let raw_size = u32_at(section, 16)?;
        let raw_offset = u32_at(section, 20)?;
        if let Some(delta) = debug_rva.checked_sub(virtual_address)
            && delta
                .checked_add(debug_size)
                .is_some_and(|end| end <= raw_size)
        {
            debug_offset = Some(u64::from(raw_offset) + u64::from(delta));
            break;
        }
    }
    let debug = read_at(
        &mut file,
        length,
        debug_offset.ok_or_else(invalid)?,
        debug_size as usize,
    )?;
    for entry in debug.as_chunks::<28>().0 {
        if u32_at(entry, 12)? != 2 {
            continue;
        }
        let size = u32_at(entry, 16)?;
        if size < 24 {
            return Err(invalid());
        }
        // Read only the fixed RSDS prefix; omit the private PDB pathname entirely.
        let record = read_at(&mut file, length, u64::from(u32_at(entry, 24)?), 24)?;
        if &record[..4] == b"RSDS" {
            return Ok(Some(PdbIdentity {
                guid: record[4..20].try_into().map_err(|_| invalid())?,
                age: u32_at(&record, 20)?,
            }));
        }
    }
    Ok(None)
}
