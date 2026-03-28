use arachne_math::{Color, Vec2};
use crate::context::{NodePaintCmd, UIContext, WidgetId, WidgetInteraction};
use crate::layout::{LayoutNode, Size};

/// Tri-state check state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CheckState {
    Checked,
    Unchecked,
    Indeterminate,
}

/// A checkbox or toggle-switch widget.
pub struct Checkbox {
    id_str: String,
    label: Option<String>,
    toggle_style: bool,
    disabled: bool,
    width: Option<f32>,
    height: Option<f32>,
}

impl Checkbox {
    pub fn new(id: &str) -> Self {
        Self {
            id_str: id.to_string(),
            label: None,
            toggle_style: false,
            disabled: false,
            width: None,
            height: None,
        }
    }

    pub fn label(mut self, l: &str) -> Self {
        self.label = Some(l.to_string());
        self
    }
    pub fn toggle(mut self) -> Self {
        self.toggle_style = true;
        self
    }
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
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

    /// Show the checkbox. Returns `true` if it was toggled this frame.
    pub fn show(&self, ctx: &mut UIContext, checked: &mut bool) -> bool {
        let id = WidgetId::new(&self.id_str);
        let size = if self.toggle_style { 40.0 } else { 24.0 };

        let layout = LayoutNode {
            width: Size::Fixed(self.width.unwrap_or(size)),
            height: Size::Fixed(self.height.unwrap_or(if self.toggle_style { 24.0 } else { size })),
            ..Default::default()
        };

        let node_id = ctx.add_widget(id, layout, !self.disabled);

        let interaction = if self.disabled {
            WidgetInteraction::default()
        } else {
            ctx.get_interaction(id)
        };

        if interaction.clicked {
            *checked = !*checked;
        }

        let style = ctx.style_for_state(self.disabled, &interaction);
        let w = self.width.unwrap_or(size);
        let h = self.height.unwrap_or(if self.toggle_style { 24.0 } else { size });

        if self.toggle_style {
            let bg = if *checked {
                Color::new(0.2, 0.6, 0.2, 1.0)
            } else {
                Color::new(0.4, 0.4, 0.4, 1.0)
            };
            ctx.add_paint_cmd(
                node_id,
                NodePaintCmd::FilledRect {
                    color: if self.disabled {
                        Color::new(0.3, 0.3, 0.3, 1.0)
                    } else {
                        bg
                    },
                    border_radius: h / 2.0,
                },
            );

            let knob_size = h * 0.7;
            let knob_x = if *checked {
                w - knob_size - 2.0
            } else {
                2.0
            };
            let knob_y = (h - knob_size) / 2.0;
            ctx.add_paint_cmd(
                node_id,
                NodePaintCmd::FilledRectCustom {
                    rect_offset: Vec2::new(knob_x, knob_y),
                    rect_size: Vec2::new(knob_size, knob_size),
                    color: Color::WHITE,
                    border_radius: knob_size / 2.0,
                },
            );
        } else {
            ctx.add_paint_cmd(
                node_id,
                NodePaintCmd::BorderedRect {
                    fill: style.background_color,
                    border_color: style.border_color,
                    border_width: style.border_width,
                    border_radius: 3.0,
                },
            );

            if *checked {
                let inset = 4.0;
                ctx.add_paint_cmd(
                    node_id,
                    NodePaintCmd::FilledRectCustom {
                        rect_offset: Vec2::new(inset, inset),
                        rect_size: Vec2::new(w - inset * 2.0, h - inset * 2.0),
                        color: Color::new(0.3, 0.7, 0.3, 1.0),
                        border_radius: 2.0,
                    },
                );
            }
        }

        interaction.clicked
    }

