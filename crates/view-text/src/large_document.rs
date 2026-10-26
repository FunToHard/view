//! Bounded plain-text document services for editor-like applications.
//!
//! This model deliberately stays separate from the UI tree and from
//! [`PlainEditor`]. Line lookup is indexed; edits currently rebuild the line
//! index, making the memory and update costs predictable without claiming a
//! rope's asymptotic edit performance.
use crate::{TextError, TextRevision};
use std::ops::Range;

/// Limits retained text, line-index entries, search output, and edit history.
#[derive(Clone, Debug)]
pub struct LargeDocumentConfig {
    pub max_bytes: usize,
    pub max_lines: usize,
    /// Maximum number of replacements accepted in one atomic batch.
    pub max_batch_edits: usize,
    pub history_limit: usize,
    pub history_bytes_limit: usize,
    pub max_search_results: usize,
}

impl Default for LargeDocumentConfig {
    fn default() -> Self {
        Self {
            max_bytes: 64 * 1024 * 1024,
            max_lines: 1_000_000,
            max_batch_edits: 4_096,
            history_limit: 100,
            history_bytes_limit: 16 * 1024 * 1024,
            max_search_results: 10_000,
        }
    }
}

/// One replacement in coordinates of the same base revision. Batch ranges
/// must be valid UTF-8 boundaries and may not overlap.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LargeEdit {
    pub range: Range<usize>,
    pub replacement: String,
}

