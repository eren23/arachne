//! Integration test: Scene serialization roundtrip.
//!
//! - Create 100-entity scene data, serialize to JSON, deserialize, compare
//! - All component data should be identical after roundtrip

use arachne_ecs::Entity;
use arachne_math::{Quat, Transform, Vec3};
use arachne_scene::{
    ComponentRegistry, SceneGraph, TransformPropagation,
    serialize_scene, deserialize_scene, scene_to_json, scene_from_json,
};

fn e(index: u32) -> Entity {
    Entity::from_raw(index, 0)
}

/// Create 100 entities with varying transforms, serialize to JSON and back,
/// verify all data matches.
#[test]
fn scene_roundtrip_100_entities_json() {
    let mut graph = SceneGraph::new();
    let mut tp = TransformPropagation::new();
    let registry = ComponentRegistry::new();

    let mut entities = Vec::with_capacity(100);
    for i in 0..100u32 {
        let entity = e(i);
        entities.push(entity);

        // Vary transforms to catch precision issues.
        let transform = Transform::new(
            Vec3::new(
                i as f32 * 1.5,
                (i as f32).sin() * 10.0,
                (i as f32) * 0.01,
            ),
            Quat::from_axis_angle(Vec3::Y, i as f32 * 0.1),
            Vec3::new(
                1.0 + (i as f32) * 0.01,
                1.0 + (i as f32) * 0.02,
                1.0,
            ),
        );
        tp.set_local(entity, transform);

        // Create a parent-child chain: entity i is child of entity i-1.
        if i > 0 {
            graph.add_child(e(i - 1), entity);
        }
    }

    // Serialize to SerializedScene.
    let scene = serialize_scene(&graph, &tp, &entities, &registry);
    assert_eq!(scene.entities.len(), 100, "Scene should have 100 entities");

    // Convert to JSON string.
    let json = scene_to_json(&scene);
    assert!(!json.is_empty(), "JSON output should not be empty");

    // Parse JSON back.
    let parsed = scene_from_json(&json).expect("JSON parsing should succeed");
    assert_eq!(
        parsed.entities.len(),
        100,
        "Parsed scene should have 100 entities"
    );

    // Verify structural equality of serialized data.
    for (orig, parsed_e) in scene.entities.iter().zip(parsed.entities.iter()) {
        assert_eq!(orig.id, parsed_e.id, "Entity ID mismatch");
        assert_eq!(orig.parent, parsed_e.parent, "Parent mismatch for entity {}", orig.id);
        assert_eq!(
            orig.components.len(),
            parsed_e.components.len(),
            "Component count mismatch for entity {}",
            orig.id
        );
        for (oc, pc) in orig.components.iter().zip(parsed_e.components.iter()) {
            assert_eq!(oc.name, pc.name, "Component name mismatch");
            assert_eq!(oc.data, pc.data, "Component data mismatch for {}", oc.name);
        }
    }

    // Deserialize into a fresh scene graph and transform propagation.
    let mut graph2 = SceneGraph::new();
    let mut tp2 = TransformPropagation::new();
    let deserialized = deserialize_scene(&parsed, &mut graph2, &mut tp2, &registry);
    assert_eq!(
        deserialized.len(),
        100,
        "Deserialized entity count mismatch"
    );

    // Verify parent-child relationships.
    assert_eq!(graph2.parent_of(e(0)), None, "Root should have no parent");
    for i in 1..100u32 {
        assert_eq!(
            graph2.parent_of(e(i)),
            Some(e(i - 1)),
            "Entity {} should have parent {}",
            i,
            i - 1
        );
    }

    // Verify transform data roundtrip with epsilon tolerance.
    let epsilon = 1e-5;
    for i in 0..100u32 {
        let entity = e(i);
        let orig = tp.local_transform(entity).expect("original transform");
        let rt = tp2.local_transform(entity).expect("roundtripped transform");

        assert!(
            (orig.position.x - rt.position.x).abs() < epsilon,
            "Entity {} position.x: {} vs {}",
            i,
            orig.position.x,
            rt.position.x
        );
        assert!(
            (orig.position.y - rt.position.y).abs() < epsilon,
            "Entity {} position.y: {} vs {}",
            i,
            orig.position.y,
            rt.position.y
        );
        assert!(
            (orig.position.z - rt.position.z).abs() < epsilon,
            "Entity {} position.z: {} vs {}",
            i,
            orig.position.z,
            rt.position.z
        );

        assert!(
            (orig.rotation.x - rt.rotation.x).abs() < epsilon,
            "Entity {} rotation.x: {} vs {}",
            i,
            orig.rotation.x,
            rt.rotation.x
        );
        assert!(
            (orig.rotation.y - rt.rotation.y).abs() < epsilon,
            "Entity {} rotation.y: {} vs {}",
            i,
            orig.rotation.y,
            rt.rotation.y
        );
        assert!(
            (orig.rotation.z - rt.rotation.z).abs() < epsilon,
            "Entity {} rotation.z: {} vs {}",
            i,
            orig.rotation.z,
            rt.rotation.z
        );
        assert!(
            (orig.rotation.w - rt.rotation.w).abs() < epsilon,
            "Entity {} rotation.w: {} vs {}",
            i,
            orig.rotation.w,
            rt.rotation.w
        );

        assert!(
            (orig.scale.x - rt.scale.x).abs() < epsilon,
            "Entity {} scale.x: {} vs {}",
            i,
            orig.scale.x,
            rt.scale.x
        );
        assert!(
            (orig.scale.y - rt.scale.y).abs() < epsilon,
            "Entity {} scale.y: {} vs {}",
            i,
            orig.scale.y,
            rt.scale.y
        );
        assert!(
            (orig.scale.z - rt.scale.z).abs() < epsilon,
            "Entity {} scale.z: {} vs {}",
            i,
            orig.scale.z,
            rt.scale.z
        );
    }
}

/// Roundtrip an empty scene -- edge case.
#[test]
fn scene_roundtrip_empty() {
    let graph = SceneGraph::new();
    let tp = TransformPropagation::new();
    let registry = ComponentRegistry::new();
    let entities: Vec<Entity> = Vec::new();

    let scene = serialize_scene(&graph, &tp, &entities, &registry);
    assert!(scene.entities.is_empty());

    let json = scene_to_json(&scene);
    let parsed = scene_from_json(&json).unwrap();
    assert!(parsed.entities.is_empty());
}
