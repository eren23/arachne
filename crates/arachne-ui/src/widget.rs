use crate::context::WidgetInteraction;

/// Generic response from a widget.
#[derive(Clone, Copy, Debug, Default)]
pub struct WidgetResponse {
    pub clicked: bool,
    pub hovered: bool,
    pub active: bool,
    pub focused: bool,
    pub changed: bool,
}

impl WidgetResponse {
    pub fn from_interaction(i: WidgetInteraction) -> Self {
        Self {
            clicked: i.clicked,
            hovered: i.hovered,
            active: i.active,
            focused: i.focused,
            changed: false,
        }
    }
}

/// Approximate text width using monospace metrics.
/// Each character is approximately `font_size * 0.6` wide.
pub fn estimate_text_width(text: &str, font_size: f32) -> f32 {
    text.len() as f32 * font_size * 0.6
}
