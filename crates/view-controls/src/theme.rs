use crate::{ControlError, Interaction};
use view_core::{Dirty, Revision};
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba(pub [f32; 4]);
impl Rgba {
    pub fn validate(self) -> bool {
        self.0
            .iter()
            .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Style {
    pub foreground: Rgba,
    pub background: Rgba,
    pub border: Rgba,
    pub accent: Rgba,
    pub selection: Rgba,
    pub padding: f32,
    pub border_width: f32,
    pub font_size: f32,
    pub line_height: f32,
    pub font_family: String,
}
impl Default for Style {
    fn default() -> Self {
        Self {
            foreground: Rgba([0.9, 0.92, 0.96, 1.0]),
            background: Rgba([0.12, 0.14, 0.19, 1.0]),
            border: Rgba([0.35, 0.4, 0.5, 1.0]),
            accent: Rgba([0.2, 0.65, 1.0, 1.0]),
            selection: Rgba([0.2, 0.45, 0.85, 0.6]),
            padding: 6.0,
            border_width: 1.0,
            font_size: 16.0,
            line_height: 22.0,
            font_family: "sans-serif".into(),
        }
    }
}
impl Style {
    pub fn validate(&self) -> Result<(), ControlError> {
        if ![
            self.foreground,
            self.background,
            self.border,
            self.accent,
            self.selection,
        ]
        .iter()
        .all(|v| v.validate())
            || [
                self.padding,
                self.border_width,
                self.font_size,
                self.line_height,
            ]
            .iter()
            .any(|v| !v.is_finite() || *v < 0.0)
            || self.font_size == 0.0
            || self.line_height == 0.0
        {
            Err(ControlError::Invalid("style"))
        } else {
            Ok(())
        }
    }
    pub fn interaction(&self, state: Interaction) -> Self {
        let mut result = self.clone();
        if state.focused {
            result.border = result.accent;
        }
        if state.pressed {
            result.background = result.accent;
        } else if state.hovered {
            result.background.0[..3]
                .iter_mut()
                .for_each(|c| *c = (*c + 0.08).min(1.0));
        }
        if state.disabled {
            result.foreground.0[3] *= 0.45;
        }
        result
    }
}
#[derive(Clone, Debug, Default)]
pub struct StyleOverride {
    pub foreground: Option<Rgba>,
    pub background: Option<Rgba>,
    pub accent: Option<Rgba>,
    pub padding: Option<f32>,
    pub font_size: Option<f32>,
    pub line_height: Option<f32>,
    pub font_family: Option<String>,
}
impl StyleOverride {
    pub fn resolve(&self, inherited: &Style) -> Result<Style, ControlError> {
        let mut style = inherited.clone();
        if let Some(v) = self.foreground {
            style.foreground = v;
        }
        if let Some(v) = self.background {
            style.background = v;
        }
        if let Some(v) = self.accent {
            style.accent = v;
        }
        if let Some(v) = self.padding {
            style.padding = v;
        }
        if let Some(v) = self.font_size {
            style.font_size = v;
        }
        if let Some(v) = self.line_height {
            style.line_height = v;
        }
        if let Some(v) = &self.font_family {
            style.font_family = v.clone();
        }
        style.validate()?;
        Ok(style)
    }
}
pub struct Theme {
    style: Style,
    revision: Revision,
}
impl Theme {
    pub fn new(style: Style) -> Result<Self, ControlError> {
        style.validate()?;
        Ok(Self {
            style,
            revision: Revision::INITIAL,
        })
    }
    pub fn style(&self) -> &Style {
        &self.style
    }
    pub fn revision(&self) -> Revision {
        self.revision
    }
    /// Call for each node after resolving its inherited/local override. A color
    /// reload invalidates paint only; font/spacing changes also invalidate layout.
    pub fn difference(old: &Style, new: &Style) -> Dirty {
        if old == new {
            Dirty::NONE
        } else if old.padding != new.padding
            || old.border_width != new.border_width
            || old.font_size != new.font_size
            || old.line_height != new.line_height
            || old.font_family != new.font_family
        {
            Dirty::LAYOUT.union(Dirty::PAINT)
        } else {
            Dirty::PAINT
        }
    }
    pub fn reload(&mut self, style: Style) -> Result<Dirty, ControlError> {
        style.validate()?;
        let dirty = Self::difference(&self.style, &style);
        if dirty != Dirty::NONE {
            self.revision = self
                .revision
                .checked_next()
                .map_err(|_| ControlError::Stale)?;
            self.style = style;
        }
        Ok(dirty)
    }
}