impl LargeEdit {
    pub fn new(range: Range<usize>, replacement: impl Into<String>) -> Self {
        Self {
            range,
            replacement: replacement.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchResults {
    pub revision: TextRevision,
    pub matches: Vec<Range<usize>>,
    pub truncated: bool,
}

#[derive(Clone, Debug)]
struct HistoryEntry {
    forward: Vec<LargeEdit>,
    inverse: Vec<LargeEdit>,
    bytes: usize,
}

/// Application-owned plain text plus bounded line/search/history services.
/// Revisions are owner-local and advance once per successful batch/undo/redo.
#[derive(Clone, Debug)]
pub struct LargeDocument {
    text: String,
    revision: TextRevision,
    line_starts: Vec<usize>,
    config: LargeDocumentConfig,
    undo: Vec<HistoryEntry>,
    redo: Vec<HistoryEntry>,
    history_bytes: usize,
}

impl LargeDocument {
    pub fn new(text: impl Into<String>, config: LargeDocumentConfig) -> Result<Self, TextError> {
        let text = text.into();
        if text.len() > config.max_bytes {
            return Err(TextError::LengthLimit);
        }
        let line_starts = index_lines(&text, config.max_lines)?;
        Ok(Self {
            text,
            revision: TextRevision::INITIAL,
            line_starts,
            config,
            undo: Vec::new(),
            redo: Vec::new(),
            history_bytes: 0,
        })
    }

    pub fn text(&self) -> &str {
        &self.text
    }
    pub const fn revision(&self) -> TextRevision {
        self.revision
    }
    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }

    /// Return a line's byte range without its trailing LF. Lookup is O(log n).
    pub fn line_range(
        &self,
        expected: TextRevision,
        line: usize,
    ) -> Result<Range<usize>, TextError> {
        self.check_revision(expected)?;
        let start = *self.line_starts.get(line).ok_or(TextError::InvalidIndex)?;
        let end = self
            .line_starts
            .get(line + 1)
            .copied()
            .unwrap_or(self.text.len());
        let end = if end > start && self.text.as_bytes()[end - 1] == b'\n' {
            end - 1
        } else {
            end
        };
        Ok(start..end)
    }

    pub fn line(&self, expected: TextRevision, line: usize) -> Result<&str, TextError> {
        let range = self.line_range(expected, line)?;
        Ok(&self.text[range])
    }

    /// Find non-overlapping literal UTF-8 matches. Empty queries are rejected;
    /// result allocation is bounded by `max_search_results`.
    pub fn search(
        &self,
        expected: TextRevision,
        query: &str,
        from: usize,
    ) -> Result<SearchResults, TextError> {
        self.check_revision(expected)?;
        if query.is_empty() {
            return Err(TextError::InvalidRange);
        }
        if from > self.text.len() || !self.text.is_char_boundary(from) {
            return Err(TextError::InvalidIndex);
        }
        let mut matches = Vec::new();
        let mut cursor = from;
        let mut truncated = false;
        while let Some(relative) = self.text[cursor..].find(query) {
            let start = cursor + relative;
            if matches.len() == self.config.max_search_results {
                truncated = true;
                break;
            }
            let end = start + query.len();
            matches.push(start..end);
            cursor = end;
        }
        Ok(SearchResults {
            revision: self.revision,
            matches,
            truncated,
        })
    }

    /// Apply a set of edits against one revision atomically. Ranges use base
    /// text coordinates; a successful non-empty batch advances revision once.
    pub fn apply_batch(
        &mut self,
        expected: TextRevision,
        edits: Vec<LargeEdit>,
    ) -> Result<TextRevision, TextError> {
        self.check_revision(expected)?;
        if edits.is_empty() {
            return Ok(self.revision);
        }
        if edits.len() > self.config.max_batch_edits {
            return Err(TextError::LengthLimit);
        }
        let bytes = history_cost(&edits);
        let recordable = self.config.history_limit > 0 && bytes <= self.config.history_bytes_limit;
        let (next_text, line_starts, inverse) = self.prepare_edits(&edits, recordable)?;
        let next_revision = self.revision.checked_next()?;
        self.text = next_text;
        self.line_starts = line_starts;
        self.revision = next_revision;
        self.history_bytes -= self.redo.iter().map(|entry| entry.bytes).sum::<usize>();
        self.redo.clear();
        if recordable {
            self.undo.push(HistoryEntry {
                forward: edits,
                inverse,
                bytes,
            });
            self.history_bytes += bytes;
            while self.undo.len() > self.config.history_limit
                || self.history_bytes > self.config.history_bytes_limit
            {
                let removed = self.undo.remove(0);
                self.history_bytes -= removed.bytes;
            }
        } else {
            // An unrecordable edit is a history barrier: older ranges may no
            // longer describe this document and must never be replayed.
            self.undo.clear();
            self.redo.clear();
            self.history_bytes = 0;
        }
        Ok(next_revision)
    }

    pub fn undo(&mut self, expected: TextRevision) -> Result<bool, TextError> {
        self.check_revision(expected)?;
        let Some(entry) = self.undo.last().cloned() else {
            return Ok(false);
        };
        let (text, line_starts, _) = self.prepare_edits(&entry.inverse, false)?;
        let revision = self.revision.checked_next()?;
        self.undo.pop();
        self.text = text;
        self.line_starts = line_starts;
        self.revision = revision;
        self.redo.push(entry);
        Ok(true)
    }

    pub fn redo(&mut self, expected: TextRevision) -> Result<bool, TextError> {
        self.check_revision(expected)?;
        let Some(entry) = self.redo.last().cloned() else {
            return Ok(false);
        };
        let (text, line_starts, _) = self.prepare_edits(&entry.forward, false)?;
        let revision = self.revision.checked_next()?;
        self.redo.pop();
        self.text = text;
        self.line_starts = line_starts;
        self.revision = revision;
        self.undo.push(entry.clone());
        while self.undo.len() > self.config.history_limit
            || self.history_bytes > self.config.history_bytes_limit
        {
            let removed = self.undo.remove(0);
            self.history_bytes -= removed.bytes;
        }
        Ok(true)
    }

    fn check_revision(&self, expected: TextRevision) -> Result<(), TextError> {
        if expected == self.revision {
            Ok(())
        } else {
            Err(TextError::StaleRevision)
        }
    }

    fn prepare_edits(
        &self,
        edits: &[LargeEdit],
        record_inverse: bool,
    ) -> Result<(String, Vec<usize>, Vec<LargeEdit>), TextError> {
        if edits.len() > self.config.max_batch_edits {
            return Err(TextError::LengthLimit);
        }
        let mut ordered: Vec<&LargeEdit> = edits.iter().collect();
        ordered.sort_by_key(|edit| edit.range.start);
        let mut previous_end = 0;
        let mut previous_start = None;
        let mut result_len = self.text.len();
        for (i, edit) in ordered.iter().enumerate() {
            let range = &edit.range;
            if range.start > range.end || range.end > self.text.len() {
                return Err(TextError::InvalidRange);
            }
            if !self.text.is_char_boundary(range.start) || !self.text.is_char_boundary(range.end) {
                return Err(TextError::InvalidIndex);
            }
            if i > 0 && (range.start < previous_end || previous_start == Some(range.start)) {
                return Err(TextError::InvalidRange);
            }
            previous_end = range.end;
            previous_start = Some(range.start);
            result_len = result_len
                .checked_sub(range.len())
                .and_then(|n| n.checked_add(edit.replacement.len()))
                .ok_or(TextError::LengthLimit)?;
        }
        if result_len > self.config.max_bytes {
            return Err(TextError::LengthLimit);
        }
        let mut result = self.text.clone();
        for edit in ordered.iter().rev() {
            result.replace_range(edit.range.clone(), &edit.replacement);
        }
        let line_starts = index_lines(&result, self.config.max_lines)?;
        let inverse = if record_inverse {
            let mut inverse = Vec::with_capacity(edits.len());
            let mut delta: i128 = 0;
            let mut group_start = None;
            let mut group_end_base = 0;
            let mut group_end_result = 0;
            let mut group_deleted = String::new();
            for edit in &ordered {
                let new_start = usize::try_from(edit.range.start as i128 + delta)
                    .map_err(|_| TextError::LengthLimit)?;
                let new_end = new_start
                    .checked_add(edit.replacement.len())
                    .ok_or(TextError::LengthLimit)?;
                if group_start.is_some() && edit.range.start != group_end_base {
                    inverse.push(LargeEdit::new(
                        group_start.take().expect("group checked")..group_end_result,
                        std::mem::take(&mut group_deleted),
                    ));
                }
                if group_start.is_none() {
                    group_start = Some(new_start);
                }
                group_end_base = edit.range.end;
                group_end_result = new_end;
                group_deleted.push_str(&self.text[edit.range.clone()]);
                delta += edit.replacement.len() as i128 - edit.range.len() as i128;
            }
            if let Some(start) = group_start {
                inverse.push(LargeEdit::new(start..group_end_result, group_deleted));
            }
            inverse
        } else {
            Vec::new()
        };
        // Inverse ranges are already expressed in ascending resulting coordinates.
        Ok((result, line_starts, inverse))
    }
}

fn index_lines(text: &str, max_lines: usize) -> Result<Vec<usize>, TextError> {
    if max_lines == 0 {
        return Err(TextError::LengthLimit);
    }
    let mut starts = Vec::new();
    starts.push(0);
    for (index, _) in text.match_indices('\n') {
        if starts.len() == max_lines {
            return Err(TextError::LengthLimit);
        }
        starts.push(index + 1);
    }
    Ok(starts)
}

fn history_cost(edits: &[LargeEdit]) -> usize {
    edits.iter().fold(0usize, |sum, edit| {
        sum.saturating_add(edit.range.len())
            .saturating_add(edit.replacement.len())
    })
}
