//! Password entry state with redacted presentation and accessibility output.
//!
//! This is a UI privacy boundary, not secure memory: Rust strings may be copied
//! or retained by allocators. Applications should avoid logging the value.
use crate::{ControlError, ControlSemantics, TextCommand, TextField, TextResponse};
use view_core::{LogicalRect, NodeId, PointerEvent, ScaleFactor};
use view_text::{Clipboard, EditorConfig, ExternalPolicy, TextError, ValueUpdate};

/// Password entry backed by the shared plain-text editing session. It deliberately
/// does not implement `Debug` or `Clone`, since either could duplicate/expose secret state.
pub struct PasswordField {
    field: TextField,
    revealed: bool,
}

impl PasswordField {
    pub fn new(label: impl Into<String>) -> Result<Self, TextError> {
        let config = EditorConfig {
            newline: view_text::NewlinePolicy::Reject,
            ..EditorConfig::default()
        };
        let mut field = TextField::new(label, "", config)?;
        field.mark_password();
        Ok(Self {
            field,
            revealed: false,
        })
    }
    pub fn label(&self) -> &str {
        &self.field.label
    }
    pub fn display_text(&self) -> String {
        if self.revealed {
            self.field.snapshot().text.text().to_owned()
        } else {
            self.field.masked_display()
        }
    }
    pub fn set_value(&mut self, value: impl Into<String>) -> Result<(), ControlError> {
        if self.field.editor().disabled() || self.field.editor().read_only() {
            return Err(ControlError::Disabled);
        }
        let snapshot = self.field.snapshot();
        self.field
            .external(
                snapshot.text.revision(),
                &value.into(),
                ExternalPolicy::ResetSelection,
            )
            .map(|_| ())
            .map_err(ControlError::from)
    }
    /// Explicitly retrieve the secret for application submission.
    pub fn expose_value(&self) -> String {
        self.field.snapshot().text.text().to_owned()
    }
    pub fn set_focused(&mut self, focused: bool) {
        self.field.set_focused(focused, 0);
    }
    pub fn set_modes(&mut self, read_only: bool, disabled: bool) {
        self.field.set_modes(read_only, disabled);
        if disabled {
            self.revealed = false;
        }
    }
    pub fn set_revealed(&mut self, revealed: bool) -> Result<(), ControlError> {
        if self.field.editor().disabled() {
            return Err(ControlError::Disabled);
        }
        self.revealed = revealed;
        Ok(())
    }
    pub fn revealed(&self) -> bool {
        self.revealed
    }
    pub fn insert_text(&mut self, text: &str, now: u64) -> Result<(), TextError> {
        self.field.insert_text(text, now)
    }
    pub fn key(
        &mut self,
        event: &view_core::KeyboardEvent,
        clipboard: &mut impl Clipboard,
    ) -> Result<TextResponse, TextError> {
        self.field.key(event, clipboard)
    }
    pub fn pointer(
        &mut self,
        event: PointerEvent,
        content: LogicalRect,
    ) -> Result<TextResponse, TextError> {
        self.field.pointer(event, content)
    }
    pub fn command(
        &mut self,
        command: TextCommand,
        clipboard: &mut impl Clipboard,
        now: u64,
    ) -> Result<TextResponse, TextError> {
        self.field.command(command, clipboard, now)
    }
    pub fn prepare(
        &mut self,
        backend: &mut view_text::CosmicBackend,
        style: &crate::Style,
        bounds: LogicalRect,
        scale: ScaleFactor,
        now: u64,
    ) -> Result<crate::TextPresentation, ControlError> {
        if self.revealed {
            self.field.prepare(backend, style, bounds, scale, now)
        } else {
            self.field
                .prepare_password(backend, style, bounds, scale, now)
        }
    }
    pub fn external(
        &mut self,
        base: view_text::TextRevision,
        value: &str,
        policy: ExternalPolicy,
    ) -> Result<ValueUpdate, TextError> {
        self.field.external(base, value, policy)
    }
    pub fn semantics(&self, id: NodeId, bounds: LogicalRect) -> ControlSemantics {
        self.field.semantics(id, bounds)
    }
}
