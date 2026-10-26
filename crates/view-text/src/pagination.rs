//! Pure pagination of revision-qualified, caller-measured logical lines.
use crate::{RichDocument, TextRevision, grapheme_boundaries};
use std::ops::Range;

/// Logical source range and positive line-box height after shaping/wrapping.
/// Ranges are in paragraph-local logical byte order, independent of bidi order.
#[derive(Clone, Debug, PartialEq)]
pub struct MeasuredDocumentLine {
    pub range: Range<usize>,
    pub height: f64,
}

/// Measurements are valid only for one immutable document revision. The host
/// must also qualify its font/style/width/DPI inputs before supplying these.
#[derive(Clone, Debug, PartialEq)]
pub struct DocumentMeasurements {
    pub revision: TextRevision,
    pub paragraphs: Vec<Vec<MeasuredDocumentLine>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PageSettings {
    pub height: f64,
    pub margin_top: f64,
    pub margin_bottom: f64,
    pub max_lines: usize,
    pub max_pages: usize,
}

impl Default for PageSettings {
    fn default() -> Self {
        Self {
            height: 1122.0,
            margin_top: 72.0,
            margin_bottom: 72.0,
            max_lines: 100_000,
            max_pages: 10_000,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaginationError {
    StaleRevision,
    InvalidMeasurements,
    InvalidSettings,
    Limit,
    /// A line or keep-together paragraph cannot fit the usable page height.
    UnbreakableContent,
}
impl std::fmt::Display for PaginationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for PaginationError {}

/// A fragment owns no text. It references a contiguous run of measured lines.
#[derive(Clone, Debug, PartialEq)]
pub struct PageFragment {
    pub paragraph: usize,
    pub lines: Range<usize>,
    /// Logical y coordinate from the top of the page, including margin_top.
    pub y: f64,
    pub height: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DocumentPage {
    pub fragments: Vec<PageFragment>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PaginatedDocument {
    pub revision: TextRevision,
    pub pages: Vec<DocumentPage>,
}

impl RichDocument {
    /// Validate complete line coverage before producing bounded page fragments.
    /// Splits occur only between measured lines. Space-before is applied once,
    /// to the first fragment; space-after is discarded beyond the page edge.
    /// This service neither reshapes text nor inserts hyphens or visual carets.
    pub fn paginate(
        &self,
        measurements: &DocumentMeasurements,
        settings: PageSettings,
    ) -> Result<PaginatedDocument, PaginationError> {
        if measurements.revision != self.revision() {
            return Err(PaginationError::StaleRevision);
        }
        if !settings.height.is_finite()
            || !settings.margin_top.is_finite()
            || !settings.margin_bottom.is_finite()
            || settings.margin_top < 0.0
            || settings.margin_bottom < 0.0
            || settings.height <= settings.margin_top + settings.margin_bottom
        {
            return Err(PaginationError::InvalidSettings);
        }
        if settings.max_pages == 0 {
            return Err(PaginationError::Limit);
        }
        let capacity = settings.height - settings.margin_top - settings.margin_bottom;
        if measurements.paragraphs.len() != self.paragraphs().len() {
            return Err(PaginationError::InvalidMeasurements);
        }
        let mut count = 0usize;
        for (paragraph, lines) in self.paragraphs().iter().zip(&measurements.paragraphs) {
            count = count
                .checked_add(lines.len())
                .ok_or(PaginationError::Limit)?;
            if count > settings.max_lines {
                return Err(PaginationError::Limit);
            }
            if lines.is_empty() {
                return Err(PaginationError::InvalidMeasurements);
            }
            let text = paragraph.text();
            let boundaries = grapheme_boundaries(&text);
            let mut end = 0;
            let mut height = paragraph.format.space_before;
            for line in lines {
                if !line.height.is_finite()
                    || line.height <= 0.0
                    || line.range.start != end
                    || line.range.end < line.range.start
                    || line.range.end > text.len()
                    || boundaries.binary_search(&line.range.end).is_err()
                    || (line.range.is_empty() && (!text.is_empty() || lines.len() != 1))
                {
                    return Err(PaginationError::InvalidMeasurements);
                }
                if line.height > capacity {
                    return Err(PaginationError::UnbreakableContent);
                }
                height += line.height;
                end = line.range.end;
            }
            if end != text.len() {
                return Err(PaginationError::InvalidMeasurements);
            }
            if paragraph.format.space_before + lines[0].height > capacity
                || (paragraph.format.keep_together && height > capacity)
            {
                return Err(PaginationError::UnbreakableContent);
            }
        }
        let mut pages = vec![DocumentPage::default()];
        let mut used = 0.0;
        for (paragraph_index, (paragraph, lines)) in self
            .paragraphs()
            .iter()
            .zip(&measurements.paragraphs)
            .enumerate()
        {
            let format = paragraph.format;
            let total = format.space_before + lines.iter().map(|line| line.height).sum::<f64>();
            if !pages.last().expect("initial page").fragments.is_empty()
                && (format.page_break_before || (format.keep_together && used + total > capacity))
            {
                next_page(&mut pages, settings.max_pages)?;
                used = 0.0;
            }
            for (line_index, line) in lines.iter().enumerate() {
                let before = if line_index == 0 {
                    format.space_before
                } else {
                    0.0
                };
                if used + before + line.height > capacity {
                    next_page(&mut pages, settings.max_pages)?;
                    used = 0.0;
                }
                used += before;
                let page = pages.last_mut().expect("initial page");
                if let Some(fragment) = page
                    .fragments
                    .last_mut()
                    .filter(|fragment| fragment.paragraph == paragraph_index)
                {
                    fragment.lines.end += 1;
                    fragment.height += line.height;
                } else {
                    page.fragments.push(PageFragment {
                        paragraph: paragraph_index,
                        lines: line_index..line_index + 1,
                        y: settings.margin_top + used,
                        height: line.height,
                    });
                }
                used += line.height;
            }
            used = (used + format.space_after).min(capacity);
        }
        Ok(PaginatedDocument {
            revision: self.revision(),
            pages,
        })
    }
}

fn next_page(pages: &mut Vec<DocumentPage>, limit: usize) -> Result<(), PaginationError> {
    if pages.len() >= limit {
        return Err(PaginationError::Limit);
    }
    pages.push(DocumentPage::default());
    Ok(())
}
