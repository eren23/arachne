use arachne_math::{Color, Vec2};
use crate::context::{NodePaintCmd, UIContext, WidgetId, WidgetInteraction};
use crate::layout::{LayoutNode, Size};

/// Slider orientation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SliderOrientation {
    Horizontal,
    Vertical,
}

#[derive(Default)]
pub(crate) struct SliderState {
    pub dragging: bool,
}

/// A draggable slider widget.
pub struct Slider {
    id_str: String,
    min: f32,
    max: f32,
    step: Option<f32>,
    orientation: SliderOrientation,
    width: Option<f32>,
    height: Option<f32>,
    disabled: bool,
}

impl Slider {
    pub fn new(id: &str, min: f32, max: f32) -> Self {
        Self {
            id_str: id.to_string(),
            min,
            max,
            step: None,
            orientation: SliderOrientation::Horizontal,
            width: None,
            height: None,
            disabled: false,
        }
    }

    pub fn step(mut self, s: f32) -> Self {
        self.step = Some(s);
        self
    }
    pub fn vertical(mut self) -> Self {
        self.orientation = SliderOrientation::Vertical;
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
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    /// Show the slider. Returns `true` if the value changed.
    pub fn show(&self, ctx: &mut UIContext, value: &mut f32) -> bool {
        let id = WidgetId::new(&self.id_str);

        let (default_w, default_h) = match self.orientation {
            SliderOrientation::Horizontal => (200.0, 24.0),
            SliderOrientation::Vertical => (24.0, 200.0),
        };

        let w = self.width.unwrap_or(default_w);
        let h = self.height.unwrap_or(default_h);

        let layout = LayoutNode {
            width: Size::Fixed(w),
            height: Size::Fixed(h),
            ..Default::default()
        };

        let node_id = ctx.add_widget(id, layout, !self.disabled);

        let interaction = if self.disabled {
            WidgetInteraction::default()
        } else {
            ctx.get_interaction(id)
        };

        // Drag state
        let mouse_down = ctx.input().mouse_down;
        let mouse_just_pressed = ctx.input().mouse_just_pressed;
        let mouse_pos = ctx.input().mouse_pos;

        let state = ctx.get_widget_data::<SliderState>(id);

        if (interaction.clicked || interaction.active) && mouse_just_pressed {
            state.dragging = true;
        }
        if !mouse_down {
            state.dragging = false;
        }

        let dragging = state.dragging;

        let mut changed = false;

        if dragging && !self.disabled {
            if let Some(rect) = ctx.prev_rect(id) {
                let t = match self.orientation {
                    SliderOrientation::Horizontal => {
                        ((mouse_pos.x - rect.min.x) / rect.width()).clamp(0.0, 1.0)
                    }
                    SliderOrientation::Vertical => {
                        ((mouse_pos.y - rect.min.y) / rect.height()).clamp(0.0, 1.0)
                    }
                };

                let mut new_val = self.min + t * (self.max - self.min);

                if let Some(step) = self.step {
                    if step > 0.0 {
                        new_val = (((new_val - self.min) / step).round() * step + self.min)
                            .clamp(self.min, self.max);
                    }
                }

                if (*value - new_val).abs() > f32::EPSILON {
                    *value = new_val;
                    changed = true;
                }
            }
        }

        // Paint track
        let style = ctx.style_for_state(self.disabled, &interaction);

        ctx.add_paint_cmd(
            node_id,
            NodePaintCmd::FilledRect {
                color: Color::new(0.3, 0.3, 0.3, 1.0),
                border_radius: style.border_radius,
            },
        );

        // Paint knob
        let t = if (self.max - self.min).abs() > f32::EPSILON {
            ((*value - self.min) / (self.max - self.min)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let knob_size = 16.0_f32;
        let knob_color = if self.disabled {
            Color::new(0.5, 0.5, 0.5, 1.0)
        } else {
            Color::WHITE
        };

        match self.orientation {
            SliderOrientation::Horizontal => {
                let track_w = w - knob_size;
                let knob_x = t * track_w;
                let knob_y = (h - knob_size) / 2.0;
                ctx.add_paint_cmd(
                    node_id,
                    NodePaintCmd::FilledRectCustom {
                        rect_offset: Vec2::new(knob_x, knob_y),
                        rect_size: Vec2::new(knob_size, knob_size),
                        color: knob_color,
                        border_radius: knob_size / 2.0,
                    },
                );
            }
            SliderOrientation::Vertical => {
                let track_h = h - knob_size;
                let knob_y = t * track_h;
                let knob_x = (w - knob_size) / 2.0;
                ctx.add_paint_cmd(
                    node_id,
                    NodePaintCmd::FilledRectCustom {
                        rect_offset: Vec2::new(knob_x, knob_y),
                        rect_size: Vec2::new(knob_size, knob_size),
                        color: knob_color,
                        border_radius: knob_size / 2.0,
                    },
                );
            }
        }

        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::InputState;
    use arachne_math::Vec2;

    #[test]
    fn slider_drag_to_midpoint() {
        let mut ctx = UIContext::new();
        let mut value = 0.0_f32;

        // Frame 1: show slider
        ctx.begin_frame(InputState::default());
        Slider::new("s", 0.0, 100.0).width(200.0).show(&mut ctx, &mut value);
        ctx.end_frame(800.0, 600.0);

        // Frame 2: press at midpoint (x=100 of 200px slider starting at x=0)
        let input = InputState {
            mouse_pos: Vec2::new(100.0, 12.0),
            mouse_down: true,
            mouse_just_pressed: true,
            ..Default::default()
        };
        ctx.begin_frame(input);
        Slider::new("s", 0.0, 100.0).width(200.0).show(&mut ctx, &mut value);
        ctx.end_frame(800.0, 600.0);

        assert!(
            (value - 50.0).abs() < 1.0,
            "Expected ~50.0, got {}",
            value
        );
    }

    #[test]
    fn slider_step_snapping() {
        let mut ctx = UIContext::new();
        let mut value = 0.0_f32;

        // Frame 1
        ctx.begin_frame(InputState::default());
        Slider::new("ss", 0.0, 100.0)
            .step(25.0)
            .width(200.0)
            .show(&mut ctx, &mut value);
        ctx.end_frame(800.0, 600.0);

        // Frame 2: drag to ~30% (x=60 of 200px)
        let input = InputState {
            mouse_pos: Vec2::new(60.0, 12.0),
            mouse_down: true,
            mouse_just_pressed: true,
            ..Default::default()
        };
        ctx.begin_frame(input);
        Slider::new("ss", 0.0, 100.0)
            .step(25.0)
            .width(200.0)
            .show(&mut ctx, &mut value);
        ctx.end_frame(800.0, 600.0);

        // 60/200 = 0.3 -> 30.0, snapped to nearest 25 = 25.0
        assert!(
            (value - 25.0).abs() < 1.0,
            "Expected 25.0 (step snap), got {}",
            value
        );
    }

    #[test]
    fn slider_disabled_no_change() {
        let mut ctx = UIContext::new();
        let mut value = 10.0_f32;

        ctx.begin_frame(InputState::default());
        Slider::new("sd", 0.0, 100.0)
            .disabled(true)
            .width(200.0)
            .show(&mut ctx, &mut value);
        ctx.end_frame(800.0, 600.0);

        let input = InputState {
            mouse_pos: Vec2::new(100.0, 12.0),
            mouse_down: true,
            mouse_just_pressed: true,
            ..Default::default()
        };
        ctx.begin_frame(input);
        Slider::new("sd", 0.0, 100.0)
            .disabled(true)
            .width(200.0)
            .show(&mut ctx, &mut value);
        ctx.end_frame(800.0, 600.0);

        assert!((value - 10.0).abs() < f32::EPSILON);
    }
}
