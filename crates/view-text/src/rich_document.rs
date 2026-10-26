//! Application-owned bounded styled paragraphs, atomic history and conservative rebasing.
use crate::{TextError, TextRevision, grapheme_boundaries};
use std::{collections::VecDeque, mem::size_of, ops::Range};

/// Inline presentation data. Font selection/shaping belongs to the host backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RichStyle {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    /// Logical font size in thousandths, in 1..=1_000_000.
    pub font_size_milli: u32,
    pub color_rgba: [u8; 4],
}

impl Default for RichStyle {
    fn default() -> Self {
        Self {
            bold: false,
            italic: false,
            underline: false,
            font_size_milli: 16_000,
            color_rgba: [0, 0, 0, 255],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RichRun {
    pub text: String,
    pub style: RichStyle,
}

impl RichRun {
    pub fn new(text: impl Into<String>, style: RichStyle) -> Self {
        Self {
            text: text.into(),
            style,
        }
    }
}

/// Paragraph properties used by the measured-line paginator.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ParagraphFormat {
    pub space_before: f64,
    pub space_after: f64,
    pub keep_together: bool,
    pub page_break_before: bool,
}

/// A hard paragraph has no embedded CR/LF. Empty paragraphs have no runs.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RichParagraph {
    pub runs: Vec<RichRun>,
    pub format: ParagraphFormat,
}

impl RichParagraph {
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            runs: vec![RichRun::new(text, RichStyle::default())],
            format: ParagraphFormat::default(),
        }
    }
    pub fn text(&self) -> String {
        self.runs.iter().map(|run| run.text.as_str()).collect()
    }
}

/// All sizes count retained payload/records, not allocator capacity or process RSS.
#[derive(Clone, Debug)]
pub struct RichDocumentConfig {
    pub max_bytes: usize,
    pub max_paragraphs: usize,
    pub max_runs: usize,
    pub max_operations: usize,
    pub history_limit: usize,
    pub history_bytes_limit: usize,
    /// Each entry contains base paragraph text and at most max_operations touches.
    pub rebase_history_limit: usize,
    /// Aggregate retained base text and rebase record accounting budget.
    pub rebase_bytes_limit: usize,
}

