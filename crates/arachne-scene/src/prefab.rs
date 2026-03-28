use arachne_ecs::Entity;
use arachne_math::Transform;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::graph::SceneGraph;
use crate::transform_prop::TransformPropagation;

// ENTITY_COUNTER ------

static ENTITY_COUNTER: AtomicU32 = AtomicU32::new(1_000_000);

fn allocate_entity() -> Entity {
    let index = ENTITY_COUNTER.fetch_add(1, Ordering::Relaxed);
    Entity::from_raw(index, 0)
}

// PREFAB_ENTITY ------

#[derive(Clone, Debug)]
pub struct PrefabEntity {
    pub name: String,
    pub local_transform: Transform,
    pub children: Vec<usize>,
    pub asset_refs: Vec<String>,
}

impl PrefabEntity {
    #[inline]
    pub fn new(name: impl Into<String>, transform: Transform) -> Self {
        Self {
            name: name.into(),
            local_transform: transform,
            children: Vec::new(),
            asset_refs: Vec::new(),
        }
    }
}

// PREFAB ------

#[derive(Clone, Debug)]
pub struct Prefab {
    pub entities: Vec<PrefabEntity>,
    pub name: String,
}

impl Prefab {
    #[inline]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            entities: Vec::new(),
            name: name.into(),
        }
    }

    #[inline]
    pub fn add_entity(&mut self, entity: PrefabEntity) -> usize {
        let index = self.entities.len();
        self.entities.push(entity);
        index
    }
}

// INSTANTIATE ------

pub fn instantiate(
    prefab: &Prefab,
    graph: &mut SceneGraph,
    transforms: &mut TransformPropagation,
) -> Vec<Entity> {
    if prefab.entities.is_empty() {
        return Vec::new();
    }

    // Map each prefab index to a real entity
    let mut entity_map: Vec<Entity> = Vec::with_capacity(prefab.entities.len());
    for _ in 0..prefab.entities.len() {
        entity_map.push(allocate_entity());
    }

    // Set up transforms and parent-child relationships
    for (i, prefab_entity) in prefab.entities.iter().enumerate() {
        let entity = entity_map[i];
        transforms.set_local(entity, prefab_entity.local_transform);

        for &child_idx in &prefab_entity.children {
            if child_idx < entity_map.len() {
                let child_entity = entity_map[child_idx];
                graph.add_child(entity, child_entity);
            }
        }
    }

    entity_map
}

// TESTS ------

#[cfg(test)]
mod tests {
    use super::*;
    use arachne_math::Vec3;

    #[test]
    fn instantiate_prefab_correct_tree() {
        // Prefab:
        //   [0] root at (0,0,0) -> children [1, 2]
        //   [1] child_a at (1,0,0) -> children [3]
        //   [2] child_b at (0,1,0) -> children []
        //   [3] grandchild at (0,0,1) -> children []
        let mut prefab = Prefab::new("test_prefab");

        let mut root_e = PrefabEntity::new("root", Transform::IDENTITY);
        root_e.children = vec![1, 2];
        prefab.add_entity(root_e);

        let mut child_a = PrefabEntity::new("child_a", Transform::from_position(Vec3::new(1.0, 0.0, 0.0)));
        child_a.children = vec![3];
        prefab.add_entity(child_a);

        let child_b = PrefabEntity::new("child_b", Transform::from_position(Vec3::new(0.0, 1.0, 0.0)));
        prefab.add_entity(child_b);

        let grandchild = PrefabEntity::new("grandchild", Transform::from_position(Vec3::new(0.0, 0.0, 1.0)));
        prefab.add_entity(grandchild);

        let mut graph = SceneGraph::new();
        let mut tp = TransformPropagation::new();

        let entities = instantiate(&prefab, &mut graph, &mut tp);
        assert_eq!(entities.len(), 4);

        // Verify hierarchy
        let root = entities[0];
        let ca = entities[1];
        let cb = entities[2];
        let gc = entities[3];

        assert_eq!(graph.parent_of(root), None);
        assert_eq!(graph.parent_of(ca), Some(root));
        assert_eq!(graph.parent_of(cb), Some(root));
        assert_eq!(graph.parent_of(gc), Some(ca));

        let root_children = graph.children_of(root);
        assert_eq!(root_children.len(), 2);
        assert!(root_children.contains(&ca));
        assert!(root_children.contains(&cb));

        let ca_children = graph.children_of(ca);
        assert_eq!(ca_children, &[gc]);

        assert!(graph.children_of(cb).is_empty());
        assert!(graph.children_of(gc).is_empty());

        // Verify transforms were set
        assert!(tp.local_transform(root).is_some());
        assert!(tp.local_transform(ca).is_some());
        assert!(tp.local_transform(cb).is_some());
        assert!(tp.local_transform(gc).is_some());
    }

    #[test]
    fn instantiate_empty_prefab() {
        let prefab = Prefab::new("empty");
        let mut graph = SceneGraph::new();
        let mut tp = TransformPropagation::new();

        let entities = instantiate(&prefab, &mut graph, &mut tp);
        assert!(entities.is_empty());
    }

    #[test]
    fn instantiate_single_entity() {
        let mut prefab = Prefab::new("single");
        prefab.add_entity(PrefabEntity::new("only", Transform::from_position(Vec3::new(5.0, 0.0, 0.0))));

        let mut graph = SceneGraph::new();
        let mut tp = TransformPropagation::new();

        let entities = instantiate(&prefab, &mut graph, &mut tp);
        assert_eq!(entities.len(), 1);
        assert_eq!(graph.parent_of(entities[0]), None);
    }
}
