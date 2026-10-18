use crate::*;
use std::sync::Arc;
use view_core::{
    Key, KeyPhase, KeyboardEvent, LogicalPoint, LogicalRect, LogicalSize, NamedKey, NodeId,
    PointerButton, PointerEvent, PointerPhase, Role, ScaleFactor, SemanticAction, SemanticNode,
};
use view_text::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextCommand {
    Copy,
    Cut,
    Paste,
    SelectAll,
    Undo,
    Redo,
    Delete,
    Submit,
    Cancel,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextResponse {
    Ignored,
    Handled,
    Submit,
    Traverse(bool),
    ContextMenu,
}
/// A persistent control session. Keep this value in a core runtime node; do not
/// reconstruct it during paint. All input effects occur through explicit methods.
pub struct TextField {
    pub label: String,
    editor: PlainEditor,
    focused: bool,
    dragging: bool,
    layout: Option<Arc<ShapedText>>,
    pub invalid: bool,
}
pub struct TextPresentation {
    pub layout: Arc<ShapedText>,
    pub origin: LogicalPoint,
    pub clip: LogicalRect,
    pub selection: Option<Selection>,
    pub caret: Option<Caret>,
    pub preedit: Option<Selection>,
    pub candidate: LogicalRect,
    pub placeholder: bool,
}
impl TextField {
    pub fn new(
        label: impl Into<String>,
        value: &str,
        config: EditorConfig,
    ) -> Result<Self, TextError> {
        Ok(Self {
            label: label.into(),
            editor: PlainEditor::new(value, config)?,
            focused: false,
            dragging: false,
            layout: None,
            invalid: false,
        })
    }
    pub fn editor(&self) -> &PlainEditor {
        &self.editor
    }
    pub fn editor_mut(&mut self) -> &mut PlainEditor {
        &mut self.editor
    }
    pub fn snapshot(&self) -> EditorSnapshot {
        self.editor.snapshot()
    }
    pub fn focused(&self) -> bool {
        self.focused
    }
    pub fn set_focused(&mut self, focused: bool, now: u64) {
        self.focused = focused && !self.editor.disabled();
        self.editor.set_focused(self.focused, now);
        if !self.focused {
            self.dragging = false;
        }
    }
    pub fn set_modes(&mut self, read_only: bool, disabled: bool) {
        self.editor.set_modes(read_only, disabled);
        if disabled {
            self.focused = false;
            self.dragging = false;
        }
    }
    pub fn unmount(&mut self, now: u64) {
        self.set_focused(false, now);
        self.layout = None;
    }
    pub fn external(
        &mut self,
        base: TextRevision,
        value: &str,
        policy: ExternalPolicy,
    ) -> Result<ValueUpdate, TextError> {
        self.editor.external_value(base, value, policy)
    }
    pub fn insert_text(&mut self, text: &str, now: u64) -> Result<(), TextError> {
        self.editor.insert(text, EditKind::Typing, now)
    }
    pub fn preedit(&mut self, text: &str, cursor: Option<(usize, usize)>) -> Result<(), TextError> {
        if text.is_empty() {
            self.editor.cancel_composition();
            return Ok(());
        }
        let (a, f) = cursor.unwrap_or((text.len(), text.len()));
        self.editor.preedit(
            text,
            Selection {
                anchor: TextIndex(a),
                focus: TextIndex(f),
            },
        )?;
        self.editor.set_composition_cursor_visible(cursor.is_some());
        Ok(())
    }
    pub fn commit_ime(&mut self, text: &str, now: u64) -> Result<(), TextError> {
        self.editor.commit_composition(text, now)
    }
    pub fn command(
        &mut self,
        command: TextCommand,
        clipboard: &mut impl Clipboard,
        now: u64,
    ) -> Result<TextResponse, TextError> {
        if self.editor.disabled() {
            return Err(TextError::Disabled);
        }
        match command {
            TextCommand::Copy => {
                self.editor.copy(clipboard)?;
            }
            TextCommand::Cut => {
                self.editor.cut(clipboard, now)?;
            }
            TextCommand::Paste => self.editor.paste(clipboard, now)?,
            TextCommand::SelectAll => self.editor.select_all()?,
            TextCommand::Undo => {
                self.editor.undo()?;
            }
            TextCommand::Redo => {
                self.editor.redo()?;
            }
            TextCommand::Delete => self.editor.delete(false, false, now)?,
            TextCommand::Cancel => {
                self.editor.cancel_composition();
                self.dragging = false;
            }
            TextCommand::Submit => return Ok(TextResponse::Submit),
        }
        Ok(TextResponse::Handled)
    }
    pub fn context_commands(&self) -> Vec<TextCommand> {
        if self.editor.disabled() {
            return vec![];
        }
        let mut commands = vec![TextCommand::Copy, TextCommand::SelectAll];
        if !self.editor.read_only() {
            commands.extend([
                TextCommand::Cut,
                TextCommand::Paste,
                TextCommand::Undo,
                TextCommand::Redo,
                TextCommand::Delete,
            ]);
        }
        commands
    }
    pub fn key(
        &mut self,
        event: &KeyboardEvent,
        clipboard: &mut impl Clipboard,
    ) -> Result<TextResponse, TextError> {
        if !self.focused || self.editor.disabled() || event.phase == KeyPhase::Up {
            return Ok(TextResponse::Ignored);
        }
        let now = event.timestamp;
        let extend = event.modifiers.shift;
        let ctrl = event.modifiers.ctrl || event.modifiers.meta;
        if self.snapshot().composition.is_some() {
            if event.key == Key::Named(NamedKey::Escape) {
                self.editor.cancel_composition();
                return Ok(TextResponse::Handled);
            }
            return Ok(TextResponse::Ignored);
        }
        if ctrl && let Key::Character(c) = &event.key {
            let cmd = match c.to_lowercase().as_str() {
                "a" => Some(TextCommand::SelectAll),
                "c" => Some(TextCommand::Copy),
                "x" => Some(TextCommand::Cut),
                "v" => Some(TextCommand::Paste),
                "z" => Some(if extend {
                    TextCommand::Redo
                } else {
                    TextCommand::Undo
                }),
                "y" => Some(TextCommand::Redo),
                _ => None,
            };
            if let Some(command) = cmd {
                return self.command(command, clipboard, now);
            }
        }
        let movement = match event.key {
            Key::Named(NamedKey::ArrowLeft) => Some(if ctrl {
                Movement::WordBackward
            } else {
                Movement::Backward
            }),
            Key::Named(NamedKey::ArrowRight) => Some(if ctrl {
                Movement::WordForward
            } else {
                Movement::Forward
            }),
            Key::Named(NamedKey::Home) => Some(if ctrl {
                Movement::DocumentStart
            } else {
                Movement::Home
            }),
            Key::Named(NamedKey::End) => Some(if ctrl {
                Movement::DocumentEnd
            } else {
                Movement::End
            }),
            _ => None,
        };
        if let Some(movement) = movement {
            if !ctrl
                && matches!(
                    event.key,
                    Key::Named(NamedKey::ArrowLeft | NamedKey::ArrowRight)
                )
            {
                if let Some(layout) = self
                    .layout
                    .as_ref()
                    .filter(|layout| layout.text() == &self.snapshot().text)
                {
                    self.editor.move_visual(
                        layout.as_ref(),
                        Caret {
                            index: self.snapshot().selection.focus,
                            affinity: Affinity::Downstream,
                        },
                        event.key == Key::Named(NamedKey::ArrowRight),
                        extend,
                        now,
                    )?;
                } else {
                    self.editor.move_caret(movement, extend, now)?;
                }
            } else {
                self.editor.move_caret(movement, extend, now)?;
            }
            return Ok(TextResponse::Handled);
        }
        match event.key {
            Key::Named(NamedKey::Backspace) => self.editor.delete(true, ctrl, now)?,
            Key::Named(NamedKey::Delete) => self.editor.delete(false, ctrl, now)?,
            Key::Named(NamedKey::Enter) => {
                if !self.editor.newline(now)? {
                    return Ok(TextResponse::Submit);
                }
            }
            Key::Named(NamedKey::Tab) => {
                if !self.editor.tab(now)? {
                    return Ok(TextResponse::Traverse(extend));
                }
            }
            Key::Named(NamedKey::ArrowUp | NamedKey::ArrowDown) => {
                let layout = self.layout.as_ref().ok_or(TextError::StaleRevision)?;
                self.editor.move_vertical(
                    layout.as_ref(),
                    event.key == Key::Named(NamedKey::ArrowDown),
                    extend,
                    now,
                )?;
            }
            Key::Named(NamedKey::Escape) => {
                self.editor.cancel_composition();
                self.dragging = false;
            }
            _ => return Ok(TextResponse::Ignored),
        }
        Ok(TextResponse::Handled)
    }
    pub fn pointer(
        &mut self,
        event: PointerEvent,
        content: LogicalRect,
    ) -> Result<TextResponse, TextError> {
        if self.editor.disabled() {
            return Ok(TextResponse::Ignored);
        }
        if event.phase == PointerPhase::Cancel {
            self.dragging = false;
            return Ok(TextResponse::Handled);
        }
        if event.phase == PointerPhase::Down
            && event.button == Some(PointerButton::Secondary)
            && content.contains(event.position)
        {
            return Ok(TextResponse::ContextMenu);
        }
        let down = event.phase == PointerPhase::Down
            && event.button == Some(PointerButton::Primary)
            && content.contains(event.position);
        if down {
            self.set_focused(true, event.timestamp);
            self.editor.cancel_composition();
            self.dragging = true;
        }
        if self.dragging
            && matches!(
                event.phase,
                PointerPhase::Down | PointerPhase::Move | PointerPhase::Up
            )
        {
            let layout = self.layout.as_ref().ok_or(TextError::StaleRevision)?;
            self.editor.pointer_select(
                layout.as_ref(),
                event.position - content.origin,
                !down || event.modifiers.shift,
                content.size,
                event.timestamp,
            )?;
            if event.phase == PointerPhase::Up {
                self.dragging = false;
            }
            return Ok(TextResponse::Handled);
        }
        Ok(TextResponse::Ignored)
    }
    pub fn select_word(&mut self, point: LogicalPoint) -> Result<(), TextError> {
        let layout = self.layout.as_ref().ok_or(TextError::StaleRevision)?;
        let caret = layout.hit_test(point + self.editor.scroll_offset())?;
        self.editor.select_word(caret.index)
    }
    /// Shapes committed navigation and transient display independently; preparation
    /// may reveal the caret but never applies an editing command.
    pub fn prepare(
        &mut self,
        backend: &mut CosmicBackend,
        style: &Style,
        bounds: LogicalRect,
        scale: ScaleFactor,
        now: u64,
    ) -> Result<TextPresentation, ControlError> {
        style.validate()?;
        if !bounds.is_valid()
            || bounds.size.width <= style.padding * 2.0
            || bounds.size.height <= style.padding * 2.0
        {
            return Err(ControlError::Invalid("field bounds"));
        }
        let pad = style.padding;
        let content = LogicalRect::new(
            bounds.origin + LogicalPoint::new(pad, pad),
            LogicalSize::new(
                (bounds.size.width - pad * 2.0).max(1.0),
                (bounds.size.height - pad * 2.0).max(1.0),
            ),
        );
        let multiline = self.editor.config().mode == LineMode::Multiline;
        let font_revision = backend.font_revision();
        let request = |text| LayoutRequest {
            text,
            font_family: style.font_family.clone(),
            font_size: style.font_size,
            line_height: style.line_height,
            max_width: if multiline {
                Some(content.size.width)
            } else {
                None
            },
            wrap: if multiline { Wrap::Word } else { Wrap::None },
            scale,
            font_revision,
        };
        let committed = self.snapshot();
        let committed_layout = Arc::new(backend.shape(&request(committed.text.clone()))?);
        if self.focused {
            self.editor.reveal(
                committed_layout.as_ref(),
                Caret {
                    index: committed.selection.focus,
                    affinity: Affinity::Downstream,
                },
                content.size,
            )?;
        }
        self.layout = Some(committed_layout);
        let display = self.editor.display_snapshot()?;
        let mut layout = Arc::new(backend.shape(&request(display.text.clone()))?);
        let raw_caret = Caret {
            index: display.selection.focus,
            affinity: Affinity::Downstream,
        };
        let mut candidate = self.editor.candidate_rect(layout.as_ref(), raw_caret)?;
        let mut display_scroll = self.editor.scroll_offset();
        if committed.composition.is_some() {
            let dx = candidate.left().min(0.0) + (candidate.right() - content.size.width).max(0.0);
            let dy = candidate.top().min(0.0) + (candidate.bottom() - content.size.height).max(0.0);
            display_scroll = display_scroll + LogicalPoint::new(dx, dy);
            candidate.origin = candidate.origin - LogicalPoint::new(dx, dy);
        }
        candidate.origin = candidate.origin + content.origin;
        candidate.origin.x = candidate
            .origin
            .x
            .clamp(content.left(), content.right() - 1.0);
        candidate.size.height = candidate.size.height.min(content.size.height);
        candidate.origin.y = candidate
            .origin
            .y
            .clamp(content.top(), content.bottom() - candidate.size.height);
        let boundaries = grapheme_boundaries(display.text.text());
        let snap = |i: usize| {
            TextIndex(
                *boundaries
                    .iter()
                    .find(|b| **b >= i)
                    .unwrap_or(&display.text.text().len()),
            )
        };
        let preedit = committed.composition.as_ref().map(|c| Selection {
            anchor: snap(c.range.start),
            focus: snap(c.range.start + c.preedit.len()),
        });
        let caret = (self.editor.caret_visible(now)
            && committed
                .composition
                .as_ref()
                .is_none_or(|c| c.cursor_visible))
        .then_some(Caret {
            index: snap(raw_caret.index.0),
            ..raw_caret
        });
        let selection =
            (display.selection.anchor != display.selection.focus).then_some(Selection {
                anchor: snap(display.selection.anchor.0),
                focus: snap(display.selection.focus.0),
            });
        let placeholder = self.editor.placeholder().is_some();
        if let Some(text) = self.editor.placeholder() {
            layout = Arc::new(
                backend.shape(&request(TextSnapshot::new(text, display.text.revision())))?,
            );
        }
        Ok(TextPresentation {
            layout,
            origin: content.origin - display_scroll,
            clip: content,
            selection,
            caret,
            preedit,
            candidate,
            placeholder,
        })
    }
    pub fn semantics(&self, id: NodeId, bounds: LogicalRect) -> ControlSemantics {
        let snapshot = self.snapshot();
        let mut node = SemanticNode::new(id, Role::TextInput);
        node.name = Some(self.label.clone());
        node.bounds = bounds;
        node.value = Some(snapshot.text.text().into());
        node.disabled = self.editor.disabled();
        node.focused = self.focused;
        if !node.disabled {
            node.actions.push(SemanticAction::Focus);
            if !self.editor.read_only() {
                node.actions.push(SemanticAction::SetValue);
            }
        }
        let mut s = ControlSemantics::new(node);
        s.read_only = self.editor.read_only();
        s.multiline = self.editor.config().mode == LineMode::Multiline;
        s.selection = Some(snapshot.selection);
        s.invalid = self.invalid;
        s
    }
}
impl ProfileAdapter for TextField {
    fn configuration(&self) -> EditorConfig {
        self.editor.config().clone()
    }
    fn reset(&mut self, value: &str) -> Result<(), TextError> {
        self.editor = PlainEditor::new(value, self.editor.config().clone())?;
        self.layout = None;
        self.dragging = false;
        self.editor.set_focused(self.focused, 0);
        Ok(())
    }
    fn select_range(&mut self, s: Selection) -> Result<(), TextError> {
        self.editor
            .select(self.editor.snapshot().text.revision(), s)
    }
    fn insert(&mut self, s: &str) -> Result<(), TextError> {
        self.insert_text(s, 0)
    }
    fn backspace(&mut self) -> Result<(), TextError> {
        self.editor.delete(true, false, 0)
    }
    fn undo(&mut self) -> Result<(), TextError> {
        self.editor.undo().map(|_| ())
    }
    fn value(&self) -> String {
        self.snapshot().text.text().into()
    }
    fn observation(&self) -> EditorSnapshot {
        self.snapshot()
    }
    fn external(&mut self, base: TextRevision, value: &str) -> Result<ValueUpdate, TextError> {
        self.external(base, value, ExternalPolicy::PreserveSelection)
    }
    fn command(&mut self, command: ProfileCommand) -> Result<bool, TextError> {
        match command {
            ProfileCommand::Modes {
                read_only,
                disabled,
            } => {
                self.set_modes(read_only, disabled);
                Ok(true)
            }
            ProfileCommand::Preedit(text) => {
                self.preedit(&text, Some((text.len(), text.len())))?;
                Ok(true)
            }
            ProfileCommand::Commit(text) => {
                self.commit_ime(&text, 0)?;
                Ok(true)
            }
            _ => ProfileAdapter::command(&mut self.editor, command),
        }
    }
}
pub struct TextInput(pub TextField);
impl TextInput {
    pub fn new(label: impl Into<String>, value: &str) -> Result<Self, TextError> {
        Ok(Self(TextField::new(label, value, EditorConfig::default())?))
    }
}
pub struct TextArea(pub TextField);
impl TextArea {
    pub fn new(label: impl Into<String>, value: &str) -> Result<Self, TextError> {
        Ok(Self(TextField::new(
            label,
            value,
            EditorConfig {
                mode: LineMode::Multiline,
                ..Default::default()
            },
        )?))
    }
}

pub struct NumericField {
    pub field: TextField,
    range: RangeBehavior,
}
impl NumericField {
    pub fn new(
        label: impl Into<String>,
        spec: RangeSpec,
        value: f64,
    ) -> Result<Self, ControlError> {
        let range = RangeBehavior::new(spec, value)?;
        Ok(Self {
            field: TextField::new(
                label,
                &format!("{:.*}", spec.decimals as usize, range.value()),
                EditorConfig::default(),
            )?,
            range,
        })
    }
    pub fn value(&self) -> f64 {
        self.range.value()
    }
    pub fn validate_pending(&mut self) -> bool {
        let text = self.field.snapshot();
        self.field.invalid = text.text.text().parse::<f64>().ok().is_none_or(|v| {
            !v.is_finite() || v < self.range.spec().min || v > self.range.spec().max
        });
        !self.field.invalid
    }
    pub fn commit(&mut self) -> Result<bool, ControlError> {
        if self.field.editor().disabled() || self.field.editor().read_only() {
            return Err(ControlError::Disabled);
        }
        if !self.validate_pending() {
            return Err(ControlError::Invalid("numeric intermediate value"));
        }
        let value = self
            .field
            .snapshot()
            .text
            .text()
            .parse::<f64>()
            .map_err(|_| ControlError::Invalid("numeric text"))?;
        let changed = self.range.set(value)?;
        self.cancel()?;
        Ok(changed)
    }
    pub fn cancel(&mut self) -> Result<(), ControlError> {
        let revision = self.field.snapshot().text.revision();
        self.field.external(
            revision,
            &format!(
                "{:.*}",
                self.range.spec().decimals as usize,
                self.range.value()
            ),
            ExternalPolicy::ResetSelection,
        )?;
        self.field.invalid = false;
        Ok(())
    }
    pub fn key(
        &mut self,
        e: &KeyboardEvent,
        clipboard: &mut impl Clipboard,
    ) -> Result<TextResponse, ControlError> {
        if e.phase != KeyPhase::Up && self.field.focused() {
            if e.key == Key::Named(NamedKey::Escape) {
                self.cancel()?;
                return Ok(TextResponse::Handled);
            }
            if matches!(
                e.key,
                Key::Named(
                    NamedKey::ArrowUp | NamedKey::ArrowDown | NamedKey::PageUp | NamedKey::PageDown
                )
            ) {
                if self.field.editor().disabled() || self.field.editor().read_only() {
                    return Err(ControlError::Disabled);
                }
                self.range.key(e)?;
                self.cancel()?;
                return Ok(TextResponse::Handled);
            }
        }
        let response = self.field.key(e, clipboard)?;
        if response == TextResponse::Submit {
            self.commit()?;
        }
        Ok(response)
    }
}
