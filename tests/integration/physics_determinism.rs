//! Integration test: Physics determinism.
//!
//! Run a 100-frame physics simulation twice with identical inputs.
//! Assert bit-identical final state (positions, velocities).

use arachne_math::Vec2;
use arachne_physics::world::PhysicsConfig;
use arachne_physics::{Collider, PhysicsWorld, RigidBodyData};

/// Sets up a deterministic physics world with several bodies and runs it for
/// `frame_count` frames using a fixed timestep.  Returns the final positions
/// and velocities of all bodies.
fn run_physics_sim(frame_count: usize) -> Vec<(Vec2, Vec2, f32, f32)> {
    let config = PhysicsConfig {
        gravity: Vec2::new(0.0, -9.81),
        solver_iterations: 8,
        fixed_timestep: 1.0 / 60.0,
        broadphase_cell_size: 4.0,
    };
    let mut world = PhysicsWorld::new(config);

    // Add a static ground body at y = 0.
    let ground = world.add_body(RigidBodyData::new_static(Vec2::new(0.0, -5.0)));
    world.set_collider(ground, Collider::aabb(Vec2::new(100.0, 5.0)));

    // Add 10 dynamic bodies at varying positions.
    for i in 0..10 {
        let body = RigidBodyData::new_dynamic(
            Vec2::new(i as f32 * 2.0, 10.0 + i as f32 * 0.5),
            1.0,
            1.0,
        );
        let handle = world.add_body(body);
        world.set_collider(handle, Collider::circle(0.5));
    }

    // Step the simulation frame_count times using fixed dt.
    let dt = 1.0 / 60.0;
    for _ in 0..frame_count {
        world.step(dt);
    }

    // Collect final state.
    world
        .bodies
        .iter()
        .map(|b| (b.position, b.linear_velocity, b.rotation, b.angular_velocity))
        .collect()
}

/// Run the same simulation twice and verify bit-identical results.
#[test]
fn physics_determinism_100_frames() {
    let run1 = run_physics_sim(100);
    let run2 = run_physics_sim(100);

    assert_eq!(
        run1.len(),
        run2.len(),
        "Body count mismatch between runs"
    );

    for (i, (state1, state2)) in run1.iter().zip(run2.iter()).enumerate() {
        // Bit-identical comparison on positions.
        assert_eq!(
            state1.0.x.to_bits(),
            state2.0.x.to_bits(),
            "Body {}: position.x differs (run1={}, run2={})",
            i,
            state1.0.x,
            state2.0.x
        );
        assert_eq!(
            state1.0.y.to_bits(),
            state2.0.y.to_bits(),
            "Body {}: position.y differs (run1={}, run2={})",
            i,
            state1.0.y,
            state2.0.y
        );

        // Bit-identical comparison on velocities.
        assert_eq!(
            state1.1.x.to_bits(),
            state2.1.x.to_bits(),
            "Body {}: velocity.x differs (run1={}, run2={})",
            i,
            state1.1.x,
            state2.1.x
        );
        assert_eq!(
            state1.1.y.to_bits(),
            state2.1.y.to_bits(),
            "Body {}: velocity.y differs (run1={}, run2={})",
            i,
            state1.1.y,
            state2.1.y
        );

        // Bit-identical comparison on rotation and angular velocity.
        assert_eq!(
            state1.2.to_bits(),
            state2.2.to_bits(),
            "Body {}: rotation differs (run1={}, run2={})",
            i,
            state1.2,
            state2.2
        );
        assert_eq!(
            state1.3.to_bits(),
            state2.3.to_bits(),
            "Body {}: angular_velocity differs (run1={}, run2={})",
            i,
            state1.3,
            state2.3
        );
    }
}

/// Verify that physics is deterministic even with more bodies and longer sim.
#[test]
fn physics_determinism_300_frames_20_bodies() {
    fn run(frame_count: usize) -> Vec<(Vec2, Vec2)> {
        let config = PhysicsConfig {
            gravity: Vec2::new(0.0, -9.81),
            solver_iterations: 8,
            fixed_timestep: 1.0 / 60.0,
            broadphase_cell_size: 4.0,
        };
        let mut world = PhysicsWorld::new(config);

        // Ground.
        let ground = world.add_body(RigidBodyData::new_static(Vec2::new(0.0, -10.0)));
        world.set_collider(ground, Collider::aabb(Vec2::new(200.0, 10.0)));

        // 20 dynamic bodies in a grid pattern.
        for row in 0..4 {
            for col in 0..5 {
                let body = RigidBodyData::new_dynamic(
                    Vec2::new(col as f32 * 3.0, 5.0 + row as f32 * 3.0),
                    1.0 + (row * 5 + col) as f32 * 0.1,
                    1.0,
                );
                let h = world.add_body(body);
                world.set_collider(h, Collider::circle(0.4 + col as f32 * 0.05));
            }
        }

        let dt = 1.0 / 60.0;
        for _ in 0..frame_count {
            world.step(dt);
        }

        world
            .bodies
            .iter()
            .map(|b| (b.position, b.linear_velocity))
            .collect()
    }

    let r1 = run(300);
    let r2 = run(300);

    for (i, (s1, s2)) in r1.iter().zip(r2.iter()).enumerate() {
        assert_eq!(
            s1.0.x.to_bits(),
            s2.0.x.to_bits(),
            "Body {} position.x mismatch",
            i
        );
        assert_eq!(
            s1.0.y.to_bits(),
            s2.0.y.to_bits(),
            "Body {} position.y mismatch",
            i
        );
        assert_eq!(
            s1.1.x.to_bits(),
            s2.1.x.to_bits(),
            "Body {} velocity.x mismatch",
            i
        );
        assert_eq!(
            s1.1.y.to_bits(),
            s2.1.y.to_bits(),
            "Body {} velocity.y mismatch",
            i
        );
    }
}
