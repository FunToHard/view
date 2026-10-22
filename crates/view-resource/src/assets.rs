//! CPU assets decoded by application workers; renderer upload remains explicit.
use std::collections::BTreeMap;
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssetError {
    Limit,
    Invalid,
    Unsupported,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}
impl Image {
    pub fn rgba(
        width: u32,
        height: u32,
        rgba: Vec<u8>,
        max_bytes: usize,
    ) -> Result<Self, AssetError> {
        let size = (width as usize)
            .checked_mul(height as usize)
            .and_then(|n| n.checked_mul(4))
            .ok_or(AssetError::Limit)?;
        if size > max_bytes {
            return Err(AssetError::Limit);
        }
        if width == 0 || height == 0 || rgba.len() != size {
            return Err(AssetError::Invalid);
        }
        Ok(Self {
            width,
            height,
            rgba,
        })
    }
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
    pub fn pixels(&self) -> &[u8] {
        &self.rgba
    }
}

struct Entry<T> {
    value: T,
    bytes: usize,
    used: u64,
    expires: Duration,
}
/// App-owned byte/item bounded LRU. Expiry is host-clock based, never a hidden timer.
pub struct Cache<T> {
    entries: BTreeMap<String, Entry<T>>,
    max_bytes: usize,
    max_items: usize,
    bytes: usize,
    tick: u64,
}
impl<T> Cache<T> {
    pub fn new(max_items: usize, max_bytes: usize) -> Self {
        Self {
            entries: BTreeMap::new(),
            max_bytes,
            max_items,
            bytes: 0,
            tick: 0,
        }
    }
    pub fn bytes(&self) -> usize {
        self.bytes
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    fn tick(&mut self) -> u64 {
        // Rebase rather than wrap the LRU clock.
        if self.tick == u64::MAX {
            for entry in self.entries.values_mut() {
                entry.used = 0;
            }
            self.tick = 0;
        }
        self.tick += 1;
        self.tick
    }
    pub fn get(&mut self, key: &str, now: Duration, allow_stale: bool) -> Option<&T> {
        let tick = self.tick();
        let entry = self.entries.get_mut(key)?;
        if !allow_stale && now >= entry.expires {
            return None;
        }
        entry.used = tick;
        Some(&entry.value)
    }
    pub fn insert(
        &mut self,
        key: String,
        value: T,
        bytes: usize,
        expires: Duration,
    ) -> Result<(), AssetError> {
        if self.max_items == 0 || bytes > self.max_bytes || key.len() > 4096 {
            return Err(AssetError::Limit);
        }
        if let Some(old) = self.entries.remove(&key) {
            self.bytes -= old.bytes;
        }
        while self.entries.len() >= self.max_items || bytes > self.max_bytes - self.bytes {
            let oldest = self
                .entries
                .iter()
                .min_by_key(|(_, e)| e.used)
                .map(|(k, _)| k.clone())
                .ok_or(AssetError::Limit)?;
            self.bytes -= self.entries.remove(&oldest).expect("existing entry").bytes;
        }
        let used = self.tick();
        self.bytes += bytes;
        self.entries.insert(
            key,
            Entry {
                value,
                bytes,
                used,
                expires,
            },
        );
        Ok(())
    }
}

/// Deliberately narrow codec: uncompressed 24-bit Windows BMP with 40-byte DIB.
/// Decode on a worker; no filesystem/network access and no implicit uploads.
#[cfg(feature = "image-bmp")]
pub fn decode_bmp(bytes: &[u8], max_bytes: usize) -> Result<Image, AssetError> {
    if bytes.len() < 54 || &bytes[..2] != b"BM" {
        return Err(AssetError::Invalid);
    }
    let u32_at = |i: usize| u32::from_le_bytes(bytes[i..i + 4].try_into().expect("checked header"));
    if u32_at(14) != 40 || bytes[26..28] != [1, 0] || bytes[28..30] != [24, 0] || u32_at(30) != 0 {
        return Err(AssetError::Unsupported);
    }
    let width = u32_at(18) as i32;
    let height = u32_at(22) as i32;
    if width <= 0 || height == 0 || height == i32::MIN {
        return Err(AssetError::Invalid);
    }
    let w = width as usize;
    let h = height.unsigned_abs() as usize;
    let decoded = w
        .checked_mul(h)
        .and_then(|n| n.checked_mul(4))
        .ok_or(AssetError::Limit)?;
    if decoded > max_bytes {
        return Err(AssetError::Limit);
    }
    let stride = w
        .checked_mul(3)
        .and_then(|n| n.checked_add(3))
        .ok_or(AssetError::Limit)?
        & !3;
    let offset = u32_at(10) as usize;
    let end = stride
        .checked_mul(h)
        .and_then(|n| n.checked_add(offset))
        .ok_or(AssetError::Limit)?;
    if offset < 54 || end > bytes.len() {
        return Err(AssetError::Invalid);
    }
    let mut rgba = vec![0; decoded];
    for y in 0..h {
        let source_y = if height > 0 { h - 1 - y } else { y };
        for x in 0..w {
            let src = offset + source_y * stride + x * 3;
            let dst = (y * w + x) * 4;
            rgba[dst..dst + 4].copy_from_slice(&[bytes[src + 2], bytes[src + 1], bytes[src], 255]);
        }
    }
    Image::rgba(width as u32, height.unsigned_abs(), rgba, max_bytes)
}
