use arachne_math::{Rect, Vec2};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// How a dimension (width/height) is sized.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Size {
    /// Exact pixel value.
    Fixed(f32),
    /// Percentage of the parent's content area (0–100).
    Percent(f32),
    /// Size to content (intrinsic) or fill available space.
    Auto,
}

impl Default for Size {
    fn default() -> Self {
        Size::Auto
    }
}

/// Direction of the main axis for flex layout.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum FlexDirection {
    #[default]
    Row,
    Column,
}

/// Cross-axis alignment of children.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum AlignItems {
    #[default]
    Start,
    Center,
    End,
    Stretch,
}

/// Main-axis distribution of children.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum JustifyContent {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
}

/// Overflow behaviour.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum Overflow {
    #[default]
    Visible,
    Hidden,
}

/// Edge insets (top, right, bottom, left).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Edges {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Default for Edges {
    fn default() -> Self {
        Self::ZERO
    }
}

impl Edges {
    pub const ZERO: Self = Self {
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
    };

    #[inline]
    pub fn all(v: f32) -> Self {
        Self {
            top: v,
            right: v,
            bottom: v,
            left: v,
        }
    }

    #[inline]
    pub fn symmetric(vertical: f32, horizontal: f32) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }

    #[inline]
    pub fn horizontal(&self) -> f32 {
        self.left + self.right
    }

    #[inline]
    pub fn vertical(&self) -> f32 {
        self.top + self.bottom
    }
}

/// Index into the [`LayoutTree`] arena.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeId(pub usize);

/// A node in the layout tree describing size constraints and flex properties.
#[derive(Clone, Debug)]
pub struct LayoutNode {
    pub width: Size,
    pub height: Size,
    pub min_width: Option<f32>,
    pub max_width: Option<f32>,
    pub min_height: Option<f32>,
    pub max_height: Option<f32>,
    pub flex_direction: FlexDirection,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub align_items: AlignItems,
    pub justify_content: JustifyContent,
    pub gap: f32,
    pub padding: Edges,
    pub margin: Edges,
    pub overflow: Overflow,
    pub children: Vec<NodeId>,
}

impl Default for LayoutNode {
    fn default() -> Self {
        Self {
            width: Size::Auto,
            height: Size::Auto,
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            flex_direction: FlexDirection::Row,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            align_items: AlignItems::Start,
            justify_content: JustifyContent::Start,
            gap: 0.0,
            padding: Edges::ZERO,
            margin: Edges::ZERO,
            overflow: Overflow::Visible,
            children: Vec::new(),
        }
    }
}

/// The computed position and size of a layout node (border-box, excludes margin).
#[derive(Clone, Copy, Debug, Default)]
pub struct ComputedLayout {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl ComputedLayout {
    /// Convert to an axis-aligned [`Rect`].
    #[inline]
    pub fn rect(&self) -> Rect {
        Rect::from_min_size(Vec2::new(self.x, self.y), Vec2::new(self.width, self.height))
    }
}

// ---------------------------------------------------------------------------
// Layout Tree
// ---------------------------------------------------------------------------

/// Arena-based layout tree with a two-pass flex layout algorithm.
pub struct LayoutTree {
    nodes: Vec<LayoutNode>,
    computed: Vec<ComputedLayout>,
    intrinsic: Vec<(f32, f32)>,
}

impl LayoutTree {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            computed: Vec::new(),
            intrinsic: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.nodes.clear();
        self.computed.clear();
        self.intrinsic.clear();
    }

    /// Add a node and return its [`NodeId`].
    pub fn add_node(&mut self, node: LayoutNode) -> NodeId {
        let id = NodeId(self.nodes.len());
        self.nodes.push(node);
        self.computed.push(ComputedLayout::default());
        self.intrinsic.push((0.0, 0.0));
        id
    }

    /// Register `child` as a child of `parent`.
    pub fn add_child(&mut self, parent: NodeId, child: NodeId) {
        self.nodes[parent.0].children.push(child);
    }

    pub fn get_node(&self, id: NodeId) -> &LayoutNode {
        &self.nodes[id.0]
    }

    pub fn get_node_mut(&mut self, id: NodeId) -> &mut LayoutNode {
        &mut self.nodes[id.0]
    }

