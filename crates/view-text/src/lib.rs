//! Text foundations. Indices are UTF-8 byte offsets validated against an immutable
//! snapshot. Scalar boundaries are not necessarily grapheme/caret boundaries.
//! Backend implementations must supply those policies; no complete control is
//! certified by these interfaces. All layout coordinates are local logical units.
#![forbid(unsafe_code)]

use std::{fmt, sync::Arc};
use view_core::{LogicalPoint, LogicalRect, LogicalSize, Revision, ScaleFactor};

/// A text contract failure. Rejected operations must leave session state intact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextError {
    InvalidIndex,
    InvalidRange,
    StaleRevision,
    RevisionExhausted,
    InvalidMetrics,
    Unsupported,
}

impl fmt::Display for TextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for TextError {}

/// UTF-8 byte offset. Construction alone does not validate it against any text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TextIndex(pub usize);

/// Revision of one text owner's value stream, distinct from UI commit revisions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TextRevision(Revision);

impl TextRevision {
    pub const INITIAL: Self = Self(Revision::INITIAL);
    pub const fn get(self) -> u64 {
        self.0.get()
    }
    pub fn checked_next(self) -> Result<Self, TextError> {
        self.0
            .checked_next()
            .map(Self)
            .map_err(|_| TextError::RevisionExhausted)
    }
}

/// Logical selection; anchor is preserved even when focus precedes it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    pub anchor: TextIndex,
    pub focus: TextIndex,
}

impl Selection {
    pub const fn caret(at: TextIndex) -> Self {
        Self {
            anchor: at,
            focus: at,
        }
    }
    pub fn range(self) -> std::ops::Range<usize> {
        self.anchor.0.min(self.focus.0)..self.anchor.0.max(self.focus.0)
    }
}

/// Immutable text plus its owner-local revision. Keep snapshots alive while any
/// layout references them. Never compare revisions across different text owners.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextSnapshot {
    text: Arc<str>,
    revision: TextRevision,
}

impl TextSnapshot {
    pub fn new(text: impl Into<Arc<str>>, revision: TextRevision) -> Self {
        Self {
            text: text.into(),
            revision,
        }
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub const fn revision(&self) -> TextRevision {
        self.revision
    }
    pub fn validate_index(&self, index: TextIndex) -> Result<(), TextError> {
        if self.text.is_char_boundary(index.0) {
            Ok(())
        } else {
            Err(TextError::InvalidIndex)
        }
    }
    pub fn validate_selection(&self, selection: Selection) -> Result<(), TextError> {
        self.validate_index(selection.anchor)?;
        self.validate_index(selection.focus)
    }
    /// Convert a scalar boundary to platform UTF-16 code units.
    pub fn to_utf16(&self, index: TextIndex) -> Result<usize, TextError> {
        self.validate_index(index)?;
        Ok(self.text[..index.0].encode_utf16().count())
    }
    /// Reject offsets inside a surrogate pair instead of rounding them.
    pub fn from_utf16(&self, offset: usize) -> Result<TextIndex, TextError> {
        let mut units = 0;
        for (byte, ch) in self.text.char_indices() {
            if units == offset {
                return Ok(TextIndex(byte));
            }
            units += ch.len_utf16();
            if units > offset {
                return Err(TextError::InvalidIndex);
            }
        }
        if units == offset {
            Ok(TextIndex(self.text.len()))
        } else {
            Err(TextError::InvalidIndex)
        }
    }
    /// Validate a proposed replacement without changing the snapshot. Endpoints
    /// must be scalar boundaries; editor profiles impose grapheme/length policies.
    pub fn validate_edit(&self, edit: &TextEdit) -> Result<(), TextError> {
        if edit.base != self.revision {
            return Err(TextError::StaleRevision);
        }
        if edit.range.start > edit.range.end {
            return Err(TextError::InvalidRange);
        }
        self.validate_index(TextIndex(edit.range.start))?;
        self.validate_index(TextIndex(edit.range.end))
    }
}

/// A revision-checked replacement in UTF-8 bytes, addressed to a specific session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextEdit {
    pub base: TextRevision,
    pub range: std::ops::Range<usize>,
    pub replacement: String,
}

/// Resolves two visual caret positions at a logical wrap/bidi boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Affinity {
    Upstream,
    Downstream,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Caret {
    pub index: TextIndex,
    pub affinity: Affinity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wrap {
    None,
    Word,
    Grapheme,
}

/// Backend-independent shaping inputs. Font revision is supplied by the owner
/// whenever its font collection/fallback configuration changes. DPI is explicit.
#[derive(Clone, Debug)]
pub struct LayoutRequest {
    pub text: TextSnapshot,
    pub font_family: String,
    pub font_size: f32,
    pub line_height: f32,
    pub max_width: Option<f32>,
    pub wrap: Wrap,
    pub scale: ScaleFactor,
    pub font_revision: Revision,
}

impl LayoutRequest {
    pub fn validate(&self) -> Result<(), TextError> {
        if !self.font_size.is_finite()
            || self.font_size <= 0.0
            || !self.line_height.is_finite()
            || self.line_height <= 0.0
            || self.max_width.is_some_and(|w| !w.is_finite() || w < 0.0)
        {
            return Err(TextError::InvalidMetrics);
        }
        Ok(())
    }
}

/// Immutable shaped result. Implementations retain all CPU resources needed by
/// this layout; GPU atlas allocation is a separate renderer-owned lifetime.
/// Hit/movement outputs must be valid caret boundaries in `text()`.
pub trait TextLayout {
    fn text(&self) -> &TextSnapshot;
    fn size(&self) -> LogicalSize;
    fn hit_test(&self, point: LogicalPoint) -> Result<Caret, TextError>;
    fn caret_bounds(&self, caret: Caret) -> Result<LogicalRect, TextError>;
    fn selection_bounds(&self, selection: Selection) -> Result<Vec<LogicalRect>, TextError>;
}

/// Measurement/shaping has no application effects or session mutations. Cache
/// keys include text identity/content, revision and every layout request field.
pub trait TextBackend {
    type Layout: TextLayout;
    fn shape(&mut self, request: &LayoutRequest) -> Result<Self::Layout, TextError>;
}

/// Composition is transient and separate from committed text/history. Its range
/// refers to committed UTF-8 bytes; selection refers to the preedit string.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Composition {
    pub range: std::ops::Range<usize>,
    pub preedit: String,
    pub selection: Selection,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorSnapshot {
    pub text: TextSnapshot,
    pub selection: Selection,
    pub composition: Option<Composition>,
}

/// Minimal adapter boundary for the forthcoming shared behavior engine. The
/// session owns history/selection/composition; value revisions change on edits,
/// not selection changes. Reject stale edits atomically. Profile-specific commands
/// and external replacement policies are introduced with their behavior tests.
pub trait EditorSession {
    fn snapshot(&self) -> EditorSnapshot;
    fn apply(&mut self, edit: TextEdit) -> Result<EditorSnapshot, TextError>;
    fn select(&mut self, base: TextRevision, selection: Selection) -> Result<(), TextError>;
}