    /// Show as a tri-state checkbox. Returns `true` if toggled.
    pub fn show_tristate(&self, ctx: &mut UIContext, state: &mut CheckState) -> bool {
        let id = WidgetId::new(&self.id_str);
        let size = 24.0_f32;

        let layout = LayoutNode {
            width: Size::Fixed(self.width.unwrap_or(size)),
            height: Size::Fixed(self.height.unwrap_or(size)),
            ..Default::default()
        };

        let node_id = ctx.add_widget(id, layout, !self.disabled);

        let interaction = if self.disabled {
            WidgetInteraction::default()
        } else {
            ctx.get_interaction(id)
        };

        if interaction.clicked {
            *state = match *state {
                CheckState::Unchecked => CheckState::Checked,
                CheckState::Checked => CheckState::Unchecked,
                CheckState::Indeterminate => CheckState::Checked,
            };
        }

        let style = ctx.style_for_state(self.disabled, &interaction);
        let w = self.width.unwrap_or(size);
        let h = self.height.unwrap_or(size);

        ctx.add_paint_cmd(
            node_id,
            NodePaintCmd::BorderedRect {
                fill: style.background_color,
                border_color: style.border_color,
                border_width: style.border_width,
                border_radius: 3.0,
            },
        );

        match state {
            CheckState::Checked => {
                let inset = 4.0;
                ctx.add_paint_cmd(
                    node_id,
                    NodePaintCmd::FilledRectCustom {
                        rect_offset: Vec2::new(inset, inset),
                        rect_size: Vec2::new(w - inset * 2.0, h - inset * 2.0),
                        color: Color::new(0.3, 0.7, 0.3, 1.0),
                        border_radius: 2.0,
                    },
                );
            }
            CheckState::Indeterminate => {
                let inset = 6.0;
                let bar_h = 4.0;
                ctx.add_paint_cmd(
                    node_id,
                    NodePaintCmd::FilledRectCustom {
                        rect_offset: Vec2::new(inset, (h - bar_h) / 2.0),
                        rect_size: Vec2::new(w - inset * 2.0, bar_h),
                        color: Color::new(0.7, 0.7, 0.3, 1.0),
                        border_radius: 1.0,
                    },
                );
            }
            CheckState::Unchecked => {}
        }

        interaction.clicked
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::InputState;
    use arachne_math::Vec2;

    #[test]
    fn checkbox_click_toggles() {
        let mut ctx = UIContext::new();
        let mut checked = false;

        // Frame 1
        ctx.begin_frame(InputState::default());
        Checkbox::new("cb").show(&mut ctx, &mut checked);
        ctx.end_frame(800.0, 600.0);
        assert!(!checked);

        // Frame 2: click
        let input = InputState {
            mouse_pos: Vec2::new(12.0, 12.0),
            mouse_just_pressed: true,
            mouse_just_released: true,
            ..Default::default()
        };
        ctx.begin_frame(input);
        Checkbox::new("cb").show(&mut ctx, &mut checked);
        ctx.end_frame(800.0, 600.0);
        assert!(checked);

        // Frame 3: click again
        let input = InputState {
            mouse_pos: Vec2::new(12.0, 12.0),
            mouse_just_pressed: true,
            mouse_just_released: true,
            ..Default::default()
        };
        ctx.begin_frame(input);
        Checkbox::new("cb").show(&mut ctx, &mut checked);
        ctx.end_frame(800.0, 600.0);
        assert!(!checked);
    }

    #[test]
    fn checkbox_disabled_no_toggle() {
        let mut ctx = UIContext::new();
        let mut checked = false;

        ctx.begin_frame(InputState::default());
        Checkbox::new("cbd").disabled(true).show(&mut ctx, &mut checked);
        ctx.end_frame(800.0, 600.0);

        let input = InputState {
            mouse_pos: Vec2::new(12.0, 12.0),
            mouse_just_pressed: true,
            mouse_just_released: true,
            ..Default::default()
        };
        ctx.begin_frame(input);
        Checkbox::new("cbd").disabled(true).show(&mut ctx, &mut checked);
        ctx.end_frame(800.0, 600.0);
        assert!(!checked);
    }
}
