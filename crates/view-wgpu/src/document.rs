//! Bounded, CPU-side SDR image document operations.
//!
//! This is an optional application helper, not a renderer or a document model
//! owned by the view runtime. Pixels are straight-alpha, 8-bit sRGB RGBA. All
//! operations are bounded and commit as one undoable transaction. Composition
//! and blur explicitly process linear-light premultiplied color before returning
//! to straight-alpha RGBA8 storage.

use std::collections::BTreeMap;

const TILE_SIZE: u32 = 16;
const BYTES_PER_PIXEL: usize = 4;
const EXPORT_MAGIC: &[u8; 8] = b"VWRGBA\0\x01";
const BMP_PIXEL_OFFSET: u32 = 14 + 108;

type TileKey = (u32, u32);
type Tile = Vec<u8>;
type Change = (TileKey, Option<Tile>);

/// Hard limits for a document and its undo/export storage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImageBudget {
    /// Maximum number of image pixels, including transparent pixels.
    pub max_pixels: usize,
    /// Maximum number of allocated tiles after an operation.
    pub max_tiles: usize,
    /// Maximum number of retained undo transactions.
    pub max_history: usize,
    /// Maximum aggregate bytes retained by undo snapshots.
    pub max_history_bytes: usize,
    /// Maximum bytes emitted by the lossless export method.
    pub max_export_bytes: usize,
}

impl Default for ImageBudget {
    fn default() -> Self {
        Self {
            max_pixels: 16 * 1024 * 1024,
            max_tiles: 65_536,
            max_history: 64,
            max_history_bytes: 64 * 1024 * 1024,
            max_export_bytes: 64 * 1024 * 1024,
        }
    }
}

/// A clipped, half-open image-space rectangle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImageRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Borrowed, tightly packed row-major straight-alpha sRGB RGBA8 source.
///
/// Operations validate dimensions and the exact byte length before reading it.
#[derive(Clone, Copy, Debug)]
pub struct RgbaImage<'a> {
    pub width: u32,
    pub height: u32,
    pub pixels: &'a [u8],
}

/// Borrowed row-major coverage, where 0 excludes and 255 fully includes a pixel.
///
/// Coverage is linear and has no color transfer function. Composition masks
/// match the source dimensions; blur masks match the requested rectangle before
/// clipping. Inputs remain owned by the application and are never retained.
#[derive(Clone, Copy, Debug)]
pub struct AlphaMask<'a> {
    pub width: u32,
    pub height: u32,
    pub coverage: &'a [u8],
}

/// Per-operation bounds, checked before allocating mask/composition/blur storage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImageOperationBudget {
    /// Maximum source and mask bytes (including borrowed inputs).
    pub max_source_bytes: usize,
    /// Maximum temporary payload, including staged tiles and undo clones.
    /// Tile accounting includes a conservative 1024-byte metadata allowance.
    pub max_scratch_bytes: usize,
    /// Maximum pixels in the clipped source region, including a blur halo.
    pub max_work_pixels: usize,
}

impl Default for ImageOperationBudget {
    fn default() -> Self {
        Self {
            max_source_bytes: 64 * 1024 * 1024,
            max_scratch_bytes: 128 * 1024 * 1024,
            max_work_pixels: 16 * 1024 * 1024,
        }
    }
}

/// The bounded blur profile cannot be expanded through caller-supplied budgets.
pub const MAX_BLUR_RADIUS: u32 = 32;

/// Small deterministic SDR effects suitable for document transactions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SdrEffect {
    /// Invert encoded sRGB color channels while preserving alpha.
    Invert,
    /// Convert encoded sRGB channels to BT.709 luma while preserving alpha.
    Grayscale,
    /// Multiply encoded sRGB channels by a factor in 0..=4, rounded to u8.
    Exposure { factor_milli: u16 },
}

/// Mouse/tablet sample validated before it enters a document stroke.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PenSample {
    pub x: f64,
    pub y: f64,
    /// Normalized pressure in the inclusive range 0..=1.
    pub pressure: f32,
    /// Signed tablet tilt in degrees, each axis in -90..=90.
    pub tilt_x_degrees: f32,
    pub tilt_y_degrees: f32,
}

