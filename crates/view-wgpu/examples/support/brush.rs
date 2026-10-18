//! Original bounded app-owned tiled brush fixture, not a framework document model.
use std::collections::BTreeMap;
use view_core::canvas::CanvasPoint;

pub const TILE: i32 = 16;
/// This mouse-only fixture does not advertise pressure or tilt.
pub const PRESSURE_TILT: (bool, bool) = (false, false);
type Key = (i32, i32);
type Delta = Vec<(Key, Option<Vec<u8>>)>;
pub struct BrushDocument {
    tiles: BTreeMap<Key, Vec<u8>>,
    preview: Vec<CanvasPoint>,
    history: Vec<Delta>,
    max_tiles: usize,
    max_samples: usize,
    max_history: usize,
    active: bool,
}
impl BrushDocument {
    pub fn new(max_tiles: usize, max_samples: usize, max_history: usize) -> Self {
        Self {
            tiles: BTreeMap::new(),
            preview: vec![],
            history: vec![],
            max_tiles,
            max_samples,
            max_history,
            active: false,
        }
    }
    pub fn tiles(&self) -> &BTreeMap<Key, Vec<u8>> {
        &self.tiles
    }
    pub fn preview(&self) -> &[CanvasPoint] {
        &self.preview
    }
    pub fn undo_len(&self) -> usize {
        self.history.len()
    }
    pub fn begin(&mut self, p: CanvasPoint) -> Result<(), &'static str> {
        if self.active {
            return Err("stroke already active");
        }
        self.active = true;
        if let Err(e) = self.append(p) {
            self.active = false;
            return Err(e);
        }
        Ok(())
    }
    pub fn append(&mut self, p: CanvasPoint) -> Result<(), &'static str> {
        if !self.active
            || !p.x.is_finite()
            || !p.y.is_finite()
            || p.x.abs() > 1e8
            || p.y.abs() > 1e8
        {
            return Err("invalid stroke sample");
        }
        let start = self.preview.last().copied().unwrap_or(p);
        let distance = (p.x - start.x).hypot(p.y - start.y);
        let steps = (distance.ceil() as usize).max(1);
        if steps > self.max_samples.saturating_sub(self.preview.len()) {
            return Err("preview budget");
        }
        for i in 1..=steps {
            let t = i as f64 / steps as f64;
            self.preview.push(CanvasPoint {
                x: start.x + (p.x - start.x) * t,
                y: start.y + (p.y - start.y) * t,
            });
        }
        Ok(())
    }
    pub fn cancel(&mut self) {
        self.preview.clear();
        self.active = false;
    }
    pub fn commit(&mut self) -> Result<bool, &'static str> {
        if !self.active {
            return Ok(false);
        }
        if self.max_history == 0 {
            return Err("undo budget disabled");
        }
        let mut changes: BTreeMap<Key, Vec<(usize, [u8; 4])>> = BTreeMap::new();
        for p in &self.preview {
            let x = p.x.floor() as i32;
            let y = p.y.floor() as i32;
            for dy in -2i32..=2 {
                for dx in -2i32..=2 {
                    if dx * dx + dy * dy > 4 {
                        continue;
                    }
                    let px = x + dx;
                    let py = y + dy;
                    let key = (px.div_euclid(TILE), py.div_euclid(TILE));
                    let offset = ((py.rem_euclid(TILE) * TILE + px.rem_euclid(TILE)) * 4) as usize;
                    changes
                        .entry(key)
                        .or_default()
                        .push((offset, [255, 64, 0, 255]));
                }
            }
        }
        let additional = changes
            .keys()
            .filter(|key| !self.tiles.contains_key(key))
            .count();
        if self.tiles.len() + additional > self.max_tiles {
            return Err("document tile budget");
        }
        let mut delta = Vec::new();
        for (key, edits) in changes {
            delta.push((key, self.tiles.get(&key).cloned()));
            let tile = self
                .tiles
                .entry(key)
                .or_insert_with(|| vec![0; (TILE * TILE * 4) as usize]);
            for (offset, color) in edits {
                tile[offset..offset + 4].copy_from_slice(&color);
            }
        }
        if self.history.len() == self.max_history {
            self.history.remove(0);
        }
        self.history.push(delta);
        self.cancel();
        Ok(true)
    }
    pub fn undo(&mut self) -> bool {
        self.cancel();
        let Some(delta) = self.history.pop() else {
            return false;
        };
        for (key, previous) in delta {
            match previous {
                Some(bytes) => {
                    self.tiles.insert(key, bytes);
                }
                None => {
                    self.tiles.remove(&key);
                }
            }
        }
        true
    }
}
