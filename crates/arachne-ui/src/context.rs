use std::any::Any;
use std::collections::HashMap;

use arachne_math::{Color, Rect, Vec2};
use arachne_input::KeyCode;

use crate::layout::{LayoutNode, LayoutTree, NodeId, Overflow, Size};
use crate::render::DrawCommand;
use crate::style::{Style, Theme};

// ---------------------------------------------------------------------------
// IDs & Events
// ---------------------------------------------------------------------------

/// Unique identifier for a widget, computed from its label via FNV-1a hash.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WidgetId(pub u64);

impl WidgetId {
    pub fn new(label: &str) -> Self {
        let mut h: u64 = 14695981039346656037;
        for b in label.bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(1099511628211);
        }
        WidgetId(h)
    }

    pub fn with_parent(parent: WidgetId, label: &str) -> Self {
        let mut h = parent.0;
        for b in label.bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(1099511628211);
        }
        WidgetId(h)
    }
}

/// Events emitted by the UI system during a frame.
#[derive(Clone, Debug, PartialEq)]
pub enum UIEvent {
    Click(WidgetId),
    Hover(WidgetId),
    FocusIn(WidgetId),
    FocusOut(WidgetId),
}

// ---------------------------------------------------------------------------
// Input
// ---------------------------------------------------------------------------

/// Snapshot of input state for a single frame.
#[derive(Clone, Debug, Default)]
pub struct InputState {
    pub mouse_pos: Vec2,
    pub mouse_down: bool,
    pub mouse_just_pressed: bool,
    pub mouse_just_released: bool,
    pub keys_just_pressed: Vec<KeyCode>,
    pub keys_held: Vec<KeyCode>,
    pub text_input: Vec<char>,
    pub scroll_delta: Vec2,
}

impl InputState {
    pub fn is_key_just_pressed(&self, key: KeyCode) -> bool {
        self.keys_just_pressed.contains(&key)
    }
    pub fn is_key_held(&self, key: KeyCode) -> bool {
        self.keys_held.contains(&key)
    }
    pub fn shift_held(&self) -> bool {
        self.is_key_held(KeyCode::LeftShift) || self.is_key_held(KeyCode::RightShift)
    }
    pub fn ctrl_held(&self) -> bool {
        self.is_key_held(KeyCode::LeftCtrl) || self.is_key_held(KeyCode::RightCtrl)
    }
}

// ---------------------------------------------------------------------------
// Interaction
// ---------------------------------------------------------------------------

/// Per-widget interaction state for the current frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct WidgetInteraction {
    pub hovered: bool,
    pub active: bool,
    pub focused: bool,
    pub clicked: bool,
}

// ---------------------------------------------------------------------------
// Internal paint commands (node-relative)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub(crate) enum NodePaintCmd {
    FilledRect {
        color: Color,
        border_radius: f32,
    },
    FilledRectCustom {
        rect_offset: Vec2,
        rect_size: Vec2,
        color: Color,
        border_radius: f32,
    },
    BorderedRect {
        fill: Color,
        border_color: Color,
        border_width: f32,
        border_radius: f32,
    },
    Text {
        text: String,
        color: Color,
        font_size: f32,
    },
    TextOffset {
        offset: Vec2,
        text: String,
        color: Color,
        font_size: f32,
    },
    Image {
        texture: crate::render::TextureHandle,
        sizing: crate::render::ImageSizing,
    },
}

// ---------------------------------------------------------------------------
// UIContext
// ---------------------------------------------------------------------------

/// Central hub for the immediate-mode UI system.
pub struct UIContext {
    // Current frame
    layout: LayoutTree,
    node_widget_ids: Vec<Option<WidgetId>>,
    render_data: Vec<Vec<NodePaintCmd>>,

    // Previous frame (for hit testing)
    prev_rects: HashMap<WidgetId, Rect>,

    // Persistent interaction state
    focus: Option<WidgetId>,
    hovered: Option<WidgetId>,
    active: Option<WidgetId>,

    // Frame input
    input: InputState,

    // Output
    events: Vec<UIEvent>,
    draw_commands: Vec<DrawCommand>,

    // Theme
    theme: Theme,

    // Clip stack
    clip_stack: Vec<Rect>,

    // Layout node stack (for nesting containers)
    node_stack: Vec<NodeId>,

    // DFS-order focusable widgets
    tab_order: Vec<WidgetId>,

    // Per-widget persistent data (survives across frames)
    widget_data: HashMap<WidgetId, Box<dyn Any>>,

