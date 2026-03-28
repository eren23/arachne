use arachne_math::Color;
use crate::context::{NodePaintCmd, UIContext, WidgetId};
use crate::layout::{LayoutNode, Size};
use crate::widget::estimate_text_width;

/// Text alignment within a label.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// A text display widget.
pub struct Label {
    text: String,
    id_override: Option<String>,
    width: Option<f32>,
    color: Option<Color>,
    font_size: Option<f32>,
    align: TextAlign,
    wrap: bool,
}

impl Label {
    pub fn new(text: &str) -> Self {
        Self {
            text: text.to_string(),
            id_override: None,
            width: None,
            color: None,
            font_size: None,
            align: TextAlign::Left,
            wrap: true,
        }
    }

    pub fn id(mut self, id: &str) -> Self {
        self.id_override = Some(id.to_string());
        self
    }
    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }
    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }
    pub fn font_size(mut self, s: f32) -> Self {
        self.font_size = Some(s);
        self
    }
    pub fn align(mut self, a: TextAlign) -> Self {
        self.align = a;
        self
    }
    pub fn wrap(mut self, w: bool) -> Self {
        self.wrap = w;
        self
    }

    pub fn show(&self, ctx: &mut UIContext) {
        let id_str = self.id_override.as_deref().unwrap_or(&self.text);
        let id = WidgetId::new(id_str);
        let font_size = self.font_size.unwrap_or(ctx.theme().default.font_size);
        let text_w = estimate_text_width(&self.text, font_size);

        let layout = LayoutNode {
            width: Size::Fixed(self.width.unwrap_or(text_w)),
            height: Size::Fixed(font_size),
            ..Default::default()
        };

        let node_id = ctx.add_widget(id, layout, false);
        let color = self.color.unwrap_or(ctx.theme().default.text_color);

        let display_text = if !self.wrap {
            let max_w = self.width.unwrap_or(f32::MAX);
            truncate_with_ellipsis(&self.text, max_w, font_size)
        } else {
            self.text.clone()
        };

        ctx.add_paint_cmd(
            node_id,
            NodePaintCmd::Text {
                text: display_text,
                color,
                font_size,
            },
        );
    }
}

fn truncate_with_ellipsis(text: &str, max_width: f32, font_size: f32) -> String {
    let char_w = font_size * 0.6;
    let text_w = text.len() as f32 * char_w;
    if text_w <= max_width {
        text.to_string()
    } else {
        let ellipsis_w = 3.0 * char_w;
        let available = max_width - ellipsis_w;
        let max_chars = (available / char_w).max(0.0) as usize;
        let truncated: String = text.chars().take(max_chars).collect();
        format!("{}...", truncated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncation() {
        // font_size 10 -> char_w = 6
        // "Hello World" = 11 chars = 66px
        // max_width = 50 -> ellipsis = 18px, available = 32px, max_chars = 5
        let result = truncate_with_ellipsis("Hello World", 50.0, 10.0);
        assert!(result.ends_with("..."));
        assert!(result.len() <= 8); // 5 + 3
    }

    #[test]
    fn no_truncation_when_fits() {
        let result = truncate_with_ellipsis("Hi", 100.0, 10.0);
        assert_eq!(result, "Hi");
    }
}
