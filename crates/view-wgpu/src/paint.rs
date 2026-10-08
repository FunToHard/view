use crate::{DeviceGeneration, RenderError};
use std::sync::Arc;
use view_core::{LogicalPoint, LogicalRect, Revision, ScaleFactor, Transform2D};

/// Straight sRGB input. Converted to linear premultiplied values at upload.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color(pub [f32; 4]);
impl Color {
    pub const WHITE: Self = Self([1.0; 4]);
    pub const TRANSPARENT: Self = Self([0.0; 4]);
    pub fn linear_premultiplied(self) -> Result<[f32; 4], RenderError> {
        if self
            .0
            .iter()
            .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
        {
            return Err(RenderError::Invalid("color".into()));
        }
        let [r, g, b, a] = self.0;
        let linear = |v: f32| {
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        Ok([linear(r) * a, linear(g) * a, linear(b) * a, a])
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlphaMode {
    Straight,
    Premultiplied,
    Opaque,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    TopLeft,
    BottomLeft,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transfer {
    Srgb,
    Linear,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextureMetadata {
    pub alpha: AlphaMode,
    pub origin: Origin,
    pub transfer: Transfer,
}
impl Default for TextureMetadata {
    fn default() -> Self {
        Self {
            alpha: AlphaMode::Straight,
            origin: Origin::TopLeft,
            transfer: Transfer::Srgb,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ImageId {
    pub(crate) generation: DeviceGeneration,
    pub(crate) serial: u64,
}

#[derive(Clone)]
pub enum Primitive {
    Rect(LogicalRect),
    Fill(lyon::path::Path),
    Stroke {
        path: lyon::path::Path,
        width: f32,
    },
    Image {
        image: ImageId,
        rect: LogicalRect,
    },
    /// The exact cosmic buffer used to derive the shared editor's geometry.
    Text {
        layout: Arc<view_text::ShapedText>,
        origin: LogicalPoint,
    },
}
#[derive(Clone)]
pub struct Draw {
    pub primitive: Primitive,
    pub color: Color,
    pub transform: Transform2D,
    /// Window logical coordinates, matching committed hit geometry. Empty clips
    /// draw nothing. Rotated local clips must be expressed as an isolated layer.
    pub clip: Option<LogicalRect>,
}
impl Draw {
    pub fn new(primitive: Primitive, color: Color) -> Self {
        Self {
            primitive,
            color,
            transform: Transform2D::IDENTITY,
            clip: None,
        }
    }
}
#[derive(Clone)]
pub enum Paint {
    Draw(Draw),
    Layer { opacity: f32, items: Vec<Paint> },
}
#[derive(Clone)]
pub struct PaintSnapshot {
    pub revision: Revision,
    pub size: [u32; 2],
    pub scale: ScaleFactor,
    pub items: Vec<Paint>,
}

/// Shared editor geometry composed in explicit selection/text/preedit/caret order.
/// `preedit` is a selection in the caller's shaped display snapshot; committed
/// value and transient IME display snapshot ownership remain with the editor.
pub struct TextVisuals {
    pub layout: Arc<view_text::ShapedText>,
    pub origin: LogicalPoint,
    pub selection: Option<view_text::Selection>,
    pub caret: Option<view_text::Caret>,
    pub preedit: Option<view_text::Selection>,
    pub foreground: Color,
    pub selection_color: Color,
    pub clip: Option<LogicalRect>,
}
impl TextVisuals {
    pub fn paint(&self) -> Result<Vec<Paint>, RenderError> {
        use view_text::TextLayout;
        let mut result = Vec::new();
        let draw_rect = |mut rect: LogicalRect, color: Color| {
            rect.origin = rect.origin + self.origin;
            let mut draw = Draw::new(Primitive::Rect(rect), color);
            draw.clip = self.clip;
            Paint::Draw(draw)
        };
        let error = |e: view_text::TextError| RenderError::Invalid(e.to_string());
        if let Some(selection) = self.selection {
            for rect in self.layout.selection_bounds(selection).map_err(error)? {
                result.push(draw_rect(rect, self.selection_color));
            }
        }
        let mut text = Draw::new(
            Primitive::Text {
                layout: self.layout.clone(),
                origin: self.origin,
            },
            self.foreground,
        );
        text.clip = self.clip;
        result.push(Paint::Draw(text));
        if let Some(preedit) = self.preedit {
            for mut rect in self.layout.selection_bounds(preedit).map_err(error)? {
                rect.origin.y += rect.size.height - 1.0;
                rect.size.height = 1.0;
                result.push(draw_rect(rect, self.foreground));
            }
        }
        if let Some(caret) = self.caret {
            result.push(draw_rect(
                self.layout.caret_bounds(caret).map_err(error)?,
                self.foreground,
            ));
        }
        Ok(result)
    }
}
