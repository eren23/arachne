use arachne_math::Color;
use crate::context::{NodePaintCmd, UIContext, WidgetId, WidgetInteraction};
use crate::layout::{LayoutNode, Size};

#[derive(Default)]
pub(crate) struct DropdownState {
    pub is_open: bool,
    pub filter: String,
}

/// A dropdown/select widget.
pub struct Dropdown {
    id_str: String,
    options: Vec<String>,
    width: Option<f32>,
    disabled: bool,
    searchable: bool,
}

impl Dropdown {
    pub fn new(id: &str, options: Vec<String>) -> Self {
        Self {
            id_str: id.to_string(),
            options,
            width: None,
            disabled: false,
            searchable: false,
        }
    }

    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }
    pub fn searchable(mut self, s: bool) -> Self {
        self.searchable = s;
        self
    }

    /// Show the dropdown. Returns `true` if selection changed.
    pub fn show(&self, ctx: &mut UIContext, selected: &mut Option<usize>) -> bool {
        let id = WidgetId::new(&self.id_str);
        let w = self.width.unwrap_or(200.0);

        let layout = LayoutNode {
            width: Size::Fixed(w),
            height: Size::Fixed(32.0),
            ..Default::default()
        };

        let node_id = ctx.add_widget(id, layout, !self.disabled);

        let interaction = if self.disabled {
            WidgetInteraction::default()
        } else {
            ctx.get_interaction(id)
        };

        let mouse_just_pressed = ctx.input().mouse_just_pressed;

        let mut state: DropdownState =
            std::mem::take(ctx.get_widget_data::<DropdownState>(id));

        // Toggle on button click
        if interaction.clicked {
            state.is_open = !state.is_open;
        }

        let mut changed = false;

        if state.is_open {
            // Filter options if searchable
            let filtered: Vec<(usize, &String)> = if self.searchable && !state.filter.is_empty() {
                self.options
                    .iter()
                    .enumerate()
                    .filter(|(_, o)| {
                        o.to_lowercase()
                            .contains(&state.filter.to_lowercase())
                    })
                    .collect()
            } else {
                self.options.iter().enumerate().collect()
            };

            let mut any_option_hovered = false;

            for &(orig_idx, option) in &filtered {
                let opt_id = WidgetId::with_parent(id, &format!("opt_{}", orig_idx));
                let opt_layout = LayoutNode {
                    width: Size::Fixed(w),
                    height: Size::Fixed(28.0),
                    ..Default::default()
                };
                let opt_node = ctx.add_widget(opt_id, opt_layout, true);
                let opt_inter = ctx.get_interaction(opt_id);

                if opt_inter.hovered {
                    any_option_hovered = true;
                }

                if opt_inter.clicked {
                    *selected = Some(orig_idx);
                    state.is_open = false;
                    changed = true;
                }

                let bg = if opt_inter.hovered {
                    Color::new(0.3, 0.3, 0.4, 1.0)
                } else {
                    Color::new(0.2, 0.2, 0.2, 1.0)
                };
                ctx.add_paint_cmd(
                    opt_node,
                    NodePaintCmd::FilledRect {
                        color: bg,
                        border_radius: 0.0,
                    },
                );
                ctx.add_paint_cmd(
                    opt_node,
                    NodePaintCmd::Text {
                        text: option.clone(),
                        color: Color::WHITE,
                        font_size: 14.0,
                    },
                );
            }

            // Close on click outside
            if state.is_open
                && mouse_just_pressed
                && !interaction.hovered
                && !any_option_hovered
            {
                // Check prev rects for options
                let on_option = filtered.iter().any(|&(orig_idx, _)| {
                    let opt_id = WidgetId::with_parent(id, &format!("opt_{}", orig_idx));
                    ctx.prev_rect(opt_id)
                        .map_or(false, |r| r.contains(ctx.input().mouse_pos))
                });
                if !on_option {
                    state.is_open = false;
                }
            }
        }

        // Write state back
        *ctx.get_widget_data::<DropdownState>(id) = state;

        // Paint button
        let style = ctx.style_for_state(self.disabled, &interaction);
        let display = selected
            .and_then(|i| self.options.get(i))
            .cloned()
            .unwrap_or_else(|| "Select...".to_string());

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
                text: display,
                color: style.text_color,
                font_size: style.font_size,
            },
        );

        changed
    }

    /// Whether the dropdown is currently open.
    pub fn is_open(&self, ctx: &mut UIContext) -> bool {
        let id = WidgetId::new(&self.id_str);
        ctx.get_widget_data::<DropdownState>(id).is_open
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::InputState;
    use arachne_math::Vec2;

    #[test]
    fn dropdown_open_select_close() {
        let options = vec![
            "Apple".to_string(),
            "Banana".to_string(),
            "Cherry".to_string(),
        ];
        let mut ctx = UIContext::new();
        let mut selected: Option<usize> = None;

        // Frame 1: show dropdown (closed)
        ctx.begin_frame(InputState::default());
        Dropdown::new("dd", options.clone()).show(&mut ctx, &mut selected);
        ctx.end_frame(800.0, 600.0);
        assert!(!Dropdown::new("dd", options.clone()).is_open(&mut ctx));
        assert_eq!(selected, None);

        // Frame 2: click to open
        let input = InputState {
            mouse_pos: Vec2::new(100.0, 16.0),
            mouse_just_pressed: true,
            mouse_just_released: true,
            ..Default::default()
        };
        ctx.begin_frame(input);
        Dropdown::new("dd", options.clone()).show(&mut ctx, &mut selected);
        ctx.end_frame(800.0, 600.0);
        assert!(Dropdown::new("dd", options.clone()).is_open(&mut ctx));

        // Frame 3: click on "Banana" (second option)
        // Button: y=0..32, Apple: y=32..60, Banana: y=60..88
        // Center of Banana = 74
        let input = InputState {
            mouse_pos: Vec2::new(100.0, 74.0),
            mouse_just_pressed: true,
            mouse_just_released: true,
            ..Default::default()
        };
        ctx.begin_frame(input);
        let changed = Dropdown::new("dd", options.clone()).show(&mut ctx, &mut selected);
        ctx.end_frame(800.0, 600.0);

        assert!(changed);
        assert_eq!(selected, Some(1)); // Banana
        assert!(!Dropdown::new("dd", options.clone()).is_open(&mut ctx));
    }

    #[test]
    fn dropdown_disabled_no_open() {
        let options = vec!["A".to_string(), "B".to_string()];
        let mut ctx = UIContext::new();
        let mut selected: Option<usize> = None;

        ctx.begin_frame(InputState::default());
        Dropdown::new("ddd", options.clone())
            .disabled(true)
            .show(&mut ctx, &mut selected);
        ctx.end_frame(800.0, 600.0);

        let input = InputState {
            mouse_pos: Vec2::new(100.0, 16.0),
            mouse_just_pressed: true,
            mouse_just_released: true,
            ..Default::default()
        };
        ctx.begin_frame(input);
        Dropdown::new("ddd", options.clone())
            .disabled(true)
            .show(&mut ctx, &mut selected);
        ctx.end_frame(800.0, 600.0);

        assert!(!Dropdown::new("ddd", options).is_open(&mut ctx));
    }
}
