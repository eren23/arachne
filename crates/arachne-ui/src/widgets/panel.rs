use arachne_math::Vec2;
use crate::context::{NodePaintCmd, UIContext, WidgetId, WidgetInteraction};
use crate::layout::{FlexDirection, LayoutNode, Overflow, Size};

#[derive(Default, Clone)]
pub(crate) struct PanelState {
    pub scroll_offset: Vec2,
}

/// A scrollable container widget.
pub struct Panel {
    id_str: String,
    width: f32,
    height: f32,
    direction: FlexDirection,
}

impl Panel {
    pub fn new(id: &str, width: f32, height: f32) -> Self {
        Self {
            id_str: id.to_string(),
            width,
            height,
            direction: FlexDirection::Column,
        }
    }

    pub fn direction(mut self, d: FlexDirection) -> Self {
        self.direction = d;
        self
    }

    /// Begin the scrollable panel. Call [`end`] when done adding children.
    pub fn begin(&self, ctx: &mut UIContext) -> PanelHandle {
        let id = WidgetId::new(&self.id_str);

        let layout = LayoutNode {
            width: Size::Fixed(self.width),
            height: Size::Fixed(self.height),
            overflow: Overflow::Hidden,
            flex_direction: self.direction,
            ..Default::default()
        };

        let node_id = ctx.add_widget(id, layout, false);

        // Interaction (for scroll)
        let interaction = ctx.get_interaction(id);

        // Handle scroll input
        let scroll_delta = ctx.input().scroll_delta;
        let hovered = interaction.hovered;

        let state: PanelState =
            std::mem::take(ctx.get_widget_data::<PanelState>(id));
        let mut scroll = state.scroll_offset;

        if hovered {
            scroll.y = (scroll.y - scroll_delta.y).max(0.0);
            scroll.x = (scroll.x - scroll_delta.x).max(0.0);
        }

        *ctx.get_widget_data::<PanelState>(id) = PanelState {
            scroll_offset: scroll,
        };

        ctx.set_scroll_offset(node_id, scroll);

        // Background
        let style = ctx.style_for_state(false, &WidgetInteraction::default());
        ctx.add_paint_cmd(
            node_id,
            NodePaintCmd::FilledRect {
                color: style.background_color,
                border_radius: 0.0,
            },
        );

        ctx.push_parent(node_id);

        PanelHandle {
            id,
            node_id,
        }
    }

    /// End the scrollable panel.
    pub fn end(&self, ctx: &mut UIContext) {
        ctx.pop_parent();
    }

    /// Get the current scroll offset.
    pub fn scroll_offset(&self, ctx: &mut UIContext) -> Vec2 {
        let id = WidgetId::new(&self.id_str);
        ctx.get_widget_data::<PanelState>(id).scroll_offset
    }
}

/// Handle returned by [`Panel::begin`].
pub struct PanelHandle {
    pub id: WidgetId,
    pub node_id: crate::layout::NodeId,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::InputState;
    use crate::widgets::label::Label;
    use arachne_math::Vec2;

    #[test]
    fn panel_scroll_changes_visible_region() {
        let mut ctx = UIContext::new();

        // Frame 1: Panel with content taller than viewport
        ctx.begin_frame(InputState::default());
        let panel = Panel::new("panel", 200.0, 100.0);
        panel.begin(&mut ctx);
        for i in 0..10 {
            Label::new(&format!("Item {}", i))
                .id(&format!("item_{}", i))
                .show(&mut ctx);
        }
        panel.end(&mut ctx);
        ctx.end_frame(800.0, 600.0);

        // Check that items have clip rects (from overflow:hidden)
        let clipped_count = ctx
            .draw_commands()
            .iter()
            .filter(|cmd| match cmd {
                crate::render::DrawCommand::Text { clip, .. } => clip.is_some(),
                _ => false,
            })
            .count();
        assert!(clipped_count > 0, "Panel children should be clipped");

        // Frame 2: Scroll down
        let input = InputState {
            mouse_pos: Vec2::new(100.0, 50.0), // inside panel
            scroll_delta: Vec2::new(0.0, -60.0), // scroll down
            ..Default::default()
        };
        ctx.begin_frame(input);
        let panel = Panel::new("panel", 200.0, 100.0);
        panel.begin(&mut ctx);
        for i in 0..10 {
            Label::new(&format!("Item {}", i))
                .id(&format!("item_{}", i))
                .show(&mut ctx);
        }
        panel.end(&mut ctx);
        ctx.end_frame(800.0, 600.0);

        // The scroll offset should be > 0
        let offset = Panel::new("panel", 200.0, 100.0).scroll_offset(&mut ctx);
        assert!(
            offset.y > 0.0,
            "Scroll offset should be positive after scrolling, got {}",
            offset.y
        );

        // Check that draw commands have shifted positions
        let text_positions: Vec<f32> = ctx
            .draw_commands()
            .iter()
            .filter_map(|cmd| match cmd {
                crate::render::DrawCommand::Text { position, .. } => Some(position.y),
                _ => None,
            })
            .collect();

        // Some items should have negative y positions (scrolled above viewport)
        let has_shifted = text_positions.iter().any(|&y| y < 0.0);
        assert!(
            has_shifted,
            "After scrolling, some items should be above viewport. Positions: {:?}",
            text_positions
        );
    }
}