/// Input data validation is implemented; no native hardware is certified here.
pub const NATIVE_PRESSURE_SUPPORTED: bool = false;
/// Input data validation is implemented; no native hardware is certified here.
pub const NATIVE_TILT_SUPPORTED: bool = false;

impl PenSample {
    pub fn validate(self) -> Result<Self, ImageError> {
        if !self.x.is_finite()
            || !self.y.is_finite()
            || self.x.abs() > 1e8
            || self.y.abs() > 1e8
            || !self.pressure.is_finite()
            || !(0.0..=1.0).contains(&self.pressure)
            || !self.tilt_x_degrees.is_finite()
            || !self.tilt_y_degrees.is_finite()
            || !(-90.0..=90.0).contains(&self.tilt_x_degrees)
            || !(-90.0..=90.0).contains(&self.tilt_y_degrees)
        {
            return Err(ImageError::InvalidSample);
        }
        Ok(self)
    }
}

/// Errors returned by bounded image document operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImageError {
    InvalidDimensions,
    PixelBudget,
    TileBudget,
    HistoryDisabled,
    HistoryBudget,
    ExportBudget,
    InvalidEffect,
    InvalidSample,
    InvalidSource,
    InvalidMask,
    SourceBudget,
    ScratchBudget,
    WorkBudget,
}

impl std::fmt::Display for ImageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for ImageError {}

/// Sparse tiled RGBA8 image with bounded, transaction-level undo.
pub struct ImageDocument {
    width: u32,
    height: u32,
    budget: ImageBudget,
    tiles: BTreeMap<TileKey, Tile>,
    history: Vec<Vec<Change>>,
    history_bytes: usize,
}

impl ImageDocument {
    pub fn new(width: u32, height: u32, budget: ImageBudget) -> Result<Self, ImageError> {
        if width == 0 || height == 0 {
            return Err(ImageError::InvalidDimensions);
        }
        let pixel_count = (width as usize)
            .checked_mul(height as usize)
            .ok_or(ImageError::PixelBudget)?;
        if pixel_count > budget.max_pixels {
            return Err(ImageError::PixelBudget);
        }
        Ok(Self {
            width,
            height,
            budget,
            tiles: BTreeMap::new(),
            history: Vec::new(),
            history_bytes: 0,
        })
    }

    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    pub fn allocated_tiles(&self) -> usize {
        self.tiles.len()
    }

    pub fn undo_len(&self) -> usize {
        self.history.len()
    }

