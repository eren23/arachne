use arachne_math::{Color, Rect, Vec2};

/// An opaque handle to a texture. Compatible with arachne-render's TextureHandle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextureHandle(pub u32);

/// How an image should be sized within its container.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ImageSizing {
    /// Scale to fit entirely within the container, preserving aspect ratio.
    Contain,
    /// Scale to cover the entire container, preserving aspect ratio (may crop).
    Cover,
    /// Stretch to fill the container exactly.
    Fill,
    /// Display at original size, no scaling.
    None,
}

impl Default for ImageSizing {
    fn default() -> Self {
        ImageSizing::Contain
    }
}

/// A 2D draw command emitted by the UI system.
#[derive(Clone, Debug)]
pub enum DrawCommand {
    FilledRect {
        rect: Rect,
        color: Color,
        border_radius: f32,
        clip: Option<Rect>,
    },
    BorderedRect {
        rect: Rect,
        fill: Color,
        border_color: Color,
        border_width: f32,
        border_radius: f32,
        clip: Option<Rect>,
    },
    Text {
        position: Vec2,
        text: String,
        color: Color,
        font_size: f32,
        clip: Option<Rect>,
    },
    Image {
        rect: Rect,
        texture: TextureHandle,
        sizing: ImageSizing,
        clip: Option<Rect>,
    },
}