    pub fn get_layout(&self, id: NodeId) -> ComputedLayout {
        self.computed[id.0]
    }

    pub fn get_rect(&self, id: NodeId) -> Rect {
        self.computed[id.0].rect()
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Run the two-pass layout algorithm starting at `root`.
    pub fn compute(&mut self, root: NodeId, available_w: f32, available_h: f32) {
        self.measure(root);
        // Resolve the root node's size before arranging.
        let node = &self.nodes[root.0];
        let intrinsic = self.intrinsic[root.0];
        let w = resolve(
            node.width,
            available_w,
            intrinsic.0,
            node.min_width,
            node.max_width,
        );
        let h = resolve(
            node.height,
            available_h,
            intrinsic.1,
            node.min_height,
            node.max_height,
        );
        self.arrange(root, 0.0, 0.0, w, h);
    }

    // -- Pass 1: Measure intrinsic sizes bottom-up --------------------------

    fn measure(&mut self, id: NodeId) -> (f32, f32) {
        let children: Vec<NodeId> = self.nodes[id.0].children.clone();
        let mut child_sizes: Vec<(f32, f32)> = Vec::with_capacity(children.len());
        for &cid in &children {
            child_sizes.push(self.measure(cid));
        }

        let node = &self.nodes[id.0];
        let direction = node.flex_direction;
        let gap = node.gap;
        let padding = node.padding;

        let gaps = if children.len() > 1 {
            (children.len() - 1) as f32 * gap
        } else {
            0.0
        };

        // Accumulate child outer sizes (intrinsic + margin).
        let child_outer: Vec<(f32, f32)> = child_sizes
            .iter()
            .enumerate()
            .map(|(i, &(w, h))| {
                let cm = self.nodes[children[i].0].margin;
                (w + cm.horizontal(), h + cm.vertical())
            })
            .collect();

        let intrinsic_w = match node.width {
            Size::Fixed(v) => v,
            Size::Percent(_) => 0.0,
            Size::Auto => {
                if children.is_empty() {
                    padding.horizontal()
                } else {
                    match direction {
                        FlexDirection::Row => {
                            child_outer.iter().map(|c| c.0).sum::<f32>()
                                + gaps
                                + padding.horizontal()
                        }
                        FlexDirection::Column => {
                            child_outer
                                .iter()
                                .map(|c| c.0)
                                .fold(0.0_f32, f32::max)
                                + padding.horizontal()
                        }
                    }
                }
            }
        };

        let intrinsic_h = match node.height {
            Size::Fixed(v) => v,
            Size::Percent(_) => 0.0,
            Size::Auto => {
                if children.is_empty() {
                    padding.vertical()
                } else {
                    match direction {
                        FlexDirection::Row => {
                            child_outer
                                .iter()
                                .map(|c| c.1)
                                .fold(0.0_f32, f32::max)
                                + padding.vertical()
                        }
                        FlexDirection::Column => {
                            child_outer.iter().map(|c| c.1).sum::<f32>()
                                + gaps
                                + padding.vertical()
                        }
                    }
                }
            }
        };

        self.intrinsic[id.0] = (intrinsic_w, intrinsic_h);
        (intrinsic_w, intrinsic_h)
    }

    // -- Pass 2: Arrange top-down -------------------------------------------

    /// `resolved_w` and `resolved_h` are the pre-resolved border-box size
    /// (the parent has already resolved Size::Fixed/Percent/Auto/flex).
    fn arrange(
        &mut self,
        id: NodeId,
        x: f32,
        y: f32,
        resolved_w: f32,
        resolved_h: f32,
    ) {
        let margin = self.nodes[id.0].margin;

        let w = resolved_w;
        let h = resolved_h;

        let mx = x + margin.left;
        let my = y + margin.top;

        self.computed[id.0] = ComputedLayout {
            x: mx,
            y: my,
            width: w,
            height: h,
        };

        let children: Vec<NodeId> = self.nodes[id.0].children.clone();
        if children.is_empty() {
            return;
        }

        let padding = self.nodes[id.0].padding;
        let cx = mx + padding.left;
        let cy = my + padding.top;
        let cw = (w - padding.horizontal()).max(0.0);
        let ch = (h - padding.vertical()).max(0.0);

        let direction = self.nodes[id.0].flex_direction;
        let justify = self.nodes[id.0].justify_content;
        let align = self.nodes[id.0].align_items;
        let gap = self.nodes[id.0].gap;

        match direction {
            FlexDirection::Row => self.arrange_row(&children, cx, cy, cw, ch, justify, align, gap),
            FlexDirection::Column => {
                self.arrange_column(&children, cx, cy, cw, ch, justify, align, gap)
            }
        }
    }

    fn arrange_row(
        &mut self,
        children: &[NodeId],
        cx: f32,
        cy: f32,
        cw: f32,
        ch: f32,
        justify: JustifyContent,
        align: AlignItems,
        gap: f32,
    ) {
        let gaps_total = if children.len() > 1 {
            (children.len() - 1) as f32 * gap
        } else {
            0.0
        };

        // Resolve child widths (border-box, excluding margin).
        let mut child_widths: Vec<f32> = Vec::with_capacity(children.len());
        let mut total_fixed: f32 = 0.0;
        let mut total_flex: f32 = 0.0;

        for &cid in children {
            let child = &self.nodes[cid.0];
            let ci = self.intrinsic[cid.0];
            let w = match child.width {
                Size::Fixed(v) => v,
                Size::Percent(p) => cw * p / 100.0,
                Size::Auto => {
                    if child.flex_grow > 0.0 {
                        0.0
                    } else {
                        ci.0
                    }
                }
            };
            child_widths.push(w);
            total_fixed += w + child.margin.horizontal();
            total_flex += child.flex_grow;
        }

        // Distribute remaining space via flex_grow.
        let remaining = (cw - total_fixed - gaps_total).max(0.0);
        if total_flex > 0.0 {
            for (i, &cid) in children.iter().enumerate() {
                let grow = self.nodes[cid.0].flex_grow;
                if grow > 0.0 {
                    child_widths[i] += remaining * grow / total_flex;
                }
            }
        }

        // Clamp to min/max.
        for (i, &cid) in children.iter().enumerate() {
            let child = &self.nodes[cid.0];
            if let Some(min) = child.min_width {
                child_widths[i] = child_widths[i].max(min);
            }
            if let Some(max) = child.max_width {
                child_widths[i] = child_widths[i].min(max);
            }
        }

        // Total children width for justification.
        let total_w: f32 = child_widths.iter().sum::<f32>()
            + children
                .iter()
                .map(|&cid| self.nodes[cid.0].margin.horizontal())
                .sum::<f32>()
            + gaps_total;
        let free = (cw - total_w).max(0.0);

        let (mut cursor, extra) = justify_offset(justify, free, children.len(), cx);

        for (i, &cid) in children.iter().enumerate() {
            let child_height_size = self.nodes[cid.0].height;
            let child_min_h = self.nodes[cid.0].min_height;
            let child_max_h = self.nodes[cid.0].max_height;
            let child_margin = self.nodes[cid.0].margin;
            let child_intrinsic_h = self.intrinsic[cid.0].1;

            let child_h = resolve_cross(
                child_height_size,
                ch,
                child_intrinsic_h,
                child_min_h,
                child_max_h,
                align,
                child_margin.vertical(),
            );

            let child_y = cross_offset(align, cy, ch, child_h, child_margin.vertical());

            self.arrange(cid, cursor, child_y, child_widths[i], child_h);

            cursor += child_widths[i] + child_margin.horizontal() + gap + extra;
        }
    }

    fn arrange_column(
        &mut self,
        children: &[NodeId],
        cx: f32,
        cy: f32,
        cw: f32,
        ch: f32,
        justify: JustifyContent,
        align: AlignItems,
        gap: f32,
    ) {
        let gaps_total = if children.len() > 1 {
            (children.len() - 1) as f32 * gap
        } else {
            0.0
        };

        let mut child_heights: Vec<f32> = Vec::with_capacity(children.len());
        let mut total_fixed: f32 = 0.0;
        let mut total_flex: f32 = 0.0;

        for &cid in children {
            let child = &self.nodes[cid.0];
            let ci = self.intrinsic[cid.0];
            let h = match child.height {
                Size::Fixed(v) => v,
                Size::Percent(p) => ch * p / 100.0,
                Size::Auto => {
                    if child.flex_grow > 0.0 {
                        0.0
                    } else {
                        ci.1
                    }
                }
            };
            child_heights.push(h);
            total_fixed += h + child.margin.vertical();
            total_flex += child.flex_grow;
        }

        let remaining = (ch - total_fixed - gaps_total).max(0.0);
        if total_flex > 0.0 {
            for (i, &cid) in children.iter().enumerate() {
                let grow = self.nodes[cid.0].flex_grow;
                if grow > 0.0 {
                    child_heights[i] += remaining * grow / total_flex;
                }
            }
        }

        for (i, &cid) in children.iter().enumerate() {
            let child = &self.nodes[cid.0];
            if let Some(min) = child.min_height {
                child_heights[i] = child_heights[i].max(min);
            }
            if let Some(max) = child.max_height {
                child_heights[i] = child_heights[i].min(max);
            }
        }

        let total_h: f32 = child_heights.iter().sum::<f32>()
            + children
                .iter()
                .map(|&cid| self.nodes[cid.0].margin.vertical())
                .sum::<f32>()
            + gaps_total;
        let free = (ch - total_h).max(0.0);

        let (mut cursor, extra) = justify_offset(justify, free, children.len(), cy);

        for (i, &cid) in children.iter().enumerate() {
            let child_width_size = self.nodes[cid.0].width;
            let child_min_w = self.nodes[cid.0].min_width;
            let child_max_w = self.nodes[cid.0].max_width;
            let child_margin = self.nodes[cid.0].margin;
            let child_intrinsic_w = self.intrinsic[cid.0].0;

            let child_w = resolve_cross(
                child_width_size,
                cw,
                child_intrinsic_w,
                child_min_w,
                child_max_w,
                align,
                child_margin.horizontal(),
            );

            let child_x = cross_offset(align, cx, cw, child_w, child_margin.horizontal());

            self.arrange(cid, child_x, cursor, child_w, child_heights[i]);

            cursor += child_heights[i] + child_margin.vertical() + gap + extra;
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn resolve(
    size: Size,
    available: f32,
    intrinsic: f32,
    min: Option<f32>,
    max: Option<f32>,
) -> f32 {
    let v = match size {
        Size::Fixed(f) => f,
        Size::Percent(p) => available * p / 100.0,
        Size::Auto => {
            if intrinsic > 0.0 {
                intrinsic
            } else {
                available
            }
        }
    };
    clamp_opt(v, min, max)
}

fn resolve_cross(
    size: Size,
    available: f32,
    intrinsic: f32,
    min: Option<f32>,
    max: Option<f32>,
    align: AlignItems,
    margin: f32,
) -> f32 {
    let v = match size {
        Size::Fixed(f) => f,
        Size::Percent(p) => available * p / 100.0,
        Size::Auto => match align {
            AlignItems::Stretch => available - margin,
            _ => {
                if intrinsic > 0.0 {
                    intrinsic
                } else {
                    available - margin
                }
            }
        },
    };
    clamp_opt(v, min, max)
}

fn cross_offset(align: AlignItems, start: f32, available: f32, size: f32, margin: f32) -> f32 {
    match align {
        AlignItems::Start | AlignItems::Stretch => start,
        AlignItems::Center => start + (available - size - margin) / 2.0,
        AlignItems::End => start + available - size - margin,
    }
}

fn justify_offset(
    justify: JustifyContent,
    free: f32,
    count: usize,
    start: f32,
) -> (f32, f32) {
    match justify {
        JustifyContent::Start => (start, 0.0),
        JustifyContent::End => (start + free, 0.0),
        JustifyContent::Center => (start + free / 2.0, 0.0),
        JustifyContent::SpaceBetween => {
            if count > 1 {
                (start, free / (count - 1) as f32)
            } else {
                (start, 0.0)
            }
        }
        JustifyContent::SpaceAround => {
            if count > 0 {
                let space = free / count as f32;
                (start + space / 2.0, space)
            } else {
                (start, 0.0)
            }
        }
    }
}

#[inline]
fn clamp_opt(v: f32, min: Option<f32>, max: Option<f32>) -> f32 {
    let v = if let Some(lo) = min { v.max(lo) } else { v };
    if let Some(hi) = max {
        v.min(hi)
    } else {
        v
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1.0
    }

    // -- Row layout --

    #[test]
    fn row_three_children_100px() {
        let mut tree = LayoutTree::new();
        let parent = tree.add_node(LayoutNode {
            width: Size::Fixed(300.0),
            height: Size::Fixed(100.0),
            flex_direction: FlexDirection::Row,
            ..Default::default()
        });
        let c0 = tree.add_node(LayoutNode {
            width: Size::Fixed(100.0),
            height: Size::Fixed(100.0),
            ..Default::default()
        });
        let c1 = tree.add_node(LayoutNode {
            width: Size::Fixed(100.0),
            height: Size::Fixed(100.0),
            ..Default::default()
        });
        let c2 = tree.add_node(LayoutNode {
            width: Size::Fixed(100.0),
            height: Size::Fixed(100.0),
            ..Default::default()
        });
        tree.add_child(parent, c0);
        tree.add_child(parent, c1);
        tree.add_child(parent, c2);
        tree.compute(parent, 300.0, 100.0);

        assert!(approx(tree.get_layout(c0).x, 0.0));
        assert!(approx(tree.get_layout(c1).x, 100.0));
        assert!(approx(tree.get_layout(c2).x, 200.0));
        assert!(approx(tree.get_layout(c0).width, 100.0));
        assert!(approx(tree.get_layout(c1).width, 100.0));
        assert!(approx(tree.get_layout(c2).width, 100.0));
    }

    // -- Column layout --

    #[test]
    fn column_three_children_50px() {
        let mut tree = LayoutTree::new();
        let parent = tree.add_node(LayoutNode {
            width: Size::Fixed(200.0),
            height: Size::Fixed(150.0),
            flex_direction: FlexDirection::Column,
            ..Default::default()
        });
        let c0 = tree.add_node(LayoutNode {
            width: Size::Fixed(200.0),
            height: Size::Fixed(50.0),
            ..Default::default()
        });
        let c1 = tree.add_node(LayoutNode {
            width: Size::Fixed(200.0),
            height: Size::Fixed(50.0),
            ..Default::default()
        });
        let c2 = tree.add_node(LayoutNode {
            width: Size::Fixed(200.0),
            height: Size::Fixed(50.0),
            ..Default::default()
        });
        tree.add_child(parent, c0);
        tree.add_child(parent, c1);
        tree.add_child(parent, c2);
        tree.compute(parent, 200.0, 150.0);

        assert!(approx(tree.get_layout(c0).y, 0.0));
        assert!(approx(tree.get_layout(c1).y, 50.0));
        assert!(approx(tree.get_layout(c2).y, 100.0));
    }

    // -- Flex grow --

    #[test]
    fn flex_grow_1_2_in_300px() {
        let mut tree = LayoutTree::new();
        let parent = tree.add_node(LayoutNode {
            width: Size::Fixed(300.0),
            height: Size::Fixed(100.0),
            flex_direction: FlexDirection::Row,
            ..Default::default()
        });
        let c0 = tree.add_node(LayoutNode {
            flex_grow: 1.0,
            ..Default::default()
        });
        let c1 = tree.add_node(LayoutNode {
            flex_grow: 2.0,
            ..Default::default()
        });
        tree.add_child(parent, c0);
        tree.add_child(parent, c1);
        tree.compute(parent, 300.0, 100.0);

        assert!(approx(tree.get_layout(c0).width, 100.0));
        assert!(approx(tree.get_layout(c1).width, 200.0));
        assert!(approx(tree.get_layout(c0).x, 0.0));
        assert!(approx(tree.get_layout(c1).x, 100.0));
    }

    // -- Padding / Margin --

    #[test]
    fn padding_offsets_children() {
        let mut tree = LayoutTree::new();
        let parent = tree.add_node(LayoutNode {
            width: Size::Fixed(300.0),
            height: Size::Fixed(100.0),
            padding: Edges::all(10.0),
            flex_direction: FlexDirection::Row,
            ..Default::default()
        });
        let c0 = tree.add_node(LayoutNode {
            width: Size::Fixed(100.0),
            height: Size::Fixed(50.0),
            ..Default::default()
        });
        tree.add_child(parent, c0);
        tree.compute(parent, 300.0, 100.0);

        assert!(approx(tree.get_layout(c0).x, 10.0));
        assert!(approx(tree.get_layout(c0).y, 10.0));
    }

    #[test]
    fn margin_offsets_child() {
        let mut tree = LayoutTree::new();
        let parent = tree.add_node(LayoutNode {
            width: Size::Fixed(300.0),
            height: Size::Fixed(100.0),
            padding: Edges::all(10.0),
            flex_direction: FlexDirection::Row,
            ..Default::default()
        });
        let c0 = tree.add_node(LayoutNode {
            width: Size::Fixed(100.0),
            height: Size::Fixed(50.0),
            margin: Edges::all(5.0),
            ..Default::default()
        });
        tree.add_child(parent, c0);
        tree.compute(parent, 300.0, 100.0);

        // Parent padding 10 + child margin 5
        assert!(approx(tree.get_layout(c0).x, 15.0));
        assert!(approx(tree.get_layout(c0).y, 15.0));
    }

    // -- Gap --

    #[test]
    fn row_gap() {
        let mut tree = LayoutTree::new();
        let parent = tree.add_node(LayoutNode {
            width: Size::Fixed(300.0),
            height: Size::Fixed(100.0),
            flex_direction: FlexDirection::Row,
            gap: 10.0,
            ..Default::default()
        });
        let c0 = tree.add_node(LayoutNode {
            width: Size::Fixed(90.0),
            height: Size::Fixed(100.0),
            ..Default::default()
        });
        let c1 = tree.add_node(LayoutNode {
            width: Size::Fixed(90.0),
            height: Size::Fixed(100.0),
            ..Default::default()
        });
        let c2 = tree.add_node(LayoutNode {
            width: Size::Fixed(90.0),
            height: Size::Fixed(100.0),
            ..Default::default()
        });
        tree.add_child(parent, c0);
        tree.add_child(parent, c1);
        tree.add_child(parent, c2);
        tree.compute(parent, 300.0, 100.0);

        assert!(approx(tree.get_layout(c0).x, 0.0));
        assert!(approx(tree.get_layout(c1).x, 100.0)); // 90 + 10
        assert!(approx(tree.get_layout(c2).x, 200.0)); // 90+10+90+10
    }

    // -- Justify content --

    #[test]
    fn justify_center() {
        let mut tree = LayoutTree::new();
        let parent = tree.add_node(LayoutNode {
            width: Size::Fixed(300.0),
            height: Size::Fixed(100.0),
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::Center,
            ..Default::default()
        });
        let c0 = tree.add_node(LayoutNode {
            width: Size::Fixed(100.0),
            height: Size::Fixed(100.0),
            ..Default::default()
        });
        tree.add_child(parent, c0);
        tree.compute(parent, 300.0, 100.0);

        // Centered: (300 - 100) / 2 = 100
        assert!(approx(tree.get_layout(c0).x, 100.0));
    }

    #[test]
    fn justify_space_between() {
        let mut tree = LayoutTree::new();
        let parent = tree.add_node(LayoutNode {
            width: Size::Fixed(300.0),
            height: Size::Fixed(100.0),
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            ..Default::default()
        });
        let c0 = tree.add_node(LayoutNode {
            width: Size::Fixed(50.0),
            height: Size::Fixed(100.0),
            ..Default::default()
        });
        let c1 = tree.add_node(LayoutNode {
            width: Size::Fixed(50.0),
            height: Size::Fixed(100.0),
            ..Default::default()
        });
        tree.add_child(parent, c0);
        tree.add_child(parent, c1);
        tree.compute(parent, 300.0, 100.0);

        // Free space = 300 - 100 = 200, distributed between 2 items => 200 gap
        assert!(approx(tree.get_layout(c0).x, 0.0));
        assert!(approx(tree.get_layout(c1).x, 250.0)); // 50 + 200
    }

    // -- Align items --

    #[test]
    fn align_center() {
        let mut tree = LayoutTree::new();
        let parent = tree.add_node(LayoutNode {
            width: Size::Fixed(300.0),
            height: Size::Fixed(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            ..Default::default()
        });
        let c0 = tree.add_node(LayoutNode {
            width: Size::Fixed(100.0),
            height: Size::Fixed(40.0),
            ..Default::default()
        });
        tree.add_child(parent, c0);
        tree.compute(parent, 300.0, 100.0);

        // Centered vertically: (100 - 40) / 2 = 30
        assert!(approx(tree.get_layout(c0).y, 30.0));
    }

    // -- Min / Max constraints --

    #[test]
    fn min_width_enforced() {
        let mut tree = LayoutTree::new();
        let parent = tree.add_node(LayoutNode {
            width: Size::Fixed(300.0),
            height: Size::Fixed(100.0),
            flex_direction: FlexDirection::Row,
            ..Default::default()
        });
        let c0 = tree.add_node(LayoutNode {
            width: Size::Fixed(10.0),
            height: Size::Fixed(100.0),
            min_width: Some(50.0),
            ..Default::default()
        });
        tree.add_child(parent, c0);
        tree.compute(parent, 300.0, 100.0);

        assert!(approx(tree.get_layout(c0).width, 50.0));
    }

    #[test]
    fn max_width_enforced() {
        let mut tree = LayoutTree::new();
        let parent = tree.add_node(LayoutNode {
            width: Size::Fixed(300.0),
            height: Size::Fixed(100.0),
            flex_direction: FlexDirection::Row,
            ..Default::default()
        });
        let c0 = tree.add_node(LayoutNode {
            width: Size::Fixed(500.0),
            height: Size::Fixed(100.0),
            max_width: Some(200.0),
            ..Default::default()
        });
        tree.add_child(parent, c0);
        tree.compute(parent, 300.0, 100.0);

        assert!(approx(tree.get_layout(c0).width, 200.0));
    }

    // -- Percent sizing --

    #[test]
    fn percent_width() {
        let mut tree = LayoutTree::new();
        let parent = tree.add_node(LayoutNode {
            width: Size::Fixed(400.0),
            height: Size::Fixed(100.0),
            flex_direction: FlexDirection::Row,
            ..Default::default()
        });
        let c0 = tree.add_node(LayoutNode {
            width: Size::Percent(50.0),
            height: Size::Fixed(100.0),
            ..Default::default()
        });
        tree.add_child(parent, c0);
        tree.compute(parent, 400.0, 100.0);

        assert!(approx(tree.get_layout(c0).width, 200.0));
    }

    // -- Benchmark --

    #[test]
    fn benchmark_layout_200_widgets() {
        let mut tree = LayoutTree::new();
        let root = tree.add_node(LayoutNode {
            width: Size::Fixed(1920.0),
            height: Size::Fixed(1080.0),
            flex_direction: FlexDirection::Column,
            ..Default::default()
        });
        for _ in 0..200 {
            let child = tree.add_node(LayoutNode {
                width: Size::Fixed(100.0),
                height: Size::Fixed(20.0),
                ..Default::default()
            });
            tree.add_child(root, child);
        }

        let start = std::time::Instant::now();
        tree.compute(root, 1920.0, 1080.0);
        let elapsed = start.elapsed();

        eprintln!("Layout 200 widgets: {:?}", elapsed);
        assert!(
            elapsed.as_micros() < 1000,
            "Layout took {}µs, expected < 1000µs",
            elapsed.as_micros()
        );
    }

    // -- Nested layout --

    #[test]
    fn nested_row_in_column() {
        let mut tree = LayoutTree::new();
        let root = tree.add_node(LayoutNode {
            width: Size::Fixed(400.0),
            height: Size::Fixed(200.0),
            flex_direction: FlexDirection::Column,
            ..Default::default()
        });
        let row = tree.add_node(LayoutNode {
            width: Size::Fixed(400.0),
            height: Size::Fixed(100.0),
            flex_direction: FlexDirection::Row,
            ..Default::default()
        });
        let c0 = tree.add_node(LayoutNode {
            width: Size::Fixed(200.0),
            height: Size::Fixed(100.0),
            ..Default::default()
        });
        let c1 = tree.add_node(LayoutNode {
            width: Size::Fixed(200.0),
            height: Size::Fixed(100.0),
            ..Default::default()
        });
        tree.add_child(root, row);
        tree.add_child(row, c0);
        tree.add_child(row, c1);
        tree.compute(root, 400.0, 200.0);

        assert!(approx(tree.get_layout(row).y, 0.0));
        assert!(approx(tree.get_layout(c0).x, 0.0));
        assert!(approx(tree.get_layout(c1).x, 200.0));
    }
}