    /// Read an allocated tile's pixel. An absent tile (transparent black) or an
    /// out-of-bounds coordinate returns `None`; dense sampling can substitute
    /// `[0; 4]` after checking document bounds.
    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let key = (x / TILE_SIZE, y / TILE_SIZE);
        let offset = tile_offset(x % TILE_SIZE, y % TILE_SIZE);
        let tile = self.tiles.get(&key)?;
        tile[offset..offset + BYTES_PER_PIXEL].try_into().ok()
    }

    /// Set one pixel as one undoable edit. Transparent black pixels stay sparse.
    pub fn set_pixel(&mut self, x: u32, y: u32, rgba: [u8; 4]) -> Result<(), ImageError> {
        if x >= self.width || y >= self.height {
            return Err(ImageError::InvalidDimensions);
        }
        if self.budget.max_history == 0 {
            return Err(ImageError::HistoryDisabled);
        }
        let key = (x / TILE_SIZE, y / TILE_SIZE);
        let prior = self.tiles.get(&key).cloned();
        let prior_pixel = prior.as_ref().map(|tile| {
            tile[tile_offset(x % TILE_SIZE, y % TILE_SIZE)
                ..tile_offset(x % TILE_SIZE, y % TILE_SIZE) + BYTES_PER_PIXEL]
                .to_vec()
        });
        if prior_pixel.as_deref() == Some(rgba.as_slice())
            || (prior.is_none() && rgba == [0, 0, 0, 0])
        {
            return Ok(());
        }
        let mut next = prior.clone().unwrap_or_else(empty_tile);
        let offset = tile_offset(x % TILE_SIZE, y % TILE_SIZE);
        next[offset..offset + BYTES_PER_PIXEL].copy_from_slice(&rgba);
        let next = if next.iter().all(|value| *value == 0) {
            None
        } else {
            Some(next)
        };
        self.commit_changes(vec![(key, next)], vec![(key, prior)])?;
        Ok(())
    }

    /// Apply an effect to the clipped rectangle as one atomic undo transaction.
    pub fn apply(&mut self, rect: ImageRect, effect: SdrEffect) -> Result<(), ImageError> {
        self.apply_with_budget(rect, effect, ImageOperationBudget::default())
    }

    /// Apply an encoded-channel point effect using explicit processing limits.
    ///
    /// Unlike composition and blur, these legacy point effects operate directly
    /// on straight, encoded sRGB bytes. No mask or color conversion is implicit.
    pub fn apply_with_budget(
        &mut self,
        rect: ImageRect,
        effect: SdrEffect,
        budget: ImageOperationBudget,
    ) -> Result<(), ImageError> {
        if self.budget.max_history == 0 {
            return Err(ImageError::HistoryDisabled);
        }
        if let SdrEffect::Exposure { factor_milli } = effect
            && factor_milli > 4_000
        {
            return Err(ImageError::InvalidEffect);
        }
        let left = rect.x.min(self.width);
        let top = rect.y.min(self.height);
        let right = rect.x.saturating_add(rect.width).min(self.width);
        let bottom = rect.y.saturating_add(rect.height).min(self.height);
        if left >= right || top >= bottom {
            return Ok(());
        }
        let clipped = ImageRect {
            x: left,
            y: top,
            width: right - left,
            height: bottom - top,
        };
        let pixels = rect_pixels(clipped)?;
        check_source_bytes(
            pixels.checked_mul(4).ok_or(ImageError::SourceBudget)?,
            0,
            budget,
        )?;
        self.preflight(clipped, pixels, 0, budget)?;

        let mut updated = BTreeMap::<TileKey, Tile>::new();
        for y in top..bottom {
            for x in left..right {
                let key = (x / TILE_SIZE, y / TILE_SIZE);
                let tile = updated
                    .entry(key)
                    .or_insert_with(|| self.tiles.get(&key).cloned().unwrap_or_else(empty_tile));
                let offset = tile_offset(x % TILE_SIZE, y % TILE_SIZE);
                apply_effect(&mut tile[offset..offset + BYTES_PER_PIXEL], effect);
            }
        }
        let mut changes = Vec::new();
        let mut prior = Vec::new();
        for (key, tile) in updated {
            let before = self.tiles.get(&key).cloned();
            if before.as_ref() == Some(&tile) || (before.is_none() && tile.iter().all(|v| *v == 0))
            {
                continue;
            }
            let after = if tile.iter().all(|value| *value == 0) {
                None
            } else {
                Some(tile)
            };
            prior.push((key, before));
            changes.push((key, after));
        }
        self.commit_changes(changes, prior)
    }

    /// Source-over a borrowed image at a signed document-space origin.
    ///
    /// Source and destination are decoded to linear sRGB, premultiplied, then
    /// composed using source alpha multiplied by optional mask coverage. The
    /// result is unpremultiplied and encoded back to straight sRGB RGBA8. Pixels
    /// with zero effective source alpha are preserved byte-for-byte. Negative
    /// origins and off-document extents clip without rescaling the source/mask.
    /// A successful nonempty change creates exactly one undo transaction.
    pub fn composite(
        &mut self,
        source: RgbaImage<'_>,
        origin: (i64, i64),
        mask: Option<AlphaMask<'_>>,
        budget: ImageOperationBudget,
    ) -> Result<(), ImageError> {
        let source_bytes = packed_len(source.width, source.height, BYTES_PER_PIXEL)
            .ok_or(ImageError::InvalidSource)?;
        if source.width == 0 || source.height == 0 || source.pixels.len() != source_bytes {
            return Err(ImageError::InvalidSource);
        }
        let mask_bytes = validate_mask(mask, source.width, source.height)?;
        check_source_bytes(source_bytes, mask_bytes, budget)?;
        let Some(rect) = self.placed_rect(source.width, source.height, origin) else {
            return Ok(());
        };
        self.preflight(rect, rect_pixels(rect)?, 0, budget)?;
        let mut updated = BTreeMap::new();
        for y in rect.y..rect.y + rect.height {
            for x in rect.x..rect.x + rect.width {
                // Overlap validation ensures these differences fit the source.
                let sx = (i128::from(x) - i128::from(origin.0)) as u32;
                let sy = (i128::from(y) - i128::from(origin.1)) as u32;
                let index = sy as usize * source.width as usize + sx as usize;
                let coverage = mask.map_or(255, |m| m.coverage[index]);
                let source: [u8; 4] = source.pixels[index * 4..index * 4 + 4]
                    .try_into()
                    .expect("validated RGBA source");
                if coverage == 0 || source[3] == 0 {
                    continue;
                }
                let mut src = linear_premultiplied(source);
                let coverage = f64::from(coverage) / 255.0;
                for value in &mut src {
                    *value *= coverage;
                }
                let dst = linear_premultiplied(self.pixel(x, y).unwrap_or([0; 4]));
                let mut composed = [0.0; 4];
                for channel in 0..4 {
                    composed[channel] = src[channel] + dst[channel] * (1.0 - src[3]);
                }
                self.stage_pixel(&mut updated, x, y, encode_straight(composed));
            }
        }
        self.commit_staged(updated)
    }

    /// Separable box blur with a radius of 0..=32 and optional output mask.
    ///
    /// Filtering uses linear-light premultiplied sRGB, including alpha. It reads
    /// an immutable snapshot of the document, with edge pixels replicated at
    /// document boundaries. The requested rectangle clips writes, not sampling:
    /// pixels just outside it contribute through the halo. Coverage interpolates
    /// original and blurred premultiplied values. A zero radius is a no-op.
    /// Scratch/work bounds include the halo; source bounds include read pixels
    /// and the entire borrowed mask. Undo capacity is conservatively preflighted
    /// for all intersected allocated tiles, even if some pixels do not change.
    pub fn box_blur(
        &mut self,
        rect: ImageRect,
        radius: u32,
        mask: Option<AlphaMask<'_>>,
        budget: ImageOperationBudget,
    ) -> Result<(), ImageError> {
        if radius > MAX_BLUR_RADIUS {
            return Err(ImageError::InvalidEffect);
        }
        let mask_bytes = validate_mask(mask, rect.width, rect.height)?;
        check_source_bytes(0, mask_bytes, budget)?;
        let Some(output) =
            self.placed_rect(rect.width, rect.height, (rect.x.into(), rect.y.into()))
        else {
            return Ok(());
        };
        if radius == 0 {
            return Ok(());
        }
        let top = output.y.saturating_sub(radius);
        let bottom = output
            .y
            .saturating_add(output.height)
            .saturating_add(radius)
            .min(self.height);
        let left = output.x.saturating_sub(radius);
        let right = output
            .x
            .saturating_add(output.width)
            .saturating_add(radius)
            .min(self.width);
        let read_pixels = rect_pixels(ImageRect {
            x: left,
            y: top,
            width: right - left,
            height: bottom - top,
        })?;
        check_source_bytes(
            read_pixels.checked_mul(4).ok_or(ImageError::SourceBudget)?,
            mask_bytes,
            budget,
        )?;
        let intermediate_len = (output.width as usize)
            .checked_mul((bottom - top) as usize)
            .ok_or(ImageError::ScratchBudget)?;
        let intermediate_bytes = intermediate_len
            .checked_mul(std::mem::size_of::<[f64; 4]>())
            .ok_or(ImageError::ScratchBudget)?;
        self.preflight(output, read_pixels, intermediate_bytes, budget)?;

        let divisor = f64::from(radius * 2 + 1);
        let mut horizontal = Vec::with_capacity(intermediate_len);
        for y in top..bottom {
            let mut sum = [0.0; 4];
            for dx in -i64::from(radius)..=i64::from(radius) {
                let x = (i64::from(output.x) + dx).clamp(0, i64::from(self.width) - 1) as u32;
                add_scaled(&mut sum, self.linear_pixel(x, y), 1.0);
            }
            for x in output.x..output.x + output.width {
                horizontal.push(sum.map(|value| value / divisor));
                if x + 1 < output.x + output.width {
                    let remove = (i64::from(x) - i64::from(radius)).max(0) as u32;
                    let add = (i64::from(x) + i64::from(radius) + 1).min(i64::from(self.width) - 1)
                        as u32;
                    add_scaled(&mut sum, self.linear_pixel(remove, y), -1.0);
                    add_scaled(&mut sum, self.linear_pixel(add, y), 1.0);
                }
            }
        }
        let mut updated = BTreeMap::new();
        for x in output.x..output.x + output.width {
            let column = (x - output.x) as usize;
            let row_value = |y: i64| {
                let y = y.clamp(0, i64::from(self.height) - 1) as u32;
                horizontal[(y - top) as usize * output.width as usize + column]
            };
            let mut sum = [0.0; 4];
            for dy in -i64::from(radius)..=i64::from(radius) {
                add_scaled(&mut sum, row_value(i64::from(output.y) + dy), 1.0);
            }
            for y in output.y..output.y + output.height {
                let coverage = mask.map_or(255, |m| {
                    m.coverage[(y - rect.y) as usize * rect.width as usize + (x - rect.x) as usize]
                });
                if coverage != 0 {
                    let factor = f64::from(coverage) / 255.0;
                    let mut result = self.linear_pixel(x, y);
                    for channel in 0..4 {
                        result[channel] =
                            result[channel] * (1.0 - factor) + sum[channel] / divisor * factor;
                    }
                    self.stage_pixel(&mut updated, x, y, encode_straight(result));
                }
                if y + 1 < output.y + output.height {
                    add_scaled(&mut sum, row_value(i64::from(y) - i64::from(radius)), -1.0);
                    add_scaled(
                        &mut sum,
                        row_value(i64::from(y) + i64::from(radius) + 1),
                        1.0,
                    );
                }
            }
        }
        self.commit_staged(updated)
    }

    fn linear_pixel(&self, x: u32, y: u32) -> [f64; 4] {
        linear_premultiplied(self.pixel(x, y).unwrap_or([0; 4]))
    }

    fn placed_rect(&self, width: u32, height: u32, origin: (i64, i64)) -> Option<ImageRect> {
        // i128 allows arbitrary signed origins without wrapping their extents.
        let left = i128::from(origin.0).max(0).min(self.width.into());
        let top = i128::from(origin.1).max(0).min(self.height.into());
        let right = (i128::from(origin.0) + i128::from(width))
            .max(0)
            .min(self.width.into());
        let bottom = (i128::from(origin.1) + i128::from(height))
            .max(0)
            .min(self.height.into());
        (left < right && top < bottom).then_some(ImageRect {
            x: left as u32,
            y: top as u32,
            width: (right - left) as u32,
            height: (bottom - top) as u32,
        })
    }

    fn preflight(
        &self,
        rect: ImageRect,
        work_pixels: usize,
        scratch_bytes: usize,
        budget: ImageOperationBudget,
    ) -> Result<(), ImageError> {
        if work_pixels > budget.max_work_pixels {
            return Err(ImageError::WorkBudget);
        }
        let columns = (rect.x + rect.width - 1) / TILE_SIZE - rect.x / TILE_SIZE + 1;
        let rows = (rect.y + rect.height - 1) / TILE_SIZE - rect.y / TILE_SIZE + 1;
        let tile_count = (columns as usize)
            .checked_mul(rows as usize)
            .ok_or(ImageError::ScratchBudget)?;
        // One staged tile and one undo clone per tile, plus metadata allowance.
        let tile_bytes = tile_count
            .checked_mul(2 * empty_tile_bytes() + 1024)
            .ok_or(ImageError::ScratchBudget)?;
        if scratch_bytes
            .checked_add(tile_bytes)
            .ok_or(ImageError::ScratchBudget)?
            > budget.max_scratch_bytes
        {
            return Err(ImageError::ScratchBudget);
        }
        if self.budget.max_history == 0 {
            return Err(ImageError::HistoryDisabled);
        }
        if self.history.len() >= self.budget.max_history {
            return Err(ImageError::HistoryBudget);
        }
        let mut history_bytes = 0usize;
        for ty in rect.y / TILE_SIZE..=(rect.y + rect.height - 1) / TILE_SIZE {
            for tx in rect.x / TILE_SIZE..=(rect.x + rect.width - 1) / TILE_SIZE {
                if self.tiles.contains_key(&(tx, ty)) {
                    history_bytes = history_bytes
                        .checked_add(empty_tile_bytes())
                        .ok_or(ImageError::HistoryBudget)?;
                }
            }
        }
        if history_bytes
            > self
                .budget
                .max_history_bytes
                .saturating_sub(self.history_bytes)
        {
            return Err(ImageError::HistoryBudget);
        }
        Ok(())
    }

    fn stage_pixel(&self, updated: &mut BTreeMap<TileKey, Tile>, x: u32, y: u32, pixel: [u8; 4]) {
        if self.pixel(x, y).unwrap_or([0; 4]) == pixel {
            return;
        }
        let key = (x / TILE_SIZE, y / TILE_SIZE);
        let tile = updated
            .entry(key)
            .or_insert_with(|| self.tiles.get(&key).cloned().unwrap_or_else(empty_tile));
        let offset = tile_offset(x % TILE_SIZE, y % TILE_SIZE);
        tile[offset..offset + 4].copy_from_slice(&pixel);
    }

    fn commit_staged(&mut self, updated: BTreeMap<TileKey, Tile>) -> Result<(), ImageError> {
        let mut changes = Vec::new();
        let mut history = Vec::new();
        for (key, tile) in updated {
            history.push((key, self.tiles.get(&key).cloned()));
            changes.push((key, (!tile.iter().all(|value| *value == 0)).then_some(tile)));
        }
        self.commit_changes(changes, history)
    }

    /// Restore the most recent committed pixel/effect operation.
    pub fn undo(&mut self) -> bool {
        let Some(changes) = self.history.pop() else {
            return false;
        };
        self.history_bytes = self.history_bytes.saturating_sub(change_bytes(&changes));
        for (key, prior) in changes {
            match prior {
                Some(tile) => {
                    self.tiles.insert(key, tile);
                }
                None => {
                    self.tiles.remove(&key);
                }
            }
        }
        true
    }

    /// Export the complete image in the deterministic `VWRGBA\0\x01` format.
    ///
    /// The format is an 8-byte magic/version, little-endian u32 width and height,
    /// followed by row-major straight-alpha sRGB RGBA8 pixels. It is lossless and
    /// intentionally codec-free; the caller owns file I/O and format selection.
    pub fn encode_rgba8(&self) -> Result<Vec<u8>, ImageError> {
        let pixel_bytes = (self.width as usize)
            .checked_mul(self.height as usize)
            .and_then(|n| n.checked_mul(BYTES_PER_PIXEL))
            .ok_or(ImageError::ExportBudget)?;
        let capacity = 16usize
            .checked_add(pixel_bytes)
            .ok_or(ImageError::ExportBudget)?;
        if capacity > self.budget.max_export_bytes {
            return Err(ImageError::ExportBudget);
        }
        let mut output = Vec::with_capacity(capacity);
        output.extend_from_slice(EXPORT_MAGIC);
        output.extend_from_slice(&self.width.to_le_bytes());
        output.extend_from_slice(&self.height.to_le_bytes());
        for y in 0..self.height {
            for x in 0..self.width {
                match self.pixel(x, y) {
                    Some(pixel) => output.extend_from_slice(&pixel),
                    None => output.extend_from_slice(&[0, 0, 0, 0]),
                }
            }
        }
        Ok(output)
    }

    /// Encode interoperable, lossless, top-down 32-bit BMP V4 with alpha and sRGB.
    pub fn encode_bmp(&self) -> Result<Vec<u8>, ImageError> {
        if self.width > i32::MAX as u32 || self.height > i32::MAX as u32 {
            return Err(ImageError::ExportBudget);
        }
        let pixel_bytes = (self.width as usize)
            .checked_mul(self.height as usize)
            .and_then(|n| n.checked_mul(BYTES_PER_PIXEL))
            .ok_or(ImageError::ExportBudget)?;
        let file_bytes = (BMP_PIXEL_OFFSET as usize)
            .checked_add(pixel_bytes)
            .ok_or(ImageError::ExportBudget)?;
        if file_bytes > self.budget.max_export_bytes || file_bytes > u32::MAX as usize {
            return Err(ImageError::ExportBudget);
        }
        let mut output = Vec::with_capacity(file_bytes);
        output.extend_from_slice(b"BM");
        push_u32(&mut output, file_bytes as u32);
        push_u16(&mut output, 0);
        push_u16(&mut output, 0);
        push_u32(&mut output, BMP_PIXEL_OFFSET);
        push_u32(&mut output, 108); // BITMAPV4HEADER
        push_i32(&mut output, self.width as i32);
        push_i32(&mut output, -(self.height as i32)); // top-down rows
        push_u16(&mut output, 1);
        push_u16(&mut output, 32);
        push_u32(&mut output, 3); // BI_BITFIELDS
        push_u32(&mut output, pixel_bytes as u32);
        push_i32(&mut output, 0);
        push_i32(&mut output, 0);
        push_u32(&mut output, 0);
        push_u32(&mut output, 0);
        push_u32(&mut output, 0x00ff_0000); // red
        push_u32(&mut output, 0x0000_ff00); // green
        push_u32(&mut output, 0x0000_00ff); // blue
        push_u32(&mut output, 0xff00_0000); // alpha
        push_u32(&mut output, 0x7352_4742); // LCS_sRGB
        output.resize(14 + 108, 0); // endpoint coordinates and rendering intent
        for y in 0..self.height {
            for x in 0..self.width {
                let [r, g, b, a] = self.pixel(x, y).unwrap_or([0, 0, 0, 0]);
                output.extend_from_slice(&[b, g, r, a]);
            }
        }
        debug_assert_eq!(output.len(), file_bytes);
        Ok(output)
    }

    fn commit_changes(
        &mut self,
        changes: Vec<(TileKey, Option<Tile>)>,
        history: Vec<Change>,
    ) -> Result<(), ImageError> {
        if changes.is_empty() {
            return Ok(());
        }
        let bytes = change_bytes(&history);
        if self.history.len() >= self.budget.max_history
            || bytes
                > self
                    .budget
                    .max_history_bytes
                    .saturating_sub(self.history_bytes)
        {
            return Err(ImageError::HistoryBudget);
        }
        let added = changes
            .iter()
            .filter(|(key, after)| !self.tiles.contains_key(key) && after.is_some())
            .count();
        let removed = changes
            .iter()
            .filter(|(key, after)| self.tiles.contains_key(key) && after.is_none())
            .count();
        if self
            .tiles
            .len()
            .saturating_add(added)
            .saturating_sub(removed)
            > self.budget.max_tiles
        {
            return Err(ImageError::TileBudget);
        }
        for (key, after) in changes {
            match after {
                Some(tile) => {
                    self.tiles.insert(key, tile);
                }
                None => {
                    self.tiles.remove(&key);
                }
            }
        }
        self.history_bytes += bytes;
        self.history.push(history);
        Ok(())
    }
}