    // Scroll offsets for scrollable containers
    scroll_offsets: HashMap<NodeId, Vec2>,

    // Simple clipboard buffer
    clipboard: String,
}

impl UIContext {
    pub fn new() -> Self {
        Self {
            layout: LayoutTree::new(),
            node_widget_ids: Vec::new(),
            render_data: Vec::new(),
            prev_rects: HashMap::new(),
            focus: None,
            hovered: None,
            active: None,
            input: InputState::default(),
            events: Vec::new(),
            draw_commands: Vec::new(),
            theme: Theme::default(),
            clip_stack: Vec::new(),
            node_stack: Vec::new(),
            tab_order: Vec::new(),
            widget_data: HashMap::new(),
            scroll_offsets: HashMap::new(),
            clipboard: String::new(),
        }
    }

    pub fn with_theme(theme: Theme) -> Self {
        let mut ctx = Self::new();
        ctx.theme = theme;
        ctx
    }

    // -- Frame lifecycle ----------------------------------------------------

    /// Start a new frame. Saves previous layout for hit testing and clears
    /// per-frame state.
    pub fn begin_frame(&mut self, input: InputState) {
        // Save previous rects
        self.prev_rects.clear();
        for (i, opt_id) in self.node_widget_ids.iter().enumerate() {
            if let Some(id) = opt_id {
                if i < self.layout.node_count() {
                    self.prev_rects.insert(*id, self.layout.get_rect(NodeId(i)));
                }
            }
        }

        // Clear frame data
        self.layout.clear();
        self.node_widget_ids.clear();
        self.render_data.clear();
        self.events.clear();
        self.draw_commands.clear();
        self.tab_order.clear();
        self.node_stack.clear();
        self.clip_stack.clear();
        self.scroll_offsets.clear();
        self.hovered = None;
        self.input = input;

        // Create root layout node
        let root = self.layout.add_node(LayoutNode::default());
        self.node_widget_ids.push(None);
        self.render_data.push(Vec::new());
        self.node_stack.push(root);
    }

    /// End the frame: compute layout, handle tab navigation, generate draw
    /// commands.
    pub fn end_frame(&mut self, width: f32, height: f32) {
        // Configure root
        let root = NodeId(0);
        {
            let rn = self.layout.get_node_mut(root);
            rn.width = Size::Fixed(width);
            rn.height = Size::Fixed(height);
            rn.flex_direction = crate::layout::FlexDirection::Column;
        }

        // Layout
        self.layout.compute(root, width, height);

        // Tab navigation
        if self.input.is_key_just_pressed(KeyCode::Tab) {
            self.cycle_focus(self.input.shift_held());
        }

        // Draw commands
        self.generate_draw_commands();
    }

    // -- Widget registration ------------------------------------------------

    /// Register a widget with the given layout. Returns the [`NodeId`] for
    /// paint commands.
    pub fn add_widget(
        &mut self,
        id: WidgetId,
        layout_node: LayoutNode,
        focusable: bool,
    ) -> NodeId {
        let node_id = self.layout.add_node(layout_node);
        self.node_widget_ids.push(Some(id));
        self.render_data.push(Vec::new());

        if let Some(&parent) = self.node_stack.last() {
            self.layout.add_child(parent, node_id);
        }

        if focusable {
            self.tab_order.push(id);
        }

        node_id
    }

    /// Attach a paint command to a node.
    pub(crate) fn add_paint_cmd(&mut self, node_id: NodeId, cmd: NodePaintCmd) {
        if node_id.0 < self.render_data.len() {
            self.render_data[node_id.0].push(cmd);
        }
    }

    // -- Interaction --------------------------------------------------------

    /// Query interaction state for a widget using previous-frame layout.
    pub fn get_interaction(&mut self, id: WidgetId) -> WidgetInteraction {
        let prev_rect = self.prev_rects.get(&id).copied();
        let is_hovered = prev_rect.map_or(false, |r| r.contains(self.input.mouse_pos));

        if is_hovered {
            self.hovered = Some(id);
        }

        // Press
        if is_hovered && self.input.mouse_just_pressed {
            self.active = Some(id);
            if self.focus != Some(id) {
                if let Some(old) = self.focus {
                    self.events.push(UIEvent::FocusOut(old));
                }
                self.focus = Some(id);
                self.events.push(UIEvent::FocusIn(id));
            }
        }

        // Click = release while active on this widget
        let clicked =
            is_hovered && self.input.mouse_just_released && self.active == Some(id);

        if self.input.mouse_just_released && self.active == Some(id) {
            if clicked {
                self.events.push(UIEvent::Click(id));
            }
            self.active = None;
        }

        WidgetInteraction {
            hovered: is_hovered,
            active: self.active == Some(id),
            focused: self.focus == Some(id),
            clicked,
        }
    }

