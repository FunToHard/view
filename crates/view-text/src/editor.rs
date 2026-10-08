use crate::*;
use unicode_segmentation::UnicodeSegmentation;
use view_core::{LogicalPoint, LogicalRect, LogicalSize};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineMode {
    SingleLine,
    Multiline,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NewlinePolicy {
    Space,
    Reject,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabPolicy {
    Traverse,
    Insert,
}
#[derive(Clone, Debug)]
pub struct EditorConfig {
    pub mode: LineMode,
    pub newline: NewlinePolicy,
    pub tab: TabPolicy,
    /// Limit in extended grapheme clusters; edits reject atomically, never truncate.
    pub max_graphemes: Option<usize>,
    pub history_limit: usize,
    pub placeholder: String,
}
impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            mode: LineMode::SingleLine,
            newline: NewlinePolicy::Space,
            tab: TabPolicy::Traverse,
            max_graphemes: None,
            history_limit: 100,
            placeholder: String::new(),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditKind {
    Typing,
    Paste,
    Composition,
    Command,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Movement {
    Backward,
    Forward,
    WordBackward,
    WordForward,
    Home,
    End,
    DocumentStart,
    DocumentEnd,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExternalPolicy {
    PreserveSelection,
    ResetSelection,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueUpdate {
    Echo,
    Replaced,
}

/// Host-provided service. Clipboard ownership/native APIs remain outside text.
pub trait Clipboard {
    fn read(&mut self) -> Result<String, TextError>;
    fn write(&mut self, text: &str) -> Result<(), TextError>;
}
#[derive(Default, Debug)]
pub struct MemoryClipboard(pub String);
impl Clipboard for MemoryClipboard {
    fn read(&mut self) -> Result<String, TextError> {
        Ok(self.0.clone())
    }
    fn write(&mut self, text: &str) -> Result<(), TextError> {
        self.0 = text.into();
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct State {
    text: String,
    selection: Selection,
}
#[derive(Clone, Debug)]
struct Group {
    kind: EditKind,
    time: u64,
    after: Selection,
}

/// Field-local plain-text engine. Application-owned document undo should consume
/// the returned revisioned snapshots as one domain transaction; do not replay
/// both this field's undo and document undo for the same command.
type Validator = Box<dyn Fn(&str) -> bool>;

/// Shared plain-text editing session with field-local history and explicit time.
pub struct PlainEditor {
    config: EditorConfig,
    state: State,
    revision: TextRevision,
    composition: Option<Composition>,
    undo: Vec<State>,
    redo: Vec<State>,
    group: Option<Group>,
    validator: Option<Validator>,
    read_only: bool,
    disabled: bool,
    focused: bool,
    blink_epoch: u64,
    preferred_x: Option<f32>,
    scroll: LogicalPoint,
}
impl PlainEditor {
    pub fn new(text: &str, config: EditorConfig) -> Result<Self, TextError> {
        let mut editor = Self {
            config,
            state: State {
                text: String::new(),
                selection: Selection::default(),
            },
            revision: TextRevision::INITIAL,
            composition: None,
            undo: Vec::new(),
            redo: Vec::new(),
            group: None,
            validator: None,
            read_only: false,
            disabled: false,
            focused: false,
            blink_epoch: 0,
            preferred_x: None,
            scroll: LogicalPoint::ZERO,
        };
        let text = editor.normalize(text)?;
        editor.validate_value(&text)?;
        editor.state.text = text;
        Ok(editor)
    }
    pub fn config(&self) -> &EditorConfig {
        &self.config
    }
    pub fn scroll_offset(&self) -> LogicalPoint {
        self.scroll
    }
    /// Apply host wheel/scrollbar intent, clamped to the shaped content viewport.
    pub fn set_scroll_offset(
        &mut self,
        layout: &impl TextLayout,
        offset: LogicalPoint,
        viewport: LogicalSize,
    ) -> Result<(), TextError> {
        self.require_layout(layout)?;
        if !offset.is_valid()
            || !viewport.width.is_finite()
            || !viewport.height.is_finite()
            || viewport.width <= 0.0
            || viewport.height <= 0.0
        {
            return Err(TextError::InvalidMetrics);
        }
        let size = layout.size();
        self.scroll = LogicalPoint::new(
            offset
                .x
                .clamp(0.0, (size.width + 1.0 - viewport.width).max(0.0)),
            if self.config.mode == LineMode::Multiline {
                offset
                    .y
                    .clamp(0.0, (size.height - viewport.height).max(0.0))
            } else {
                0.0
            },
        );
        Ok(())
    }
    pub fn read_only(&self) -> bool {
        self.read_only
    }
    pub fn disabled(&self) -> bool {
        self.disabled
    }
    pub fn set_modes(&mut self, read_only: bool, disabled: bool) {
        self.read_only = read_only;
        self.disabled = disabled;
        if read_only || disabled {
            self.composition = None;
            self.break_group();
        }
        if disabled {
            self.focused = false;
        }
    }
    pub fn set_focused(&mut self, focused: bool, now_ms: u64) {
        self.focused = focused && !self.disabled;
        self.blink_epoch = now_ms;
        self.break_group();
        if !self.focused {
            self.composition = None;
        }
    }
    /// Time comes from the caller (the harness clock in tests); no background timer.
    pub fn caret_visible(&self, now_ms: u64) -> bool {
        self.focused
            && !self.disabled
            && self.composition.as_ref().map_or_else(
                || self.state.selection.range().is_empty(),
                |c| c.selection.range().is_empty(),
            )
            && self.composition.as_ref().is_none_or(|c| c.cursor_visible)
            && (self.composition.is_some()
                || (now_ms.saturating_sub(self.blink_epoch) / 500).is_multiple_of(2))
    }
    pub fn placeholder(&self) -> Option<&str> {
        (self.state.text.is_empty()
            && self
                .composition
                .as_ref()
                .is_none_or(|c| c.preedit.is_empty()))
        .then_some(self.config.placeholder.as_str())
    }
    /// Validation rejects whole transactions. Existing text must satisfy a new rule.
    pub fn set_validator(
        &mut self,
        validator: impl Fn(&str) -> bool + 'static,
    ) -> Result<(), TextError> {
        if !validator(&self.state.text) {
            return Err(TextError::Validation);
        }
        self.validator = Some(Box::new(validator));
        Ok(())
    }
    pub fn break_group(&mut self) {
        self.group = None;
    }
    fn writable(&self) -> Result<(), TextError> {
        if self.disabled {
            Err(TextError::Disabled)
        } else if self.read_only {
            Err(TextError::ReadOnly)
        } else {
            Ok(())
        }
    }
    fn normalize(&self, text: &str) -> Result<String, TextError> {
        let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        if self.config.mode == LineMode::SingleLine && normalized.contains('\n') {
            match self.config.newline {
                NewlinePolicy::Reject => Err(TextError::Validation),
                NewlinePolicy::Space => Ok(normalized.replace('\n', " ")),
            }
        } else {
            Ok(normalized)
        }
    }
    fn validate_value(&self, text: &str) -> Result<(), TextError> {
        if self
            .config
            .max_graphemes
            .is_some_and(|max| text.graphemes(true).count() > max)
        {
            return Err(TextError::LengthLimit);
        }
        if self
            .validator
            .as_ref()
            .is_some_and(|validate| !validate(text))
        {
            return Err(TextError::Validation);
        }
        Ok(())
    }
    fn replace(
        &mut self,
        range: std::ops::Range<usize>,
        input: &str,
        kind: EditKind,
        now_ms: u64,
    ) -> Result<(), TextError> {
        self.writable()?;
        if range.start > range.end {
            return Err(TextError::InvalidRange);
        }
        validate_caret(&self.state.text, TextIndex(range.start))?;
        validate_caret(&self.state.text, TextIndex(range.end))?;
        let input = self.normalize(input)?;
        let mut value = self.state.text.clone();
        value.replace_range(range.clone(), &input);
        self.validate_value(&value)?;
        // Inserting combining/ZWJ text can merge with either neighboring cluster.
        let desired = range.start + input.len();
        let caret = grapheme_boundaries(&value)
            .into_iter()
            .find(|&i| i >= desired)
            .unwrap_or(value.len());
        if value == self.state.text {
            self.state.selection = Selection::caret(TextIndex(caret));
            self.composition = None;
            self.break_group();
            return Ok(());
        }
        let next = self.revision.checked_next()?;
        let grouped = kind == EditKind::Typing
            && range.is_empty()
            && self.group.as_ref().is_some_and(|g| {
                g.kind == kind
                    && now_ms >= g.time
                    && now_ms - g.time <= 1000
                    && g.after == self.state.selection
                    && self.state.selection.range().is_empty()
                    && range.start == self.state.selection.focus.0
            });
        if !grouped {
            self.push_undo(self.state.clone());
        }
        self.redo.clear();
        self.state = State {
            text: value,
            selection: Selection::caret(TextIndex(caret)),
        };
        self.revision = next;
        self.composition = None;
        self.preferred_x = None;
        self.blink_epoch = now_ms;
        self.group = Some(Group {
            kind,
            time: now_ms,
            after: self.state.selection,
        });
        Ok(())
    }
    fn push_undo(&mut self, state: State) {
        if self.config.history_limit == 0 {
            return;
        }
        if self.undo.len() == self.config.history_limit {
            self.undo.remove(0);
        }
        self.undo.push(state);
    }
    pub fn insert(&mut self, input: &str, kind: EditKind, now_ms: u64) -> Result<(), TextError> {
        let range = self
            .composition
            .as_ref()
            .map(|c| c.range.clone())
            .unwrap_or_else(|| self.state.selection.range());
        self.replace(range, input, kind, now_ms)
    }
    pub fn delete(&mut self, backward: bool, word: bool, now_ms: u64) -> Result<(), TextError> {
        self.writable()?;
        let mut range = self.state.selection.range();
        if range.is_empty() {
            let at = range.start;
            if backward {
                range.start = if word {
                    previous_word(&self.state.text, at)
                } else {
                    previous_grapheme(&self.state.text, at)
                };
            } else {
                range.end = if word {
                    next_word(&self.state.text, at)
                } else {
                    next_grapheme(&self.state.text, at)
                };
            }
        }
        self.replace(range, "", EditKind::Command, now_ms)
    }
    pub fn move_caret(
        &mut self,
        movement: Movement,
        extend: bool,
        now_ms: u64,
    ) -> Result<(), TextError> {
        if self.disabled {
            return Err(TextError::Disabled);
        }
        let at = self.state.selection.focus.0;
        let range = self.state.selection.range();
        let text = &self.state.text;
        let target = match movement {
            Movement::Backward if !extend && !range.is_empty() => range.start,
            Movement::Forward if !extend && !range.is_empty() => range.end,
            Movement::Backward => previous_grapheme(text, at),
            Movement::Forward => next_grapheme(text, at),
            Movement::WordBackward => previous_word(text, at),
            Movement::WordForward => next_word(text, at),
            Movement::Home => text[..at].rfind('\n').map_or(0, |i| i + 1),
            Movement::End => text[at..].find('\n').map_or(text.len(), |i| at + i),
            Movement::DocumentStart => 0,
            Movement::DocumentEnd => text.len(),
        };
        self.place(TextIndex(target), extend, now_ms)
    }
    fn place(&mut self, index: TextIndex, extend: bool, now_ms: u64) -> Result<(), TextError> {
        let anchor = if extend {
            self.state.selection.anchor
        } else {
            index
        };
        self.select(
            self.revision,
            Selection {
                anchor,
                focus: index,
            },
        )?;
        self.blink_epoch = now_ms;
        Ok(())
    }
    pub fn select_all(&mut self) -> Result<(), TextError> {
        self.select(
            self.revision,
            Selection {
                anchor: TextIndex(0),
                focus: TextIndex(self.state.text.len()),
            },
        )
    }
    pub fn select_word(&mut self, index: TextIndex) -> Result<(), TextError> {
        let range = word_range(&self.state.text, index)?;
        self.select(
            self.revision,
            Selection {
                anchor: TextIndex(range.start),
                focus: TextIndex(range.end),
            },
        )
    }
    fn require_layout(&self, layout: &impl TextLayout) -> Result<(), TextError> {
        if layout.text() != &self.snapshot().text {
            Err(TextError::StaleRevision)
        } else {
            Ok(())
        }
    }
    pub fn pointer_select(
        &mut self,
        layout: &impl TextLayout,
        point: LogicalPoint,
        extend: bool,
        viewport: LogicalSize,
        now_ms: u64,
    ) -> Result<(), TextError> {
        self.require_layout(layout)?;
        let caret = layout.hit_test(LogicalPoint::new(
            point.x + self.scroll.x,
            point.y + self.scroll.y,
        ))?;
        // Validate geometry before selection mutation so rejected drag/reveal
        // requests cannot leave a partially changed selection.
        if !viewport.width.is_finite()
            || !viewport.height.is_finite()
            || viewport.width <= 0.0
            || viewport.height <= 0.0
        {
            return Err(TextError::InvalidMetrics);
        }
        layout.caret_bounds(caret)?;
        self.place(caret.index, extend, now_ms)?;
        self.reveal(layout, caret, viewport)
    }
    /// Visual horizontal navigation is delegated to shaped layout; logical arrow
    /// commands above remain deterministic Unicode grapheme movement.
    pub fn move_visual(
        &mut self,
        layout: &impl NavigableLayout,
        caret: Caret,
        forward: bool,
        extend: bool,
        now_ms: u64,
    ) -> Result<Caret, TextError> {
        self.require_layout(layout)?;
        if caret.index != self.state.selection.focus {
            return Err(TextError::InvalidIndex);
        }
        let next = layout.visual_neighbor(caret, forward)?;
        self.place(next.index, extend, now_ms)?;
        Ok(next)
    }
    pub fn move_vertical(
        &mut self,
        layout: &impl TextLayout,
        down: bool,
        extend: bool,
        now_ms: u64,
    ) -> Result<(), TextError> {
        self.require_layout(layout)?;
        let rect = layout.caret_bounds(Caret {
            index: self.state.selection.focus,
            affinity: Affinity::Downstream,
        })?;
        let x = self.preferred_x.unwrap_or(rect.origin.x);
        let y = rect.origin.y
            + if down {
                rect.size.height * 1.5
            } else {
                -rect.size.height * 0.5
            };
        let next = layout.hit_test(LogicalPoint::new(x, y))?;
        self.place(next.index, extend, now_ms)?;
        self.preferred_x = Some(x);
        Ok(())
    }
    pub fn reveal(
        &mut self,
        layout: &impl TextLayout,
        caret: Caret,
        viewport: LogicalSize,
    ) -> Result<(), TextError> {
        self.require_layout(layout)?;
        if !viewport.width.is_finite()
            || !viewport.height.is_finite()
            || viewport.width <= 0.0
            || viewport.height <= 0.0
        {
            return Err(TextError::InvalidMetrics);
        }
        let rect = layout.caret_bounds(caret)?;
        self.scroll.x = self
            .scroll
            .x
            .min(rect.origin.x)
            .max(rect.origin.x + rect.size.width - viewport.width)
            .max(0.0);
        self.scroll.y = if self.config.mode == LineMode::Multiline {
            self.scroll
                .y
                .min(rect.origin.y)
                .max(rect.origin.y + rect.size.height - viewport.height)
                .max(0.0)
        } else {
            0.0
        };
        Ok(())
    }
    /// Logical candidate rectangle relative to the control viewport. The platform
    /// adapter adds the control's window origin before reporting it to the IME.
    pub fn candidate_rect(
        &self,
        layout: &impl TextLayout,
        caret: Caret,
    ) -> Result<LogicalRect, TextError> {
        let display = self.display_snapshot()?;
        if layout.text() != &display.text {
            return Err(TextError::StaleRevision);
        }
        // IME offsets are scalar boundaries and may lie inside a display grapheme
        // formed by preedit plus surrounding text. Snap geometry by affinity.
        display.text.validate_index(caret.index)?;
        let boundaries = grapheme_boundaries(display.text.text());
        let index = match caret.affinity {
            Affinity::Downstream => boundaries.iter().copied().find(|&i| i >= caret.index.0),
            Affinity::Upstream => boundaries.iter().copied().rfind(|&i| i <= caret.index.0),
        }
        .ok_or(TextError::InvalidIndex)?;
        let mut rect = layout.caret_bounds(Caret {
            index: TextIndex(index),
            ..caret
        })?;
        rect.origin.x -= self.scroll.x;
        rect.origin.y -= self.scroll.y;
        Ok(rect)
    }
    pub fn undo(&mut self) -> Result<bool, TextError> {
        self.history(false)
    }
    pub fn redo(&mut self) -> Result<bool, TextError> {
        self.history(true)
    }
    fn history(&mut self, redo: bool) -> Result<bool, TextError> {
        self.writable()?;
        let source = if redo { &self.redo } else { &self.undo };
        let Some(state) = source.last() else {
            self.composition = None;
            return Ok(false);
        };
        self.validate_value(&state.text)?;
        let next = self.revision.checked_next()?;
        let restored = if redo {
            self.redo.pop().unwrap()
        } else {
            self.undo.pop().unwrap()
        };
        let old = std::mem::replace(&mut self.state, restored);
        if redo {
            self.push_undo(old);
        } else {
            self.redo.push(old);
        }
        self.revision = next;
        self.composition = None;
        self.break_group();
        self.preferred_x = None;
        Ok(true)
    }
    /// Equal-revision equal-value echoes preserve selection, history and preedit.
    /// Any other stale echo rejects. A real replacement cancels preedit and clears
    /// field history so external document changes are not undone by field undo.
    pub fn external_value(
        &mut self,
        base: TextRevision,
        text: &str,
        policy: ExternalPolicy,
    ) -> Result<ValueUpdate, TextError> {
        if base != self.revision {
            return Err(TextError::StaleRevision);
        }
        let text = self.normalize(text)?;
        self.validate_value(&text)?;
        if text == self.state.text {
            return Ok(ValueUpdate::Echo);
        }
        let next = self.revision.checked_next()?;
        let clamp = |i: TextIndex| {
            TextIndex(
                grapheme_boundaries(&text)
                    .into_iter()
                    .rfind(|&b| b <= i.0)
                    .unwrap_or(0),
            )
        };
        let selection = match policy {
            ExternalPolicy::ResetSelection => Selection::default(),
            ExternalPolicy::PreserveSelection => Selection {
                anchor: clamp(self.state.selection.anchor),
                focus: clamp(self.state.selection.focus),
            },
        };
        self.state = State { text, selection };
        self.revision = next;
        self.composition = None;
        self.undo.clear();
        self.redo.clear();
        self.break_group();
        self.preferred_x = None;
        self.scroll = LogicalPoint::ZERO;
        Ok(ValueUpdate::Replaced)
    }
    pub fn copy(&self, clipboard: &mut impl Clipboard) -> Result<bool, TextError> {
        if self.disabled {
            return Err(TextError::Disabled);
        }
        let range = self.state.selection.range();
        if range.is_empty() {
            return Ok(false);
        }
        clipboard.write(&self.state.text[range])?;
        Ok(true)
    }
    pub fn cut(&mut self, clipboard: &mut impl Clipboard, now_ms: u64) -> Result<bool, TextError> {
        self.writable()?;
        // Validate deletion before changing the external clipboard.
        let range = self.state.selection.range();
        let mut value = self.state.text.clone();
        value.replace_range(range.clone(), "");
        self.validate_value(&value)?;
        self.revision.checked_next()?;
        if !self.copy(clipboard)? {
            return Ok(false);
        }
        self.replace(range, "", EditKind::Command, now_ms)?;
        Ok(true)
    }
    pub fn paste(&mut self, clipboard: &mut impl Clipboard, now_ms: u64) -> Result<(), TextError> {
        self.writable()?;
        let value = clipboard.read()?;
        self.insert(&value, EditKind::Paste, now_ms)
    }
    pub fn preedit(&mut self, text: &str, selection: Selection) -> Result<(), TextError> {
        self.writable()?;
        let snapshot = TextSnapshot::new(text, self.revision);
        snapshot.validate_selection(selection)?;
        let range = self
            .composition
            .as_ref()
            .map(|c| c.range.clone())
            .unwrap_or_else(|| self.state.selection.range());
        self.composition = Some(Composition {
            range,
            preedit: text.into(),
            selection,
            cursor_visible: true,
        });
        self.break_group();
        Ok(())
    }
    pub fn cancel_composition(&mut self) {
        self.composition = None;
        self.break_group();
    }
    pub fn set_composition_cursor_visible(&mut self, visible: bool) {
        if let Some(composition) = &mut self.composition {
            composition.cursor_visible = visible;
        }
    }
    pub fn commit_composition(&mut self, text: &str, now_ms: u64) -> Result<(), TextError> {
        self.insert(text, EditKind::Composition, now_ms)
    }
    /// Preedit display uses the current value revision plus content identity, never
    /// publishes a committed edit. Consumers must compare the complete snapshot.
    pub fn display_snapshot(&self) -> Result<EditorSnapshot, TextError> {
        let mut snapshot = self.snapshot();
        if let Some(c) = &self.composition {
            let mut text = self.state.text.clone();
            text.replace_range(c.range.clone(), &c.preedit);
            snapshot.text = TextSnapshot::new(text, self.revision);
            snapshot.selection = Selection {
                anchor: TextIndex(c.range.start + c.selection.anchor.0),
                focus: TextIndex(c.range.start + c.selection.focus.0),
            };
        }
        Ok(snapshot)
    }
    /// Returns false for focus traversal rather than inserting a Tab in form fields.
    pub fn tab(&mut self, now_ms: u64) -> Result<bool, TextError> {
        if self.disabled {
            return Err(TextError::Disabled);
        }
        if self.config.mode != LineMode::Multiline || self.config.tab == TabPolicy::Traverse {
            return Ok(false);
        }
        self.insert("\t", EditKind::Command, now_ms)?;
        Ok(true)
    }
    pub fn newline(&mut self, now_ms: u64) -> Result<bool, TextError> {
        if self.disabled {
            return Err(TextError::Disabled);
        }
        if self.config.mode == LineMode::SingleLine {
            return Ok(false);
        }
        self.insert("\n", EditKind::Command, now_ms)?;
        Ok(true)
    }
}
impl EditorSession for PlainEditor {
    fn snapshot(&self) -> EditorSnapshot {
        EditorSnapshot {
            text: TextSnapshot::new(self.state.text.clone(), self.revision),
            selection: self.state.selection,
            composition: self.composition.clone(),
        }
    }
    fn apply(&mut self, edit: TextEdit) -> Result<EditorSnapshot, TextError> {
        self.snapshot().text.validate_edit(&edit)?;
        self.replace(edit.range, &edit.replacement, EditKind::Command, 0)?;
        Ok(self.snapshot())
    }
    fn select(&mut self, base: TextRevision, selection: Selection) -> Result<(), TextError> {
        if self.disabled {
            return Err(TextError::Disabled);
        }
        if base != self.revision {
            return Err(TextError::StaleRevision);
        }
        validate_caret(&self.state.text, selection.anchor)?;
        validate_caret(&self.state.text, selection.focus)?;
        self.state.selection = selection;
        self.composition = None;
        self.break_group();
        self.preferred_x = None;
        Ok(())
    }
}

/// Backend extension for visual (as opposed to logical) caret movement.
pub trait NavigableLayout: TextLayout {
    fn visual_neighbor(&self, caret: Caret, forward: bool) -> Result<Caret, TextError>;
}
