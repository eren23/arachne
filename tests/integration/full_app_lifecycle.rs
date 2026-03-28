//! Integration test: Full app lifecycle.
//!
//! - Start app, run 600 frames (10s at 60fps)
//! - Spawn and despawn 1000 entities with mixed components
//! - Verify zero leaks: entity count = 0 at end

use arachne_app::{
    App, DefaultPlugins, HeadlessRunner, Physics2dPlugin, PhysicsBody, PhysicsBodyState,
    Transform, Vec2, Vec3,
};
use arachne_physics::{Collider, PhysicsWorld, RigidBodyData};
use arachne_render::{Sprite, TextureHandle};

/// Run 600 frames (10 seconds at 60 fps), spawn 1000 mixed entities, despawn
/// them all, and verify the entity count returns to zero.
#[test]
fn full_app_lifecycle_600_frames_1000_entities() {
    let mut app = App::new();
    app.add_plugin(DefaultPlugins);
    app.add_plugin(Physics2dPlugin);
    app.set_runner(HeadlessRunner::new(1, 1.0 / 60.0));
    app.build_plugins();

    let mut entities = Vec::with_capacity(1000);

    // Spawn 500 sprite-only entities.
    for i in 0..500u32 {
        let e = app.world.spawn((
            Sprite::new(TextureHandle(i % 8)),
            Transform::from_position(Vec3::new(
                (i % 50) as f32,
                (i / 50) as f32,
                0.0,
            )),
        ));
        entities.push(e);
    }

    // Spawn 500 physics entities (dynamic bodies with colliders).
    for i in 0..500u32 {
        let handle = {
            let physics = app.world.get_resource_mut::<PhysicsWorld>();
            let body = RigidBodyData::new_dynamic(
                Vec2::new(i as f32 * 0.5, 20.0 + (i as f32 * 0.1)),
                1.0,
                1.0,
            );
            let h = physics.add_body(body);
            physics.set_collider(h, Collider::circle(0.5));
            h
        };
        let mut pb = PhysicsBody::dynamic(1.0, 1.0);
        pb.state = PhysicsBodyState::Active(handle);
        let e = app.world.spawn((
            pb,
            Transform::from_position(Vec3::new(
                i as f32 * 0.5,
                20.0 + (i as f32 * 0.1),
                0.0,
            )),
        ));
        entities.push(e);
    }

    assert_eq!(
        app.world.entity_count(),
        1000,
        "Should have 1000 entities after spawn"
    );

    // Run 600 frames (10 seconds at 60 fps).
    app.set_runner(HeadlessRunner::new(600, 1.0 / 60.0));
    app.run();

    // Verify all 1000 entities still alive after simulation.
    assert_eq!(
        app.world.entity_count(),
        1000,
        "All 1000 entities should still be alive after 600 frames"
    );

    // Despawn all entities.
    for e in &entities {
        app.world.despawn(*e);
    }

    // Verify zero leaks.
    assert_eq!(
        app.world.entity_count(),
        0,
        "Entity count must be 0 after despawning all entities"
    );

    // Run a few more frames to ensure no crash after full despawn.
    app.set_runner(HeadlessRunner::new(10, 1.0 / 60.0));
    app.run();

    assert_eq!(
        app.world.entity_count(),
        0,
        "Entity count must remain 0 after additional frames"
    );
}