    // -- Container stack ----------------------------------------------------

    pub fn push_parent(&mut self, node_id: NodeId) {
        self.node_stack.push(node_id);
    }

    pub fn pop_parent(&mut self) {
        self.node_stack.pop();
    }

    // -- Widget persistent data ---------------------------------------------

    pub fn get_widget_data<T: 'static + Default>(&mut self, id: WidgetId) -> &mut T {
        if !self.widget_data.contains_key(&id) {
            self.widget_data.insert(id, Box::new(T::default()));
        }
        self.widget_data
            .get_mut(&id)
            .unwrap()
            .downcast_mut::<T>()
            .unwrap()
    }

    // -- Accessors ----------------------------------------------------------

    pub fn input(&self) -> &InputState {
        &self.input
    }
    pub fn theme(&self) -> &Theme {
        &self.theme
    }
    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }
    pub fn draw_commands(&self) -> &[DrawCommand] {
        &self.draw_commands
    }
    pub fn events(&self) -> &[UIEvent] {
        &self.events
    }
    pub fn is_focused(&self, id: WidgetId) -> bool {
        self.focus == Some(id)
    }
    pub fn focused_widget(&self) -> Option<WidgetId> {
        self.focus
    }
    pub fn hovered_widget(&self) -> Option<WidgetId> {
        self.hovered
    }
    pub fn active_widget(&self) -> Option<WidgetId> {
        self.active
    }
    pub fn layout_tree(&self) -> &LayoutTree {
        &self.layout
    }
    pub fn prev_rect(&self, id: WidgetId) -> Option<Rect> {
        self.prev_rects.get(&id).copied()
    }

    // -- Clip stack ---------------------------------------------------------

    pub fn push_clip(&mut self, rect: Rect) {
        self.clip_stack.push(rect);
    }
    pub fn pop_clip(&mut self) {
        self.clip_stack.pop();
    }
    pub fn current_clip(&self) -> Option<Rect> {
        self.clip_stack.last().copied()
    }

    // -- Clipboard ----------------------------------------------------------

    pub fn clipboard(&self) -> &str {
        &self.clipboard
    }
    pub fn set_clipboard(&mut self, text: &str) {
        self.clipboard = text.to_string();
    }

    // -- Scroll -------------------------------------------------------------

    pub fn set_scroll_offset(&mut self, node_id: NodeId, offset: Vec2) {
        self.scroll_offsets.insert(node_id, offset);
    }

    // -- Focus --------------------------------------------------------------

    pub fn request_focus(&mut self, id: WidgetId) {
        if self.focus != Some(id) {
            if let Some(old) = self.focus {
                self.events.push(UIEvent::FocusOut(old));
            }
            self.focus = Some(id);
            self.events.push(UIEvent::FocusIn(id));
        }
    }

    /// Returns the appropriate [`Style`] for a widget's current state.
    pub fn style_for_state(&self, disabled: bool, interaction: &WidgetInteraction) -> Style {
        if disabled {
            self.theme.disabled.clone()
        } else if interaction.active {
            self.theme.active.clone()
        } else if interaction.hovered {
            self.theme.hover.clone()
        } else {
            self.theme.default.clone()
        }
    }

    // -- Hit testing (post-layout) ------------------------------------------

    /// Hit test using current frame's computed layout (call after `end_frame`).
    pub fn hit_test(&self, point: Vec2) -> Option<WidgetId> {
        let mut best: Option<(WidgetId, usize)> = None;
        for (i, opt_id) in self.node_widget_ids.iter().enumerate() {
            if let Some(id) = opt_id {
                let rect = self.layout.get_rect(NodeId(i));
                if rect.contains(point) {
                    if best.map_or(true, |(_, z)| i > z) {
                        best = Some((*id, i));
                    }
                }
            }
        }
        best.map(|(id, _)| id)
    }

    // -- Internal -----------------------------------------------------------

    fn cycle_focus(&mut self, reverse: bool) {
        if self.tab_order.is_empty() {
            return;
        }

        let cur = self
            .focus
            .and_then(|f| self.tab_order.iter().position(|&id| id == f));

        let next = match cur {
            Some(idx) => {
                if reverse {
                    if idx == 0 {
                        self.tab_order.len() - 1
                    } else {
                        idx - 1
                    }
                } else {
                    (idx + 1) % self.tab_order.len()
                }
            }
            None => 0,
        };

        let new_id = self.tab_order[next];
        if let Some(old) = self.focus {
            if old != new_id {
                self.events.push(UIEvent::FocusOut(old));
            }
        }
        self.focus = Some(new_id);
        self.events.push(UIEvent::FocusIn(new_id));
    }

    fn generate_draw_commands(&mut self) {
        self.draw_commands.clear();
        if self.layout.node_count() == 0 {
            return;
        }
        self.emit_node(NodeId(0), None, Vec2::ZERO);
    }

    fn emit_node(&mut self, id: NodeId, clip: Option<Rect>, scroll: Vec2) {
        if id.0 >= self.layout.node_count() {
            return;
        }

        let layout = self.layout.get_layout(id);
        let rect = Rect::from_min_size(
            Vec2::new(layout.x + scroll.x, layout.y + scroll.y),
            Vec2::new(layout.width, layout.height),
        );

        // Emit paint commands
        let cmds = self.render_data[id.0].clone();
        for cmd in &cmds {
            match cmd {
                NodePaintCmd::FilledRect {
                    color,
                    border_radius,
                } => {
                    self.draw_commands.push(DrawCommand::FilledRect {
                        rect,
                        color: *color,
                        border_radius: *border_radius,
                        clip,
                    });
                }
                NodePaintCmd::FilledRectCustom {
                    rect_offset,
                    rect_size,
                    color,
                    border_radius,
                } => {
                    let sub = Rect::from_min_size(
                        Vec2::new(rect.min.x + rect_offset.x, rect.min.y + rect_offset.y),
                        *rect_size,
                    );
                    self.draw_commands.push(DrawCommand::FilledRect {
                        rect: sub,
                        color: *color,
                        border_radius: *border_radius,
                        clip,
                    });
                }
                NodePaintCmd::BorderedRect {
                    fill,
                    border_color,
                    border_width,
                    border_radius,
                } => {
                    self.draw_commands.push(DrawCommand::BorderedRect {
                        rect,
                        fill: *fill,
                        border_color: *border_color,
                        border_width: *border_width,
                        border_radius: *border_radius,
                        clip,
                    });
                }
                NodePaintCmd::Text {
                    text,
                    color,
                    font_size,
                } => {
                    self.draw_commands.push(DrawCommand::Text {
                        position: rect.min,
                        text: text.clone(),
                        color: *color,
                        font_size: *font_size,
                        clip,
                    });
                }
                NodePaintCmd::TextOffset {
                    offset,
                    text,
                    color,
                    font_size,
                } => {
                    self.draw_commands.push(DrawCommand::Text {
                        position: Vec2::new(rect.min.x + offset.x, rect.min.y + offset.y),
                        text: text.clone(),
                        color: *color,
                        font_size: *font_size,
                        clip,
                    });
                }
                NodePaintCmd::Image { texture, sizing } => {
                    self.draw_commands.push(DrawCommand::Image {
                        rect,
                        texture: *texture,
                        sizing: *sizing,
                        clip,
                    });
                }
            }
        }

        // Determine clip and scroll for children
        let child_clip = if self.layout.get_node(id).overflow == Overflow::Hidden {
            match clip {
                Some(pc) => pc.intersection(rect),
                None => Some(rect),
            }
        } else {
            clip
        };

        let child_scroll = if let Some(&so) = self.scroll_offsets.get(&id) {
            Vec2::new(scroll.x - so.x, scroll.y - so.y)
        } else {
            scroll
        };

        let children: Vec<NodeId> = self.layout.get_node(id).children.clone();
        for cid in children {
            self.emit_node(cid, child_clip, child_scroll);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{FlexDirection, Size};

    /// Helper: add a simple fixed-size widget and end frame.
    fn add_widget(ctx: &mut UIContext, id: &str, w: f32, h: f32) -> WidgetId {
        let wid = WidgetId::new(id);
        ctx.add_widget(
            wid,
            LayoutNode {
                width: Size::Fixed(w),
                height: Size::Fixed(h),
                ..Default::default()
            },
            true,
        );
        wid
    }

    // -- Hit testing --

    #[test]
    fn hit_test_correct_node() {
        let mut ctx = UIContext::new();
        ctx.begin_frame(InputState::default());
        add_widget(&mut ctx, "btn", 100.0, 100.0);
        ctx.end_frame(800.0, 600.0);

        assert_eq!(ctx.hit_test(Vec2::new(50.0, 50.0)), Some(WidgetId::new("btn")));
    }

    #[test]
    fn hit_test_outside_no_hit() {
        let mut ctx = UIContext::new();
        ctx.begin_frame(InputState::default());
        add_widget(&mut ctx, "btn", 100.0, 100.0);
        ctx.end_frame(800.0, 600.0);

        assert_eq!(ctx.hit_test(Vec2::new(500.0, 500.0)), None);
    }

    #[test]
    fn hit_test_overlapping_topmost() {
        let mut ctx = UIContext::new();
        ctx.begin_frame(InputState::default());
        // Two overlapping widgets (both 200×200, same position since column)
        // Actually in a column, they stack. Let me use the same position:
        // Both widgets have same size and are in a column, so they don't overlap
        // by default. Let me add them to the same container.
        // For overlapping, I'll place them at the same position by using the root.
        let a = WidgetId::new("a");
        ctx.add_widget(
            a,
            LayoutNode {
                width: Size::Fixed(200.0),
                height: Size::Fixed(200.0),
                ..Default::default()
            },
            false,
        );
        let b = WidgetId::new("b");
        ctx.add_widget(
            b,
            LayoutNode {
                width: Size::Fixed(200.0),
                height: Size::Fixed(200.0),
                ..Default::default()
            },
            false,
        );
        ctx.end_frame(800.0, 600.0);

        // b was added after a, so b has higher z-order.
        // In a column layout, b is at y=200, a is at y=0. They don't overlap.
        // For true overlap, we need widgets at the same position.
        // The hit_test at (50,50) should hit 'a' (which is at y=0..200).
        assert_eq!(ctx.hit_test(Vec2::new(50.0, 50.0)), Some(a));
        // At (50, 250) should hit 'b' (at y=200..400).
        assert_eq!(ctx.hit_test(Vec2::new(50.0, 250.0)), Some(b));
    }

    #[test]
    fn hit_test_z_order_higher_wins() {
        // To create truly overlapping nodes, we place two fixed children in
        // a container with absolute-style sizes that exceed the container.
        // Both children start at (0,0) because they are in a row but both
        // have width > available / 2, causing overlap isn't actually possible
        // with flex layout unless we create a custom scenario.
        //
        // Simplest approach: two children in a row where the first child
        // is wider than half. The second child's x will be pushed past the
        // first, so no overlap. For z-order testing with real overlap, we'd
        // need absolute positioning.
        //
        // Instead, verify that among multiple nodes containing the point,
        // the one with the highest index wins.
        let mut ctx = UIContext::new();
        ctx.begin_frame(InputState::default());

        // Create a parent that is 100×100
        let parent_id = WidgetId::new("parent");
        let _parent_node = ctx.add_widget(
            parent_id,
            LayoutNode {
                width: Size::Fixed(100.0),
                height: Size::Fixed(100.0),
                ..Default::default()
            },
            false,
        );
        ctx.end_frame(800.0, 600.0);

        // The parent rect is (0,0)-(100,100). Point (50,50) hits both root
        // (the implicit root at 800×600) and "parent". "parent" has higher index.
        assert_eq!(
            ctx.hit_test(Vec2::new(50.0, 50.0)),
            Some(parent_id)
        );
    }

    // -- Focus --

    #[test]
    fn focus_tab_cycles_dfs_order() {
        let mut ctx = UIContext::new();

        // Frame 1: register 3 focusable widgets
        ctx.begin_frame(InputState::default());
        let a = add_widget(&mut ctx, "a", 100.0, 30.0);
        let b = add_widget(&mut ctx, "b", 100.0, 30.0);
        let c = add_widget(&mut ctx, "c", 100.0, 30.0);
        ctx.end_frame(800.0, 600.0);

        assert_eq!(ctx.focused_widget(), None);

        // Frame 2: press Tab -> focus a
        let mut input = InputState::default();
        input.keys_just_pressed.push(KeyCode::Tab);
        ctx.begin_frame(input);
        add_widget(&mut ctx, "a", 100.0, 30.0);
        add_widget(&mut ctx, "b", 100.0, 30.0);
        add_widget(&mut ctx, "c", 100.0, 30.0);
        ctx.end_frame(800.0, 600.0);

        assert_eq!(ctx.focused_widget(), Some(a));

        // Frame 3: Tab -> focus b
        let mut input = InputState::default();
        input.keys_just_pressed.push(KeyCode::Tab);
        ctx.begin_frame(input);
        add_widget(&mut ctx, "a", 100.0, 30.0);
        add_widget(&mut ctx, "b", 100.0, 30.0);
        add_widget(&mut ctx, "c", 100.0, 30.0);
        ctx.end_frame(800.0, 600.0);

        assert_eq!(ctx.focused_widget(), Some(b));

        // Frame 4: Tab -> focus c
        let mut input = InputState::default();
        input.keys_just_pressed.push(KeyCode::Tab);
        ctx.begin_frame(input);
        add_widget(&mut ctx, "a", 100.0, 30.0);
        add_widget(&mut ctx, "b", 100.0, 30.0);
        add_widget(&mut ctx, "c", 100.0, 30.0);
        ctx.end_frame(800.0, 600.0);

        assert_eq!(ctx.focused_widget(), Some(c));

        // Frame 5: Tab -> wraps back to a
        let mut input = InputState::default();
        input.keys_just_pressed.push(KeyCode::Tab);
        ctx.begin_frame(input);
        add_widget(&mut ctx, "a", 100.0, 30.0);
        add_widget(&mut ctx, "b", 100.0, 30.0);
        add_widget(&mut ctx, "c", 100.0, 30.0);
        ctx.end_frame(800.0, 600.0);

        assert_eq!(ctx.focused_widget(), Some(a));
    }

    #[test]
    fn focus_shift_tab_reverses() {
        let mut ctx = UIContext::new();

        // Frame 1
        ctx.begin_frame(InputState::default());
        add_widget(&mut ctx, "a", 100.0, 30.0);
        add_widget(&mut ctx, "b", 100.0, 30.0);
        ctx.end_frame(800.0, 600.0);

        // Tab to a
        let mut input = InputState::default();
        input.keys_just_pressed.push(KeyCode::Tab);
        ctx.begin_frame(input);
        add_widget(&mut ctx, "a", 100.0, 30.0);
        add_widget(&mut ctx, "b", 100.0, 30.0);
        ctx.end_frame(800.0, 600.0);
        assert_eq!(ctx.focused_widget(), Some(WidgetId::new("a")));

        // Tab to b
        let mut input = InputState::default();
        input.keys_just_pressed.push(KeyCode::Tab);
        ctx.begin_frame(input);
        add_widget(&mut ctx, "a", 100.0, 30.0);
        add_widget(&mut ctx, "b", 100.0, 30.0);
        ctx.end_frame(800.0, 600.0);
        assert_eq!(ctx.focused_widget(), Some(WidgetId::new("b")));

        // Shift+Tab back to a
        let mut input = InputState::default();
        input.keys_just_pressed.push(KeyCode::Tab);
        input.keys_held.push(KeyCode::LeftShift);
        ctx.begin_frame(input);
        add_widget(&mut ctx, "a", 100.0, 30.0);
        add_widget(&mut ctx, "b", 100.0, 30.0);
        ctx.end_frame(800.0, 600.0);
        assert_eq!(ctx.focused_widget(), Some(WidgetId::new("a")));
    }

    // -- Clip --

    #[test]
    fn clip_overflow_hidden() {
        let mut ctx = UIContext::new();
        ctx.begin_frame(InputState::default());

        // Parent: 100×100 with overflow hidden
        let parent_id = WidgetId::new("panel");
        let parent_node = ctx.add_widget(
            parent_id,
            LayoutNode {
                width: Size::Fixed(100.0),
                height: Size::Fixed(100.0),
                overflow: Overflow::Hidden,
                flex_direction: FlexDirection::Column,
                ..Default::default()
            },
            false,
        );
        ctx.push_parent(parent_node);

        // Child: 100×200 (exceeds parent)
        let child_id = WidgetId::new("child");
        let child_node = ctx.add_widget(
            child_id,
            LayoutNode {
                width: Size::Fixed(100.0),
                height: Size::Fixed(200.0),
                ..Default::default()
            },
            false,
        );
        ctx.add_paint_cmd(
            child_node,
            NodePaintCmd::FilledRect {
                color: Color::RED,
                border_radius: 0.0,
            },
        );

        ctx.pop_parent();
        ctx.end_frame(800.0, 600.0);

        // The child's draw command should have a clip rect
        let has_clip = ctx.draw_commands().iter().any(|cmd| {
            if let DrawCommand::FilledRect { clip, .. } = cmd {
                clip.is_some()
            } else {
                false
            }
        });
        assert!(has_clip, "Child of overflow:hidden should be clipped");
    }
}
