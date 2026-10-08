use crate::*;
use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, fontdb};
use std::{collections::VecDeque, sync::Arc};
use unicode_segmentation::UnicodeSegmentation;
use view_core::{LogicalPoint, LogicalRect, LogicalSize, Revision};

#[derive(Clone)]
struct Stop {
    caret: Caret,
    rect: LogicalRect,
}
#[derive(Clone)]
struct Cell {
    range: std::ops::Range<usize>,
    rect: LogicalRect,
}

/// CPU layout owns its font references and geometry. Dropping/reloading the backend
/// does not invalidate an existing layout. No glyph atlas handle is exposed.
#[derive(Clone)]
pub struct ShapedText {
    text: TextSnapshot,
    size: LogicalSize,
    stops: Vec<Stop>,
    cells: Vec<Cell>,
    fonts: Vec<Arc<cosmic_text::Font>>,
    missing_glyphs: usize,
}
impl ShapedText {
    pub fn font_count(&self) -> usize {
        self.fonts.len()
    }
    /// Missing glyphs are observable; fallback is never claimed to cover all scripts.
    pub fn missing_glyphs(&self) -> usize {
        self.missing_glyphs
    }
    fn stop(&self, caret: Caret) -> Result<&Stop, TextError> {
        validate_caret(self.text.text(), caret.index)?;
        self.stops
            .iter()
            .find(|s| s.caret == caret)
            .or_else(|| self.stops.iter().find(|s| s.caret.index == caret.index))
            .ok_or(TextError::InvalidIndex)
    }
}
impl TextLayout for ShapedText {
    fn text(&self) -> &TextSnapshot {
        &self.text
    }
    fn size(&self) -> LogicalSize {
        self.size
    }
    fn hit_test(&self, point: LogicalPoint) -> Result<Caret, TextError> {
        if !point.is_valid() {
            return Err(TextError::InvalidMetrics);
        }
        self.stops
            .iter()
            .min_by(|a, b| {
                let distance = |s: &Stop| {
                    let top = s.rect.origin.y;
                    let bottom = top + s.rect.size.height;
                    let dy = if point.y < top {
                        top - point.y
                    } else if point.y >= bottom {
                        point.y - bottom + 0.001
                    } else {
                        0.0
                    };
                    (dy, (s.rect.origin.x - point.x).abs())
                };
                let (ay, ax) = distance(a);
                let (by, bx) = distance(b);
                ay.total_cmp(&by).then(ax.total_cmp(&bx))
            })
            .map(|s| s.caret)
            .ok_or(TextError::InvalidIndex)
    }
    fn caret_bounds(&self, caret: Caret) -> Result<LogicalRect, TextError> {
        Ok(self.stop(caret)?.rect)
    }
    fn selection_bounds(&self, selection: Selection) -> Result<Vec<LogicalRect>, TextError> {
        validate_caret(self.text.text(), selection.anchor)?;
        validate_caret(self.text.text(), selection.focus)?;
        let range = selection.range();
        Ok(self
            .cells
            .iter()
            .filter(|c| c.range.start < range.end && c.range.end > range.start)
            .map(|c| c.rect)
            .collect())
    }
}
impl NavigableLayout for ShapedText {
    fn visual_neighbor(&self, caret: Caret, forward: bool) -> Result<Caret, TextError> {
        let current = self.stop(caret)?;
        let mut ordered: Vec<_> = self.stops.iter().collect();
        ordered.sort_by(|a, b| {
            a.rect
                .origin
                .y
                .total_cmp(&b.rect.origin.y)
                .then(a.rect.origin.x.total_cmp(&b.rect.origin.x))
        });
        let position = ordered
            .iter()
            .position(|s| std::ptr::eq(*s, current))
            .ok_or(TextError::InvalidIndex)?;
        let different = |s: &&&Stop| s.rect.origin != current.rect.origin;
        let next = if forward {
            ordered[position + 1..].iter().find(different)
        } else {
            ordered[..position].iter().rev().find(different)
        };
        Ok(next.map_or(caret, |s| s.caret))
    }
}

