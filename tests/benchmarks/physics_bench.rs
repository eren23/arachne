//! Benchmark tests: Physics operations.
//!
//! - Broadphase pair checks (target: >=1M/sec)
//! - Narrowphase collision tests (target: >=500K/sec)
//! - Full physics step with 1000 bodies (target: <4ms)

use arachne_math::{Rect, Vec2};
use arachne_physics::world::PhysicsConfig;
use arachne_physics::{
    BodyHandle, Collider, ColliderShape, PhysicsWorld, RigidBodyData, SpatialHashGrid,
};
use arachne_physics::narrowphase::test_collision;
use std::hint::black_box;
use std::time::Instant;

/// Broadphase pair detection throughput.
/// Insert 1000 bodies into a spatial hash grid and query pairs.
/// Target: >= 1M pair checks/sec.
#[test]
fn bench_broadphase_pair_checks() {
    let body_count = 1000u32;
    let cell_size = 4.0;
    let iterations = 100u32;

    let mut grid = SpatialHashGrid::new(cell_size);

    // Pre-compute AABBs for 1000 bodies in a 2D grid pattern.
    let aabbs: Vec<(BodyHandle, Rect)> = (0..body_count)
        .map(|i| {
            let x = (i % 32) as f32 * 3.0;
            let y = (i / 32) as f32 * 3.0;
            let handle = BodyHandle(i);
            let aabb = Rect::new(
                Vec2::new(x - 0.5, y - 0.5),
                Vec2::new(x + 0.5, y + 0.5),
            );
            (handle, aabb)
        })
        .collect();

    let start = Instant::now();
    let mut total_pairs = 0u64;
    for _ in 0..iterations {
        grid.clear();
        for &(handle, aabb) in &aabbs {
            grid.insert(handle, aabb);
        }
        let pairs = grid.query_pairs();
        total_pairs += pairs.len() as u64;
        black_box(&pairs);
    }
    let elapsed = start.elapsed();

    // Total "pair checks" is the number of bodies inserted * iterations
    // (each insert is a broadphase check).
    let checks = (body_count as u64) * (iterations as u64);
    let checks_per_sec = checks as f64 / elapsed.as_secs_f64();
    eprintln!(
        "Broadphase: {} inserts + queries in {:.2}ms ({:.0}M checks/sec, avg {} pairs)",
        checks,
        elapsed.as_secs_f64() * 1000.0,
        checks_per_sec / 1_000_000.0,
        total_pairs / iterations as u64
    );
    assert!(
        checks_per_sec >= 1_000_000.0,
        "Broadphase rate {:.0}K/sec is below 1M/sec threshold",
        checks_per_sec / 1000.0
    );
}

/// Narrowphase collision test throughput (circle vs circle).
/// Target: >= 500K/sec.
#[test]
fn bench_narrowphase_circle_circle() {
    let iterations = 500_000u64;
    let shape_a = ColliderShape::Circle { radius: 0.5 };
    let shape_b = ColliderShape::Circle { radius: 0.5 };
    let offset = Vec2::ZERO;

    let start = Instant::now();
    let mut hit_count = 0u64;
    for i in 0..iterations {
        let separation = 0.8 + (i as f32 * 0.0000001);
        let pos_a = Vec2::new(0.0, 0.0);
        let pos_b = Vec2::new(separation, 0.0);

        let result = test_collision(
            BodyHandle(0),
            &shape_a,
            pos_a,
            0.0,
            offset,
            BodyHandle(1),
            &shape_b,
            pos_b,
            0.0,
            offset,
        );
        if result.is_some() {
            hit_count += 1;
        }
    }
    let elapsed = start.elapsed();
    let _ = black_box(hit_count);

    let rate = iterations as f64 / elapsed.as_secs_f64();
    eprintln!(
        "Narrowphase circle-circle: {} tests in {:.2}ms ({:.0}M/sec, {} hits)",
        iterations,
        elapsed.as_secs_f64() * 1000.0,
        rate / 1_000_000.0,
        hit_count
    );
    assert!(
        rate >= 500_000.0,
        "Narrowphase rate {:.0}K/sec is below 500K/sec threshold",
        rate / 1000.0
    );
}

/// Narrowphase collision test throughput (AABB vs AABB).
/// Target: >= 500K/sec.
#[test]
fn bench_narrowphase_aabb_aabb() {
    let iterations = 500_000u64;
    let shape_a = ColliderShape::AABB {
        half_extents: Vec2::new(0.5, 0.5),
    };
    let shape_b = ColliderShape::AABB {
        half_extents: Vec2::new(0.5, 0.5),
    };
    let offset = Vec2::ZERO;

    let start = Instant::now();
    let mut hit_count = 0u64;
    for i in 0..iterations {
        let separation = 0.8 + (i as f32 * 0.0000001);
        let pos_a = Vec2::ZERO;
        let pos_b = Vec2::new(separation, 0.0);

        let result = test_collision(
            BodyHandle(0),
            &shape_a,
            pos_a,
            0.0,
            offset,
            BodyHandle(1),
            &shape_b,
            pos_b,
            0.0,
            offset,
        );
        if result.is_some() {
            hit_count += 1;
        }
    }
    let elapsed = start.elapsed();
    let _ = black_box(hit_count);

    let rate = iterations as f64 / elapsed.as_secs_f64();
    eprintln!(
        "Narrowphase AABB-AABB: {} tests in {:.2}ms ({:.0}M/sec, {} hits)",
        iterations,
        elapsed.as_secs_f64() * 1000.0,
        rate / 1_000_000.0,
        hit_count
    );
    assert!(
        rate >= 500_000.0,
        "Narrowphase AABB rate {:.0}K/sec is below 500K/sec threshold",
        rate / 1000.0
    );
}

/// Full physics step with 1000 bodies.
/// Target: < 4ms per step.
#[test]
fn bench_full_physics_step_1000_bodies() {
    let config = PhysicsConfig {
        gravity: Vec2::new(0.0, -9.81),
        solver_iterations: 8,
        fixed_timestep: 1.0 / 60.0,
        broadphase_cell_size: 4.0,
    };
    let mut world = PhysicsWorld::new(config);

    // Static ground.
    let ground = world.add_body(RigidBodyData::new_static(Vec2::new(0.0, -10.0)));
    world.set_collider(ground, Collider::aabb(Vec2::new(500.0, 10.0)));

    // 1000 dynamic bodies scattered.
    for i in 0..1000u32 {
        let x = (i % 32) as f32 * 3.0;
        let y = 5.0 + (i / 32) as f32 * 3.0;
        let body = RigidBodyData::new_dynamic(Vec2::new(x, y), 1.0, 1.0);
        let h = world.add_body(body);
        world.set_collider(h, Collider::circle(0.5));
    }

    // Warm up with one step.
    world.step(1.0 / 60.0);

    // Measure 60 steps.
    let steps = 60u32;
    let start = Instant::now();
    for _ in 0..steps {
        world.step(1.0 / 60.0);
    }
    let elapsed = start.elapsed();

    let avg_ms = elapsed.as_secs_f64() * 1000.0 / steps as f64;
    eprintln!(
        "Full physics step (1000 bodies): {:.2}ms avg ({} steps in {:.2}ms)",
        avg_ms,
        steps,
        elapsed.as_secs_f64() * 1000.0
    );
    assert!(
        avg_ms < 4.0,
        "Physics step avg {:.2}ms exceeds 4ms budget",
        avg_ms
    );
}
