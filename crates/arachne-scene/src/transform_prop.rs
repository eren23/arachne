use arachne_ecs::Entity;
use arachne_math::{Mat4, Transform};

use crate::graph::SceneGraph;

// GLOBAL_TRANSFORM ------

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlobalTransform {
    pub matrix: Mat4,
}

impl GlobalTransform {
    #[inline]
    pub fn new(matrix: Mat4) -> Self {
        Self { matrix }
    }

    #[inline]
    pub fn identity() -> Self {
        Self { matrix: Mat4::IDENTITY }
    }
}

// TRANSFORM_PROPAGATION ------

pub struct TransformPropagation {
    globals: Vec<Mat4>,
    locals: Vec<Transform>,
    change_ticks: Vec<u32>,
    propagated_ticks: Vec<u32>,
    subtree_dirty: Vec<bool>,
    initialized: Vec<bool>,
    current_tick: u32,
    len: usize,
}

impl TransformPropagation {
    #[inline]
    pub fn new() -> Self {
        Self {
            globals: Vec::new(),
            locals: Vec::new(),
            change_ticks: Vec::new(),
            propagated_ticks: Vec::new(),
            subtree_dirty: Vec::new(),
            initialized: Vec::new(),
            current_tick: 0,
            len: 0,
        }
    }

    fn ensure_capacity(&mut self, index: usize) {
        if index >= self.len {
            let new_len = index + 1;
            self.globals.resize(new_len, Mat4::IDENTITY);
            self.locals.resize(new_len, Transform::IDENTITY);
            self.change_ticks.resize(new_len, 0);
            self.propagated_ticks.resize(new_len, 0);
            self.subtree_dirty.resize(new_len, false);
            self.initialized.resize(new_len, false);
            self.len = new_len;
        }
    }

    pub fn set_local(&mut self, entity: Entity, transform: Transform) {
        let idx = entity.index() as usize;
        self.ensure_capacity(idx);
        self.locals[idx] = transform;
        self.current_tick += 1;
        self.change_ticks[idx] = self.current_tick;
        self.initialized[idx] = true;
    }

    pub fn mark_dirty_ancestors(&mut self, entity: Entity, graph: &SceneGraph) {
        let mut current = graph.parent_of(entity);
        while let Some(ancestor) = current {
            let ai = ancestor.index() as usize;
            self.ensure_capacity(ai);
            if self.subtree_dirty[ai] {
                break;
            }
            self.subtree_dirty[ai] = true;
            current = graph.parent_of(ancestor);
        }
    }

    pub fn set_local_with_graph(
        &mut self,
        entity: Entity,
        transform: Transform,
        graph: &SceneGraph,
    ) {
        self.set_local(entity, transform);
        self.mark_dirty_ancestors(entity, graph);
    }

    #[inline]
    pub fn global_transform(&self, entity: Entity) -> Option<Mat4> {
        let idx = entity.index() as usize;
        if idx < self.len && self.initialized[idx] {
            Some(self.globals[idx])
        } else {
            None
        }
    }

    #[inline]
    pub fn local_transform(&self, entity: Entity) -> Option<Transform> {
        let idx = entity.index() as usize;
        if idx < self.len && self.initialized[idx] {
            Some(self.locals[idx])
        } else {
            None
        }
    }

    pub fn propagate(&mut self, graph: &SceneGraph) -> usize {
        let roots = graph.roots();
        let mut count = 0;
        for root in roots {
            count += self.propagate_entity(root, Mat4::IDENTITY, false, graph);
        }
        count
    }

    fn propagate_entity(
        &mut self,
        entity: Entity,
        parent_global: Mat4,
        parent_recomputed: bool,
        graph: &SceneGraph,
    ) -> usize {
        let idx = entity.index() as usize;
        if idx >= self.len {
            return 0;
        }

        let is_dirty = self.change_ticks[idx] > self.propagated_ticks[idx];
        let needs_recompute = is_dirty || parent_recomputed;

        let global;
        let mut count;

        if needs_recompute {
            global = parent_global.mul_mat4(self.locals[idx].local_to_world());
            self.globals[idx] = global;
            self.propagated_ticks[idx] = self.current_tick;
            count = 1;
        } else {
            global = self.globals[idx];
            count = 0;
        }

        if needs_recompute || self.subtree_dirty[idx] {
            let children = graph.children_of(entity);
            for i in 0..children.len() {
                let child = children[i];
                count += self.propagate_entity(child, global, needs_recompute, graph);
            }
            self.subtree_dirty[idx] = false;
        }

        count
    }

    pub fn register_entity(&mut self, entity: Entity, transform: Transform) {
        let idx = entity.index() as usize;
        self.ensure_capacity(idx);
        self.locals[idx] = transform;
        self.globals[idx] = transform.local_to_world();
        self.change_ticks[idx] = 0;
        self.propagated_ticks[idx] = 0;
        self.initialized[idx] = true;
    }
}

