//! Optional renderer decoration. Behavior and application effects live elsewhere.
use crate::{Style, TextPresentation};
use view_core::{LogicalPoint, LogicalRect, LogicalSize};
use view_wgpu::{Color, Draw, Paint, Primitive, RenderError, TextVisuals};

impl crate::Image {
    /// Resolve an application asset to a renderer-owned image. The renderer
    /// validates device generation and lifetime when preparing this draw.
    pub fn paint(
        &self,
        bounds: LogicalRect,
        resolve: impl FnOnce(u64) -> Option<view_wgpu::ImageId>,
    ) -> Result<Paint, crate::ControlError> {
        if !bounds.is_valid() {
            return Err(crate::ControlError::Invalid("image bounds"));
        }
        let image = resolve(self.asset).ok_or(crate::ControlError::Stale)?;
        Ok(Paint::Draw(Draw::new(
            Primitive::Image {
                image,
                rect: bounds,
            },
            Color::WHITE,
        )))
    }
}

impl crate::Label {
    /// Pure text preparation with explicit font, width and DPI inputs.
    pub fn paint(
        &self,
        backend: &mut view_text::CosmicBackend,
        style: &Style,
        bounds: LogicalRect,
        scale: view_core::ScaleFactor,
    ) -> Result<Paint, crate::ControlError> {
        use view_text::TextBackend;
        style.validate()?;
        if !bounds.is_valid() {
            return Err(crate::ControlError::Invalid("label bounds"));
        }
        let layout = backend.shape(&view_text::LayoutRequest {
            text: view_text::TextSnapshot::new(
                self.text.as_str(),
                view_text::TextRevision::INITIAL,
            ),
            font_family: style.font_family.clone(),
            font_size: style.font_size,
            line_height: style.line_height,
            max_width: Some(bounds.size.width),
            wrap: view_text::Wrap::Word,
            scale,
            font_revision: backend.font_revision(),
        })?;
        let mut draw = Draw::new(
            Primitive::Text {
                layout: std::sync::Arc::new(layout),
                origin: bounds.origin,
            },
            Color(style.foreground.0),
        );
        draw.clip = Some(bounds);
        Ok(Paint::Draw(draw))
    }
}

impl crate::ControlRow {
    /// Draw only the visible portion; layout and input remain backend independent.
    pub fn paint(
        &self,
        backend: &mut view_text::CosmicBackend,
        style: &Style,
        scale: view_core::ScaleFactor,
    ) -> Result<Vec<Paint>, crate::ControlError> {
        let Some(clip) = self.bounds.intersection(self.clip) else {
            return Ok(vec![]);
        };
        let style = style.interaction(crate::Interaction {
            disabled: self.disabled,
            focused: self.focused,
            ..Default::default()
        });
        let mut items = vec![];
        if self.selected {
            items.push(Paint::Draw(Draw::new(
                Primitive::Rect(clip),
                Color(style.selection.0),
            )));
        }
        if self.focused {
            let edge = LogicalRect::new(
                clip.origin,
                LogicalSize::new(2.0_f32.min(clip.size.width), clip.size.height),
            );
            items.push(Paint::Draw(Draw::new(
                Primitive::Rect(edge),
                Color(style.accent.0),
            )));
        }
        let mut label =
            crate::Label::new(&self.label).paint(backend, &style, self.bounds, scale)?;
        if let Paint::Draw(draw) = &mut label {
            draw.clip = Some(clip);
        }
        items.push(label);
        Ok(items)
    }
}

/// Paint a rectangular control frame, preserving the interior size.
pub fn frame(bounds: LogicalRect, style: &Style) -> Vec<Paint> {
    let width = style
        .border_width
        .min(bounds.size.width / 2.0)
        .min(bounds.size.height / 2.0);
    let inner = LogicalRect::new(
        bounds.origin + LogicalPoint::new(width, width),
        LogicalSize::new(
            (bounds.size.width - 2.0 * width).max(0.0),
            (bounds.size.height - 2.0 * width).max(0.0),
        ),
    );
    vec![
        Paint::Draw(Draw::new(Primitive::Rect(bounds), Color(style.border.0))),
        Paint::Draw(Draw::new(Primitive::Rect(inner), Color(style.background.0))),
    ]
}

impl TextPresentation {
    /// The same geometry drives both text decoration and the native candidate box.
    pub fn paint(&self, style: &Style) -> Result<Vec<Paint>, RenderError> {
        let mut foreground = style.foreground;
        if self.placeholder {
            foreground.0[3] *= 0.5;
        }
        let mut result = TextVisuals {
            layout: self.layout.clone(),
            origin: self.origin,
            selection: self.selection,
            caret: if self.placeholder { None } else { self.caret },
            preedit: self.preedit,
            foreground: Color(foreground.0),
            selection_color: Color(style.selection.0),
            clip: Some(self.clip),
        }
        .paint()?;
        if self.placeholder && self.caret.is_some() {
            let mut draw = Draw::new(Primitive::Rect(self.candidate), Color(style.foreground.0));
            draw.clip = Some(self.clip);
            result.push(Paint::Draw(draw));
        }
        Ok(result)
    }
}
