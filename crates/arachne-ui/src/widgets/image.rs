use crate::context::{NodePaintCmd, UIContext, WidgetId};
use crate::layout::{LayoutNode, Size};
use crate::render::{ImageSizing, TextureHandle};

/// An image display widget.
pub struct ImageWidget {
    id_str: String,
    texture: TextureHandle,
    sizing: ImageSizing,
    width: Option<f32>,
    height: Option<f32>,
}

impl ImageWidget {
    pub fn new(id: &str, texture: TextureHandle) -> Self {
        Self {
            id_str: id.to_string(),
            texture,
            sizing: ImageSizing::Contain,
            width: None,
            height: None,
        }
    }

    pub fn sizing(mut self, s: ImageSizing) -> Self {
        self.sizing = s;
        self
    }
    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }
    pub fn height(mut self, h: f32) -> Self {
        self.height = Some(h);
        self
    }

    pub fn show(&self, ctx: &mut UIContext) {
        let id = WidgetId::new(&self.id_str);

        let layout = LayoutNode {
            width: self.width.map_or(Size::Auto, Size::Fixed),
            height: self.height.map_or(Size::Auto, Size::Fixed),
            ..Default::default()
        };

        let node_id = ctx.add_widget(id, layout, false);

        ctx.add_paint_cmd(
            node_id,
            NodePaintCmd::Image {
                texture: self.texture,
                sizing: self.sizing,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::InputState;
    use crate::render::DrawCommand;

    #[test]
    fn image_generates_draw_command() {
        let mut ctx = UIContext::new();

        ctx.begin_frame(InputState::default());
        ImageWidget::new("img", TextureHandle(42))
            .width(100.0)
            .height(100.0)
            .show(&mut ctx);
        ctx.end_frame(800.0, 600.0);

        let has_image = ctx.draw_commands().iter().any(|cmd| {
            matches!(cmd, DrawCommand::Image { texture, .. } if texture.0 == 42)
        });
        assert!(has_image);
    }

    #[test]
    fn image_sizing_modes() {
        for sizing in [
            ImageSizing::Contain,
            ImageSizing::Cover,
            ImageSizing::Fill,
            ImageSizing::None,
        ] {
            let mut ctx = UIContext::new();
            ctx.begin_frame(InputState::default());
            ImageWidget::new("img", TextureHandle(1))
                .sizing(sizing)
                .width(50.0)
                .height(50.0)
                .show(&mut ctx);
            ctx.end_frame(200.0, 200.0);

            let found = ctx.draw_commands().iter().any(|cmd| {
                matches!(cmd, DrawCommand::Image { sizing: s, .. } if *s == sizing)
            });
            assert!(found, "Expected {:?} sizing in draw commands", sizing);
        }
    }
}