fn push_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_i32(bytes: &mut Vec<u8>, value: i32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn empty_tile() -> Tile {
    vec![0; empty_tile_bytes()]
}

const fn empty_tile_bytes() -> usize {
    TILE_SIZE as usize * TILE_SIZE as usize * BYTES_PER_PIXEL
}

fn packed_len(width: u32, height: u32, channels: usize) -> Option<usize> {
    (width as usize)
        .checked_mul(height as usize)?
        .checked_mul(channels)
}

fn rect_pixels(rect: ImageRect) -> Result<usize, ImageError> {
    packed_len(rect.width, rect.height, 1).ok_or(ImageError::WorkBudget)
}

fn validate_mask(
    mask: Option<AlphaMask<'_>>,
    width: u32,
    height: u32,
) -> Result<usize, ImageError> {
    let Some(mask) = mask else { return Ok(0) };
    let size = packed_len(width, height, 1).ok_or(ImageError::InvalidMask)?;
    if mask.width != width || mask.height != height || mask.coverage.len() != size {
        return Err(ImageError::InvalidMask);
    }
    Ok(size)
}

fn check_source_bytes(
    source: usize,
    mask: usize,
    budget: ImageOperationBudget,
) -> Result<(), ImageError> {
    if source.checked_add(mask).ok_or(ImageError::SourceBudget)? > budget.max_source_bytes {
        return Err(ImageError::SourceBudget);
    }
    Ok(())
}

fn linear_premultiplied(pixel: [u8; 4]) -> [f64; 4] {
    let alpha = f64::from(pixel[3]) / 255.0;
    let mut result = [0.0; 4];
    for channel in 0..3 {
        let encoded = f64::from(pixel[channel]) / 255.0;
        let linear = if encoded <= 0.04045 {
            encoded / 12.92
        } else {
            ((encoded + 0.055) / 1.055).powf(2.4)
        };
        result[channel] = linear * alpha;
    }
    result[3] = alpha;
    result
}

fn encode_straight(pixel: [f64; 4]) -> [u8; 4] {
    let alpha = pixel[3].clamp(0.0, 1.0);
    let alpha_byte = (alpha * 255.0).round() as u8;
    if alpha_byte == 0 {
        return [0; 4];
    }
    let mut result = [0; 4];
    for channel in 0..3 {
        let linear = (pixel[channel] / alpha).clamp(0.0, 1.0);
        let encoded = if linear <= 0.0031308 {
            linear * 12.92
        } else {
            1.055 * linear.powf(1.0 / 2.4) - 0.055
        };
        result[channel] = (encoded * 255.0).round() as u8;
    }
    result[3] = alpha_byte;
    result
}

fn add_scaled(sum: &mut [f64; 4], pixel: [f64; 4], scale: f64) {
    for channel in 0..4 {
        sum[channel] += pixel[channel] * scale;
    }
}

fn tile_offset(x: u32, y: u32) -> usize {
    ((y * TILE_SIZE + x) as usize) * BYTES_PER_PIXEL
}

fn change_bytes(changes: &[Change]) -> usize {
    changes
        .iter()
        .map(|(_, tile)| tile.as_ref().map_or(0, Vec::len))
        .sum()
}

fn apply_effect(pixel: &mut [u8], effect: SdrEffect) {
    match effect {
        SdrEffect::Invert => {
            for channel in &mut pixel[..3] {
                *channel = 255 - *channel;
            }
        }
        SdrEffect::Grayscale => {
            let [r, g, b] = [pixel[0] as u32, pixel[1] as u32, pixel[2] as u32];
            let luma = ((2126 * r + 7152 * g + 722 * b + 5_000) / 10_000) as u8;
            pixel[..3].fill(luma);
        }
        SdrEffect::Exposure { factor_milli } => {
            for channel in &mut pixel[..3] {
                *channel = ((*channel as u32 * factor_milli as u32 + 500) / 1_000).min(255) as u8;
            }
        }
    }
}
