use arachne_math::{Color, Vec2};
use arachne_input::KeyCode;
use crate::context::{NodePaintCmd, UIContext, WidgetId, WidgetInteraction};
use crate::layout::{LayoutNode, Size};
use crate::widget::estimate_text_width;

#[derive(Default)]
pub(crate) struct TextInputState {
    pub cursor: usize,
    pub selection_start: Option<usize>,
}

/// Result of a text input interaction.
pub struct TextInputResult {
    pub changed: bool,
    pub submitted: bool,
}

/// A single-line text input widget.
pub struct TextInput {
    id_str: String,
    width: Option<f32>,
    height: Option<f32>,
    placeholder: String,
    disabled: bool,
}

impl TextInput {
    pub fn new(id: &str) -> Self {
        Self {
            id_str: id.to_string(),
            width: None,
            height: None,
            placeholder: String::new(),
            disabled: false,
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
    pub fn placeholder(mut self, p: &str) -> Self {
        self.placeholder = p.to_string();
        self
    }
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    pub fn show(&self, ctx: &mut UIContext, text: &mut String) -> TextInputResult {
        let id = WidgetId::new(&self.id_str);

        let font_size = ctx.theme().default.font_size;
        let padding = ctx.theme().default.padding;

        let layout = LayoutNode {
            width: Size::Fixed(self.width.unwrap_or(200.0)),
            height: Size::Fixed(self.height.unwrap_or(font_size + padding.vertical())),
            ..Default::default()
        };

        let node_id = ctx.add_widget(id, layout, !self.disabled);

        let interaction = if self.disabled {
            WidgetInteraction::default()
        } else {
            ctx.get_interaction(id)
        };

        // Gather input data before borrowing widget_data
        let keys = ctx.input().keys_just_pressed.clone();
        let chars = ctx.input().text_input.clone();
        let shift = ctx.input().shift_held();
        let ctrl = ctx.input().ctrl_held();
        let clipboard_text = ctx.clipboard().to_string();

        // Get persistent state (take to release borrow)
        let mut state: TextInputState =
            std::mem::take(ctx.get_widget_data::<TextInputState>(id));

        state.cursor = state.cursor.min(text.len());
        if let Some(sel) = state.selection_start {
            if sel > text.len() {
                state.selection_start = Some(text.len());
            }
        }

        let mut changed = false;
        let mut submitted = false;
        let mut clipboard_to_set: Option<String> = None;

        if interaction.focused && !self.disabled {
            // Text input
            for &ch in &chars {
                if ch >= ' ' {
                    delete_selection(&mut state, text);
                    text.insert(state.cursor, ch);
                    state.cursor += 1;
                    changed = true;
                }
            }

            // Key handling
            for &key in &keys {
                match key {
                    KeyCode::Left => {
                        if !shift {
                            state.selection_start = None;
                        } else if state.selection_start.is_none() {
                            state.selection_start = Some(state.cursor);
                        }
                        if state.cursor > 0 {
                            state.cursor -= 1;
                        }
                    }
                    KeyCode::Right => {
                        if !shift {
                            state.selection_start = None;
                        } else if state.selection_start.is_none() {
                            state.selection_start = Some(state.cursor);
                        }
                        if state.cursor < text.len() {
                            state.cursor += 1;
                        }
                    }
                    KeyCode::Home => {
                        if !shift {
                            state.selection_start = None;
                        } else if state.selection_start.is_none() {
                            state.selection_start = Some(state.cursor);
                        }
                        state.cursor = 0;
                    }
                    KeyCode::End => {
                        if !shift {
                            state.selection_start = None;
                        } else if state.selection_start.is_none() {
                            state.selection_start = Some(state.cursor);
                        }
                        state.cursor = text.len();
                    }
                    KeyCode::Backspace => {
                        if state.selection_start.is_some() {
                            delete_selection(&mut state, text);
                            changed = true;
                        } else if state.cursor > 0 {
                            state.cursor -= 1;
                            text.remove(state.cursor);
                            changed = true;
                        }
                    }
                    KeyCode::Delete => {
                        if state.selection_start.is_some() {
                            delete_selection(&mut state, text);
                            changed = true;
                        } else if state.cursor < text.len() {
                            text.remove(state.cursor);
                            changed = true;
                        }
                    }
                    KeyCode::Enter => {
                        submitted = true;
                    }
                    KeyCode::A if ctrl => {
                        state.selection_start = Some(0);
                        state.cursor = text.len();
                    }
                    KeyCode::C if ctrl => {
                        if let Some(sel) = state.selection_start {
                            let (s, e) = ordered(sel, state.cursor);
                            clipboard_to_set = Some(text[s..e].to_string());
                        }
                    }
                    KeyCode::X if ctrl => {
                        if let Some(sel) = state.selection_start {
                            let (s, e) = ordered(sel, state.cursor);
                            clipboard_to_set = Some(text[s..e].to_string());
                            delete_selection(&mut state, text);
                            changed = true;
                        }
                    }
                    KeyCode::V if ctrl => {
                        delete_selection(&mut state, text);
                        text.insert_str(state.cursor, &clipboard_text);
                        state.cursor += clipboard_text.len();
                        changed = true;
                    }
                    _ => {}
                }
            }
        }

        // Write state back
        *ctx.get_widget_data::<TextInputState>(id) = state;

        // Set clipboard if needed
        if let Some(cb) = clipboard_to_set {
            ctx.set_clipboard(&cb);
        }

        // Read state for cursor position (borrow again)
        let cursor_pos = ctx.get_widget_data::<TextInputState>(id).cursor;

        // Paint
        let style = ctx.style_for_state(self.disabled, &interaction);

        ctx.add_paint_cmd(
            node_id,
            NodePaintCmd::BorderedRect {
                fill: style.background_color,
                border_color: if interaction.focused {
                    Color::new(0.4, 0.6, 0.9, 1.0)
                } else {
                    style.border_color
                },
                border_width: style.border_width,
                border_radius: style.border_radius,
            },
        );

        let display_text = if text.is_empty() && !interaction.focused {
            self.placeholder.clone()
        } else {
            text.clone()
        };

        let text_color = if text.is_empty() && !interaction.focused {
            Color::new(0.5, 0.5, 0.5, 1.0)
        } else {
            style.text_color
        };

        ctx.add_paint_cmd(
            node_id,
            NodePaintCmd::TextOffset {
                offset: Vec2::new(style.padding.left, style.padding.top),
                text: display_text,
                color: text_color,
                font_size: style.font_size,
            },
        );

        // Cursor
        if interaction.focused && !self.disabled {
            let cursor_x =
                estimate_text_width(&text[..cursor_pos.min(text.len())], style.font_size)
                    + style.padding.left;
            ctx.add_paint_cmd(
                node_id,
                NodePaintCmd::FilledRectCustom {
                    rect_offset: Vec2::new(cursor_x, style.padding.top),
                    rect_size: Vec2::new(2.0, style.font_size),
                    color: style.text_color,
                    border_radius: 0.0,
                },
            );
        }

        TextInputResult { changed, submitted }
    }
}

fn delete_selection(state: &mut TextInputState, text: &mut String) {
    if let Some(sel) = state.selection_start.take() {
        let (s, e) = ordered(sel, state.cursor);
        text.drain(s..e);
        state.cursor = s;
    }
}

fn ordered(a: usize, b: usize) -> (usize, usize) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::InputState;
    use arachne_math::Vec2;

    fn focus_text_input(ctx: &mut UIContext) {
        // Frame to establish layout
        ctx.begin_frame(InputState::default());
        let mut t = String::new();
        TextInput::new("ti").width(200.0).show(ctx, &mut t);
        ctx.end_frame(800.0, 600.0);

        // Click to focus
        let input = InputState {
            mouse_pos: Vec2::new(100.0, 16.0),
            mouse_just_pressed: true,
            mouse_just_released: true,
            ..Default::default()
        };
        ctx.begin_frame(input);
        TextInput::new("ti").width(200.0).show(ctx, &mut t);
        ctx.end_frame(800.0, 600.0);
    }

    #[test]
    fn type_hello() {
        let mut ctx = UIContext::new();
        let mut text = String::new();

        focus_text_input(&mut ctx);

        // Type "hello"
        let input = InputState {
            text_input: vec!['h', 'e', 'l', 'l', 'o'],
            ..Default::default()
        };
        ctx.begin_frame(input);
        let result = TextInput::new("ti").width(200.0).show(&mut ctx, &mut text);
        ctx.end_frame(800.0, 600.0);

        assert_eq!(text, "hello");
        assert!(result.changed);
    }

    #[test]
    fn select_all_and_delete() {
        let mut ctx = UIContext::new();
        let _text = "hello".to_string();

        focus_text_input(&mut ctx);

        // Put text in (simulate typing)
        let input = InputState {
            text_input: "hello".chars().collect(),
            ..Default::default()
        };
        ctx.begin_frame(input);
        let mut t2 = String::new();
        TextInput::new("ti").width(200.0).show(&mut ctx, &mut t2);
        ctx.end_frame(800.0, 600.0);

        // Select all (Ctrl+A)
        let input = InputState {
            keys_just_pressed: vec![KeyCode::A],
            keys_held: vec![KeyCode::LeftCtrl],
            ..Default::default()
        };
        ctx.begin_frame(input);
        TextInput::new("ti").width(200.0).show(&mut ctx, &mut t2);
        ctx.end_frame(800.0, 600.0);

        // Delete (Backspace)
        let input = InputState {
            keys_just_pressed: vec![KeyCode::Backspace],
            ..Default::default()
        };
        ctx.begin_frame(input);
        TextInput::new("ti").width(200.0).show(&mut ctx, &mut t2);
        ctx.end_frame(800.0, 600.0);

        assert_eq!(t2, "");
    }

    #[test]
    fn arrow_keys_move_cursor() {
        let mut ctx = UIContext::new();
        let mut text = String::new();

        focus_text_input(&mut ctx);

        // Type "abc"
        let input = InputState {
            text_input: vec!['a', 'b', 'c'],
            ..Default::default()
        };
        ctx.begin_frame(input);
        TextInput::new("ti").width(200.0).show(&mut ctx, &mut text);
        ctx.end_frame(800.0, 600.0);
        assert_eq!(text, "abc");

        // cursor should be at 3 (end). Press Left twice.
        let input = InputState {
            keys_just_pressed: vec![KeyCode::Left, KeyCode::Left],
            ..Default::default()
        };
        ctx.begin_frame(input);
        TextInput::new("ti").width(200.0).show(&mut ctx, &mut text);
        ctx.end_frame(800.0, 600.0);

        // Cursor at 1. Type 'X'
        let input = InputState {
            text_input: vec!['X'],
            ..Default::default()
        };
        ctx.begin_frame(input);
        TextInput::new("ti").width(200.0).show(&mut ctx, &mut text);
        ctx.end_frame(800.0, 600.0);

        assert_eq!(text, "aXbc");
    }

    #[test]
    fn disabled_ignores_input() {
        let mut ctx = UIContext::new();
        let mut text = String::new();

        ctx.begin_frame(InputState::default());
        TextInput::new("tid").width(200.0).disabled(true).show(&mut ctx, &mut text);
        ctx.end_frame(800.0, 600.0);

        let input = InputState {
            mouse_pos: Vec2::new(100.0, 16.0),
            mouse_just_pressed: true,
            mouse_just_released: true,
            text_input: vec!['x'],
            ..Default::default()
        };
        ctx.begin_frame(input);
        TextInput::new("tid").width(200.0).disabled(true).show(&mut ctx, &mut text);
        ctx.end_frame(800.0, 600.0);

        assert_eq!(text, "");
    }
}