impl Default for RichDocumentConfig {
    fn default() -> Self {
        Self {
            max_bytes: 4 * 1024 * 1024,
            max_paragraphs: 100_000,
            max_runs: 100_000,
            max_operations: 1_024,
            history_limit: 100,
            history_bytes_limit: 16 * 1024 * 1024,
            rebase_history_limit: 128,
            rebase_bytes_limit: 16 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RichDocumentError {
    Text(TextError),
    /// Overlap, ambiguous boundary insertions, structure, or undo/redo barrier.
    Conflict,
    /// The complete chain from the supplied base revision is no longer retained.
    HistoryUnavailable,
}

impl From<TextError> for RichDocumentError {
    fn from(value: TextError) -> Self {
        Self::Text(value)
    }
}
impl std::fmt::Display for RichDocumentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for RichDocumentError {}

/// Operations execute sequentially in one revision-checked transaction. Their
/// coordinates refer to the result of earlier operations in that transaction.
#[derive(Clone, Debug, PartialEq)]
pub enum RichOperation {
    Replace {
        paragraph: usize,
        range: Range<usize>,
        runs: Vec<RichRun>,
    },
    Style {
        paragraph: usize,
        range: Range<usize>,
        style: RichStyle,
    },
    Format {
        paragraph: usize,
        format: ParagraphFormat,
    },
    InsertParagraph {
        index: usize,
        paragraph: RichParagraph,
    },
    RemoveParagraph {
        index: usize,
    },
}

/// One remote replacement, addressed to the same document owner's base revision.
/// The host supplies authentication, transport, ordering and duplicate detection.
#[derive(Clone, Debug, PartialEq)]
pub struct RichRemoteEdit {
    pub base_revision: TextRevision,
    pub paragraph: usize,
    pub range: Range<usize>,
    pub runs: Vec<RichRun>,
}

#[derive(Clone, Debug)]
struct HistoryEntry {
    before: Vec<RichParagraph>,
    after: Vec<RichParagraph>,
    bytes: usize,
}

#[derive(Clone, Debug)]
enum Touch {
    Range {
        paragraph: usize,
        range: Range<usize>,
        inserted: usize,
    },
    Format {
        paragraph: usize,
    },
    Barrier,
}
#[derive(Clone, Debug)]
struct Change {
    base: TextRevision,
    source: Vec<String>,
    touches: Vec<Touch>,
    bytes: usize,
}

/// Bounded data services, independent of UI identities, input sessions and layout.
#[derive(Clone, Debug)]
pub struct RichDocument {
    paragraphs: Vec<RichParagraph>,
    revision: TextRevision,
    config: RichDocumentConfig,
    undo: VecDeque<HistoryEntry>,
    redo: Vec<HistoryEntry>,
    history_bytes: usize,
    changes: VecDeque<Change>,
    rebase_bytes: usize,
}

impl RichDocument {
    pub fn new(
        mut paragraphs: Vec<RichParagraph>,
        config: RichDocumentConfig,
    ) -> Result<Self, RichDocumentError> {
        validate_document(&mut paragraphs, &config)?;
        Ok(Self {
            paragraphs,
            revision: TextRevision::INITIAL,
            config,
            undo: VecDeque::new(),
            redo: Vec::new(),
            history_bytes: 0,
            changes: VecDeque::new(),
            rebase_bytes: 0,
        })
    }
    pub fn paragraphs(&self) -> &[RichParagraph] {
        &self.paragraphs
    }
    pub const fn revision(&self) -> TextRevision {
        self.revision
    }
    pub const fn history_bytes(&self) -> usize {
        self.history_bytes
    }
    pub const fn rebase_bytes(&self) -> usize {
        self.rebase_bytes
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Any validation failure leaves text, formatting, revision and history intact.
    /// Empty transactions and value-identical transactions do not advance revision.
    pub fn apply(
        &mut self,
        base: TextRevision,
        operations: Vec<RichOperation>,
    ) -> Result<TextRevision, RichDocumentError> {
        if base != self.revision {
            return Err(TextError::StaleRevision.into());
        }
        if operations.len() > self.config.max_operations {
            return Err(TextError::LengthLimit.into());
        }
        let mut candidate = self.paragraphs.clone();
        let mut touches = Vec::with_capacity(operations.len());
        for operation in operations {
            touches.push(apply_operation(&mut candidate, operation, &self.config)?);
            // Bound every intermediate state as well as the published state.
            validate_document(&mut candidate, &self.config)?;
        }
        if candidate == self.paragraphs {
            return Ok(self.revision);
        }
        let next = self.revision.checked_next()?;
        let bytes = state_bytes(&self.paragraphs)
            .checked_add(state_bytes(&candidate))
            .ok_or(TextError::LengthLimit)?;
        let change = self.prepare_change(touches);
        // A new edit discards redo, including any remote edit. Undo is whole-
        // transaction undo, never selective per-author collaborative undo.
        self.redo.clear();
        self.history_bytes = self.undo.iter().map(|entry| entry.bytes).sum();
        if self.config.history_limit == 0 || bytes > self.config.history_bytes_limit {
            // Do not leave an older undo entry spanning an unrecorded change.
            self.undo.clear();
            self.history_bytes = 0;
        } else {
            while self.undo.len() >= self.config.history_limit
                || self.history_bytes > self.config.history_bytes_limit - bytes
            {
                if let Some(entry) = self.undo.pop_front() {
                    self.history_bytes -= entry.bytes;
                }
            }
            self.undo.push_back(HistoryEntry {
                before: self.paragraphs.clone(),
                after: candidate.clone(),
                bytes,
            });
            self.history_bytes += bytes;
        }
        self.paragraphs = candidate;
        self.record_change(change, next);
        Ok(next)
    }

    pub fn undo(&mut self, base: TextRevision) -> Result<bool, RichDocumentError> {
        if base != self.revision {
            return Err(TextError::StaleRevision.into());
        }
        if self.undo.is_empty() {
            return Ok(false);
        }
        let next = self.revision.checked_next()?;
        let change = self.prepare_change(vec![Touch::Barrier]);
        let entry = self.undo.pop_back().expect("checked nonempty");
        self.paragraphs = entry.before.clone();
        self.redo.push(entry);
        self.record_change(change, next);
        Ok(true)
    }

    pub fn redo(&mut self, base: TextRevision) -> Result<bool, RichDocumentError> {
        if base != self.revision {
            return Err(TextError::StaleRevision.into());
        }
        if self.redo.is_empty() {
            return Ok(false);
        }
        let next = self.revision.checked_next()?;
        let change = self.prepare_change(vec![Touch::Barrier]);
        let entry = self.redo.pop().expect("checked nonempty");
        self.paragraphs = entry.after.clone();
        self.undo.push_back(entry);
        self.record_change(change, next);
        Ok(true)
    }

    /// Rebase one remote replacement over retained disjoint changes, then commit
    /// it using exactly the local validation/history path. This is not a CRDT.
    pub fn apply_remote(
        &mut self,
        edit: RichRemoteEdit,
    ) -> Result<TextRevision, RichDocumentError> {
        if edit.base_revision > self.revision {
            return Err(TextError::StaleRevision.into());
        }
        if edit.range.start > edit.range.end {
            return Err(TextError::InvalidRange.into());
        }
        let mut range = edit.range;
        if edit.base_revision != self.revision {
            let start = self
                .changes
                .iter()
                .position(|change| change.base == edit.base_revision)
                .ok_or(RichDocumentError::HistoryUnavailable)?;
            let source = self.changes[start]
                .source
                .get(edit.paragraph)
                .ok_or(TextError::InvalidIndex)?;
            validate_range(source, &range)?;
            for change in self.changes.iter().skip(start) {
                for touch in &change.touches {
                    match touch {
                        Touch::Barrier => return Err(RichDocumentError::Conflict),
                        Touch::Format { paragraph } if *paragraph == edit.paragraph => {
                            return Err(RichDocumentError::Conflict);
                        }
                        Touch::Range {
                            paragraph,
                            range: local,
                            inserted,
                        } if *paragraph == edit.paragraph => {
                            if ranges_conflict(&range, local) {
                                return Err(RichDocumentError::Conflict);
                            }
                            if local.end <= range.start {
                                range.start = shifted(range.start, local.len(), *inserted)?;
                                range.end = shifted(range.end, local.len(), *inserted)?;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        self.apply(
            self.revision,
            vec![RichOperation::Replace {
                paragraph: edit.paragraph,
                range,
                runs: edit.runs,
            }],
        )
    }

    fn prepare_change(&self, touches: Vec<Touch>) -> Option<Change> {
        let bytes = self.paragraphs.iter().fold(
            size_of::<Change>().saturating_add(touches.len().saturating_mul(size_of::<Touch>())),
            |bytes, paragraph| {
                paragraph
                    .runs
                    .iter()
                    .fold(bytes.saturating_add(size_of::<String>()), |bytes, run| {
                        bytes.saturating_add(run.text.len())
                    })
            },
        );
        if self.config.rebase_history_limit == 0 || bytes > self.config.rebase_bytes_limit {
            return None;
        }
        Some(Change {
            base: self.revision,
            source: self.paragraphs.iter().map(RichParagraph::text).collect(),
            touches,
            bytes,
        })
    }

    fn record_change(&mut self, change: Option<Change>, next: TextRevision) {
        if let Some(change) = change {
            while self.changes.len() >= self.config.rebase_history_limit
                || self.rebase_bytes > self.config.rebase_bytes_limit - change.bytes
            {
                if let Some(old) = self.changes.pop_front() {
                    self.rebase_bytes -= old.bytes;
                }
            }
            self.rebase_bytes += change.bytes;
            self.changes.push_back(change);
        } else {
            // Missing one transition invalidates the complete older chain.
            self.changes.clear();
            self.rebase_bytes = 0;
        }
        self.revision = next;
    }
}

fn shifted(value: usize, removed: usize, inserted: usize) -> Result<usize, RichDocumentError> {
    value
        .checked_sub(removed)
        .and_then(|value| value.checked_add(inserted))
        .ok_or(TextError::InvalidRange.into())
}

fn ranges_conflict(a: &Range<usize>, b: &Range<usize>) -> bool {
    if a.is_empty() || b.is_empty() {
        a.start <= b.end && b.start <= a.end
    } else {
        a.start < b.end && b.start < a.end
    }
}

fn apply_operation(
    paragraphs: &mut Vec<RichParagraph>,
    operation: RichOperation,
    config: &RichDocumentConfig,
) -> Result<Touch, RichDocumentError> {
    match operation {
        RichOperation::Replace {
            paragraph,
            range,
            mut runs,
        } => {
            // Check caller-provided payload before allocating a replacement.
            validate_runs(&mut runs, config)?;
            let target = paragraphs
                .get_mut(paragraph)
                .ok_or(TextError::InvalidIndex)?;
            let text = target.text();
            validate_range(&text, &range)?;
            let inserted = runs.iter().map(|run| run.text.len()).sum();
            let mut result = slice_runs(&target.runs, 0..range.start);
            result.append(&mut runs);
            result.extend(slice_runs(&target.runs, range.end..text.len()));
            target.runs = result;
            Ok(Touch::Range {
                paragraph,
                range,
                inserted,
            })
        }
        RichOperation::Style {
            paragraph,
            range,
            style,
        } => {
            validate_style(style)?;
            let target = paragraphs
                .get_mut(paragraph)
                .ok_or(TextError::InvalidIndex)?;
            let text = target.text();
            validate_range(&text, &range)?;
            let mut result = slice_runs(&target.runs, 0..range.start);
            if !range.is_empty() {
                result.push(RichRun::new(&text[range.clone()], style));
            }
            result.extend(slice_runs(&target.runs, range.end..text.len()));
            target.runs = result;
            let inserted = range.len();
            Ok(Touch::Range {
                paragraph,
                range,
                inserted,
            })
        }
        RichOperation::Format { paragraph, format } => {
            paragraphs
                .get_mut(paragraph)
                .ok_or(TextError::InvalidIndex)?
                .format = format;
            Ok(Touch::Format { paragraph })
        }
        RichOperation::InsertParagraph {
            index,
            mut paragraph,
        } => {
            if index > paragraphs.len() {
                return Err(TextError::InvalidIndex.into());
            }
            if paragraphs.len() >= config.max_paragraphs {
                return Err(TextError::LengthLimit.into());
            }
            validate_paragraph(&mut paragraph, config)?;
            paragraphs.insert(index, paragraph);
            Ok(Touch::Barrier)
        }
        RichOperation::RemoveParagraph { index } => {
            if index >= paragraphs.len() {
                return Err(TextError::InvalidIndex.into());
            }
            if paragraphs.len() == 1 {
                return Err(TextError::Validation.into());
            }
            paragraphs.remove(index);
            Ok(Touch::Barrier)
        }
    }
}

fn validate_range(text: &str, range: &Range<usize>) -> Result<(), TextError> {
    if range.start > range.end || range.end > text.len() {
        return Err(TextError::InvalidRange);
    }
    let boundaries = grapheme_boundaries(text);
    if boundaries.binary_search(&range.start).is_err()
        || boundaries.binary_search(&range.end).is_err()
    {
        return Err(TextError::InvalidIndex);
    }
    Ok(())
}

fn validate_style(style: RichStyle) -> Result<(), TextError> {
    if style.font_size_milli == 0 || style.font_size_milli > 1_000_000 {
        return Err(TextError::InvalidMetrics);
    }
    Ok(())
}

fn validate_runs(
    runs: &mut Vec<RichRun>,
    config: &RichDocumentConfig,
) -> Result<(), RichDocumentError> {
    if runs.len() > config.max_runs {
        return Err(TextError::LengthLimit.into());
    }
    let mut bytes = 0usize;
    for run in runs.iter() {
        bytes = bytes
            .checked_add(run.text.len())
            .ok_or(TextError::LengthLimit)?;
        if bytes > config.max_bytes {
            return Err(TextError::LengthLimit.into());
        }
        if run.text.contains(['\r', '\n']) {
            return Err(TextError::Validation.into());
        }
        validate_style(run.style)?;
    }
    // Canonicalize, then verify that style boundaries do not split an extended
    // grapheme (including clusters formed by concatenating individual runs).
    let mut normalized: Vec<RichRun> = Vec::with_capacity(runs.len());
    for run in runs.drain(..).filter(|run| !run.text.is_empty()) {
        if let Some(last) = normalized.last_mut().filter(|last| last.style == run.style) {
            last.text.push_str(&run.text);
        } else {
            normalized.push(run);
        }
    }
    *runs = normalized;
    let text: String = runs.iter().map(|run| run.text.as_str()).collect();
    let boundaries = grapheme_boundaries(&text);
    let mut offset = 0;
    for run in runs.iter() {
        offset += run.text.len();
        if boundaries.binary_search(&offset).is_err() {
            return Err(TextError::InvalidIndex.into());
        }
    }
    Ok(())
}

fn validate_paragraph(
    paragraph: &mut RichParagraph,
    config: &RichDocumentConfig,
) -> Result<(), RichDocumentError> {
    let format = paragraph.format;
    if !format.space_before.is_finite()
        || !format.space_after.is_finite()
        || format.space_before < 0.0
        || format.space_after < 0.0
    {
        return Err(TextError::InvalidMetrics.into());
    }
    validate_runs(&mut paragraph.runs, config)
}

fn validate_document(
    paragraphs: &mut [RichParagraph],
    config: &RichDocumentConfig,
) -> Result<(), RichDocumentError> {
    if paragraphs.is_empty() || paragraphs.len() > config.max_paragraphs {
        return Err(TextError::LengthLimit.into());
    }
    // Preflight all caller-owned payload/records before per-paragraph text or
    // boundary allocation. Raw lists obey the same cap as canonicalized lists.
    let mut submitted_bytes = 0usize;
    let mut submitted_runs = 0usize;
    for paragraph in paragraphs.iter() {
        submitted_runs = submitted_runs
            .checked_add(paragraph.runs.len())
            .ok_or(TextError::LengthLimit)?;
        for run in &paragraph.runs {
            submitted_bytes = submitted_bytes
                .checked_add(run.text.len())
                .ok_or(TextError::LengthLimit)?;
        }
        if submitted_bytes > config.max_bytes || submitted_runs > config.max_runs {
            return Err(TextError::LengthLimit.into());
        }
    }
    let mut bytes = 0usize;
    let mut runs = 0usize;
    for paragraph in paragraphs {
        validate_paragraph(paragraph, config)?;
        runs = runs
            .checked_add(paragraph.runs.len())
            .ok_or(TextError::LengthLimit)?;
        for run in &paragraph.runs {
            bytes = bytes
                .checked_add(run.text.len())
                .ok_or(TextError::LengthLimit)?;
        }
        if bytes > config.max_bytes || runs > config.max_runs {
            return Err(TextError::LengthLimit.into());
        }
    }
    Ok(())
}

fn slice_runs(runs: &[RichRun], range: Range<usize>) -> Vec<RichRun> {
    let mut offset = 0;
    runs.iter()
        .filter_map(|run| {
            let start = range.start.max(offset);
            let end = range.end.min(offset + run.text.len());
            let result = (start < end)
                .then(|| RichRun::new(&run.text[start - offset..end - offset], run.style));
            offset += run.text.len();
            result
        })
        .collect()
}

fn state_bytes(paragraphs: &[RichParagraph]) -> usize {
    paragraphs.iter().fold(0usize, |size, paragraph| {
        paragraph.runs.iter().fold(
            size.saturating_add(size_of::<RichParagraph>()),
            |size, run| {
                size.saturating_add(size_of::<RichRun>())
                    .saturating_add(run.text.len())
            },
        )
    })
}
