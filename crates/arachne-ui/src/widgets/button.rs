use crate::context::{NodePaintCmd, UIContext, WidgetId, WidgetInteraction};
use crate::layout::{LayoutNode, Size};
use crate::render::TextureHandle;
use crate::widget::estimate_text_width;

/// An immediate-mode button widget.
pub struct Button {
    label: String,
    width: Option<f32>,
    height: Option<f32>,
    disabled: bool,
    icon: Option<TextureHandle>,
}

impl Button {
    pub fn new(label: &str) -> Self {
        Self {
            label: label.to_string(),
            width: None,
            height: None,
            disabled: false,
            icon: None,
        }
    }

    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }
    pub fn height(mut self, h: f32) -> Self {
        self.height = Some(h);
        self
    }
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }
    pub fn icon(mut self, i: TextureHandle) -> Self {
        self.icon = Some(i);
        self
    }

    /// Show the button. Returns `true` if it was clicked this frame.
    pub fn show(&self, ctx: &mut UIContext) -> bool {
        let id = WidgetId::new(&self.label);

        let font_size = ctx.theme().default.font_size;
        let padding = ctx.theme().default.padding;
        let text_w = estimate_text_width(&self.label, font_size);
        let default_w = text_w + padding.horizontal();
        let default_h = font_size + padding.vertical();

        let layout = LayoutNode {
            width: Size::Fixed(self.width.unwrap_or(default_w)),
            height: Size::Fixed(self.height.unwrap_or(default_h)),
            ..Default::default()
        };

        let node_id = ctx.add_widget(id, layout, !self.disabled);

        let interaction = if self.disabled {
            WidgetInteraction::default()
        } else {
            ctx.get_interaction(id)
        };

        let style = ctx.style_for_state(self.disabled, &interaction);

        ctx.add_paint_cmd(
            node_id,
            NodePaintCmd::BorderedRect {
                fill: style.background_color,
                border_color: style.border_color,
                border_width: style.border_width,
                border_radius: style.border_radius,
            },
        );

        ctx.add_paint_cmd(
            node_id,
            NodePaintCmd::Text {
                text: self.label.clone(),
                color: style.text_color,
                font_size: style.font_size,
            },
        );

        interaction.clicked
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::InputState;
    use arachne_math::Vec2;

    #[test]
    fn button_click_fires_once() {
        let mut ctx = UIContext::new();

        // Frame 1: show button to establish layout
        ctx.begin_frame(InputState::default());
        let clicked = Button::new("test").width(100.0).height(40.0).show(&mut ctx);
        ctx.end_frame(800.0, 600.0);
        assert!(!clicked);

        // Frame 2: click on button
        let input = InputState {
            mouse_pos: Vec2::new(50.0, 20.0),
            mouse_just_pressed: true,
            mouse_just_released: true,
            ..Default::default()
        };
        ctx.begin_frame(input);
        let clicked = Button::new("test").width(100.0).height(40.0).show(&mut ctx);
        ctx.end_frame(800.0, 600.0);
        assert!(clicked);
    }

    #[test]
    fn button_disabled_ignores_click() {
        let mut ctx = UIContext::new();

        // Frame 1
        ctx.begin_frame(InputState::default());
        Button::new("dis").width(100.0).height(40.0).disabled(true).show(&mut ctx);
        ctx.end_frame(800.0, 600.0);

        // Frame 2: click
        let input = InputState {
            mouse_pos: Vec2::new(50.0, 20.0),
            mouse_just_pressed: true,
            mouse_just_released: true,
            ..Default::default()
        };
        ctx.begin_frame(input);
        let clicked = Button::new("dis").width(100.0).height(40.0).disabled(true).show(&mut ctx);
        ctx.end_frame(800.0, 600.0);
        assert!(!clicked);
    }

    #[test]
    fn button_hover_state() {
        let mut ctx = UIContext::new();

        // Frame 1: show button
        ctx.begin_frame(InputState::default());
        Button::new("hov").width(100.0).height(40.0).show(&mut ctx);
        ctx.end_frame(800.0, 600.0);

        // Frame 2: hover (mouse over, no click)
        let input = InputState {
            mouse_pos: Vec2::new(50.0, 20.0),
            ..Default::default()
        };
        ctx.begin_frame(input);
        Button::new("hov").width(100.0).height(40.0).show(&mut ctx);
        ctx.end_frame(800.0, 600.0);
        assert_eq!(ctx.hovered_widget(), Some(WidgetId::new("hov")));

        // Frame 3: mouse moves away
        let input = InputState {
            mouse_pos: Vec2::new(500.0, 500.0),
            ..Default::default()
        };
        ctx.begin_frame(input);
        Button::new("hov").width(100.0).height(40.0).show(&mut ctx);
        ctx.end_frame(800.0, 600.0);
        assert_ne!(ctx.hovered_widget(), Some(WidgetId::new("hov")));
    }
}