impl Default for TransformPropagation {
    fn default() -> Self {
        Self::new()
    }
}

// TESTS ------

#[cfg(test)]
mod tests {
    use super::*;
    use arachne_math::{Quat, Vec3};

    const EPSILON: f32 = 1e-5;

    fn e(index: u32) -> Entity {
        Entity::from_raw(index, 0)
    }

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < EPSILON
    }

    fn assert_vec3_approx(a: Vec3, b: Vec3) {
        assert!(
            approx_eq(a.x, b.x) && approx_eq(a.y, b.y) && approx_eq(a.z, b.z),
            "Vec3 mismatch: {:?} != {:?}",
            a, b,
        );
    }

    fn global_position(tp: &TransformPropagation, entity: Entity) -> Vec3 {
        let m = tp.global_transform(entity).unwrap();
        Vec3::new(m.cols[3][0], m.cols[3][1], m.cols[3][2])
    }

    #[test]
    fn parent_child_translation() {
        let mut graph = SceneGraph::new();
        let parent = e(0);
        let child = e(1);
        graph.add_child(parent, child);

        let mut tp = TransformPropagation::new();
        tp.set_local_with_graph(parent, Transform::from_position(Vec3::new(10.0, 0.0, 0.0)), &graph);
        tp.set_local_with_graph(child, Transform::from_position(Vec3::new(5.0, 0.0, 0.0)), &graph);

        tp.propagate(&graph);

        assert_vec3_approx(global_position(&tp, parent), Vec3::new(10.0, 0.0, 0.0));
        assert_vec3_approx(global_position(&tp, child), Vec3::new(15.0, 0.0, 0.0));
    }

    #[test]
    fn rotate_parent_90deg_y() {
        let mut graph = SceneGraph::new();
        let parent = e(0);
        let child = e(1);
        graph.add_child(parent, child);

        let rotation = Quat::from_axis_angle(Vec3::Y, core::f32::consts::FRAC_PI_2);

        let mut tp = TransformPropagation::new();
        tp.set_local_with_graph(parent, Transform::from_rotation(rotation), &graph);
        tp.set_local_with_graph(child, Transform::from_position(Vec3::new(5.0, 0.0, 0.0)), &graph);

        tp.propagate(&graph);

        let child_global = global_position(&tp, child);
        assert_vec3_approx(child_global, Vec3::new(0.0, 0.0, -5.0));
    }

    #[test]
    fn dirty_flags_10k_tree_modify_one_leaf() {
        let mut graph = SceneGraph::new();
        let mut tp = TransformPropagation::new();

        let chain_len = 100u32;
        let total = 10_000u32;

        // Build a chain: 0 -> 1 -> 2 -> ... -> 99
        for i in 0..chain_len {
            let entity = e(i);
            tp.register_entity(entity, Transform::IDENTITY);
            if i > 0 {
                graph.add_child(e(i - 1), entity);
            }
        }

        // Add sibling leaves to node 0
        for i in chain_len..total {
            let entity = e(i);
            tp.register_entity(entity, Transform::IDENTITY);
            graph.add_child(e(0), entity);
        }

        // Initial propagation
        tp.propagate(&graph);

        // Modify just 1 leaf at the end of the chain
        let leaf = e(chain_len - 1);
        tp.set_local_with_graph(leaf, Transform::from_position(Vec3::new(1.0, 0.0, 0.0)), &graph);

        let count = tp.propagate(&graph);

        assert!(
            count < 10,
            "Expected < 10 recomputations, got {}",
            count,
        );
    }

    #[test]
    fn bench_propagation_10k_nodes() {
        let mut graph = SceneGraph::new();
        let mut tp = TransformPropagation::new();
        let n = 10_000u32;

        // Build a balanced-ish tree: each node's parent is (index-1) / 10
        for i in 0..n {
            let entity = e(i);
            tp.register_entity(entity, Transform::from_position(Vec3::new(0.01, 0.0, 0.0)));
            if i > 0 {
                let parent_idx = (i - 1) / 10;
                graph.add_child(e(parent_idx), entity);
            }
        }

        // Mark all dirty for full propagation
        for i in 0..n {
            tp.set_local_with_graph(e(i), Transform::from_position(Vec3::new(0.01, 0.0, 0.0)), &graph);
        }

        let start = std::time::Instant::now();
        let count = tp.propagate(&graph);
        let elapsed = start.elapsed();

        assert_eq!(count, n as usize);
        let ms = elapsed.as_secs_f64() * 1000.0;
        eprintln!("Transform propagation 10K nodes: {:.3}ms ({} recomputations)", ms, count);
        assert!(
            ms < 500.0,
            "Transform propagation took {:.3}ms, expected < 500ms",
            ms,
        );
    }
}