/// Qualified cosmic-text backend with an entry-bounded LRU. Layouts returned to
/// consumers are independently owned. Reload fonts to invalidate fallback caches;
/// request DPI/font revision changes also invalidate cached shaping results.
pub struct CosmicBackend {
    fonts: FontSystem,
    cache: VecDeque<(LayoutRequest, ShapedText)>,
    capacity: usize,
    revision: Revision,
    hits: u64,
}
impl CosmicBackend {
    pub fn system(capacity: usize) -> Self {
        Self {
            fonts: FontSystem::new(),
            cache: VecDeque::new(),
            capacity,
            revision: Revision::INITIAL,
            hits: 0,
        }
    }
    /// Use only caller-owned font bytes, with no system-font discovery.
    pub fn from_fonts(
        fonts: impl IntoIterator<Item = Vec<u8>>,
        capacity: usize,
    ) -> Result<Self, TextError> {
        let system = Self::font_system(fonts)?;
        Ok(Self {
            fonts: system,
            cache: VecDeque::new(),
            capacity,
            revision: Revision::INITIAL,
            hits: 0,
        })
    }
    fn font_system(fonts: impl IntoIterator<Item = Vec<u8>>) -> Result<FontSystem, TextError> {
        let mut db = fontdb::Database::new();
        for bytes in fonts {
            db.load_font_data(bytes);
        }
        let family = db
            .faces()
            .next()
            .and_then(|f| f.families.first())
            .map(|f| f.0.clone())
            .ok_or(TextError::NoFonts)?;
        db.set_sans_serif_family(&family);
        db.set_serif_family(&family);
        db.set_monospace_family(&family);
        Ok(FontSystem::new_with_locale_and_db("en-US".into(), db))
    }
    pub fn reload_fonts(
        &mut self,
        fonts: impl IntoIterator<Item = Vec<u8>>,
    ) -> Result<(), TextError> {
        let replacement = Self::font_system(fonts)?;
        let revision = self
            .revision
            .checked_next()
            .map_err(|_| TextError::RevisionExhausted)?;
        self.fonts = replacement;
        self.revision = revision;
        self.cache.clear();
        Ok(())
    }
    pub fn font_revision(&self) -> Revision {
        self.revision
    }
    pub fn cache_hits(&self) -> u64 {
        self.hits
    }
    pub fn cached_layouts(&self) -> usize {
        self.cache.len()
    }
}
impl TextBackend for CosmicBackend {
    type Layout = ShapedText;
    fn shape(&mut self, request: &LayoutRequest) -> Result<ShapedText, TextError> {
        request.validate()?;
        if request.font_revision != self.revision {
            return Err(TextError::StaleRevision);
        }
        if let Some(i) = self.cache.iter().position(|(key, _)| key == request) {
            let entry = self.cache.remove(i).unwrap();
            let result = entry.1.clone();
            self.cache.push_back(entry);
            self.hits = self.hits.saturating_add(1);
            return Ok(result);
        }
        if self.fonts.db().faces().next().is_none() {
            return Err(TextError::NoFonts);
        }
        let mut buffer = Buffer::new(
            &mut self.fonts,
            Metrics::new(request.font_size, request.line_height),
        );
        buffer.set_size(request.max_width, None);
        buffer.set_wrap(match request.wrap {
            Wrap::None => cosmic_text::Wrap::None,
            Wrap::Word => cosmic_text::Wrap::WordOrGlyph,
            Wrap::Grapheme => cosmic_text::Wrap::Glyph,
        });
        let family = match request.font_family.as_str() {
            "sans-serif" => Family::SansSerif,
            "serif" => Family::Serif,
            "monospace" => Family::Monospace,
            name => Family::Name(name),
        };
        buffer.set_text(
            request.text.text(),
            &Attrs::new().family(family),
            Shaping::Advanced,
            None,
        );
        buffer.shape_until_scroll(&mut self.fonts, false);
        let mut starts: Vec<_> = cosmic_text::LineIter::new(request.text.text())
            .map(|(r, _)| r.start)
            .collect();
        starts.push(request.text.text().len());
        let mut result = ShapedText {
            text: request.text.clone(),
            size: LogicalSize::ZERO,
            stops: Vec::new(),
            cells: Vec::new(),
            fonts: Vec::new(),
            missing_glyphs: 0,
        };
        let mut font_ids = Vec::new();
        for run in buffer.layout_runs() {
            let offset = starts[run.line_i];
            result.size.width = result.size.width.max(run.line_w);
            result.size.height = result.size.height.max(run.line_top + run.line_height);
            if run.glyphs.is_empty() {
                result.stops.push(Stop {
                    caret: Caret {
                        index: TextIndex(offset),
                        affinity: Affinity::Downstream,
                    },
                    rect: LogicalRect::new(
                        LogicalPoint::new(0.0, run.line_top),
                        LogicalSize::new(1.0, run.line_height),
                    ),
                });
            }
            for glyph in run.glyphs {
                if glyph.glyph_id == 0 {
                    result.missing_glyphs += 1;
                }
                if !font_ids.contains(&(glyph.font_id, glyph.font_weight)) {
                    if let Some(font) = self.fonts.get_font(glyph.font_id, glyph.font_weight) {
                        result.fonts.push(font);
                    }
                    font_ids.push((glyph.font_id, glyph.font_weight));
                }
                let cluster = &run.text[glyph.start..glyph.end];
                let graphemes: Vec<_> = cluster.grapheme_indices(true).collect();
                let count = graphemes.len().max(1);
                for (n, (start, grapheme)) in graphemes.iter().enumerate() {
                    let begin = offset + glyph.start + start;
                    let end = begin + grapheme.len();
                    // Equal subdivision is the declared ligature caret policy.
                    let width = glyph.w / count as f32;
                    let x = glyph.x
                        + if glyph.level.is_rtl() {
                            (count - n - 1) as f32 * width
                        } else {
                            n as f32 * width
                        };
                    let rect = LogicalRect::new(
                        LogicalPoint::new(x, run.line_top),
                        LogicalSize::new(width, run.line_height),
                    );
                    result.cells.push(Cell {
                        range: begin..end,
                        rect,
                    });
                    for (index, x, affinity) in [
                        (
                            begin,
                            if glyph.level.is_rtl() { x + width } else { x },
                            Affinity::Downstream,
                        ),
                        (
                            end,
                            if glyph.level.is_rtl() { x } else { x + width },
                            Affinity::Upstream,
                        ),
                    ] {
                        if validate_caret(request.text.text(), TextIndex(index)).is_ok() {
                            result.stops.push(Stop {
                                caret: Caret {
                                    index: TextIndex(index),
                                    affinity,
                                },
                                rect: LogicalRect::new(
                                    LogicalPoint::new(x, run.line_top),
                                    LogicalSize::new(1.0, run.line_height),
                                ),
                            });
                        }
                    }
                }
            }
        }
        if result.stops.is_empty() {
            return Err(TextError::InvalidMetrics);
        }
        if self.capacity > 0 {
            if self.cache.len() == self.capacity {
                self.cache.pop_front();
            }
            self.cache.push_back((request.clone(), result.clone()));
        }
        Ok(result)
    }
}
