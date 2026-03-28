//! Integration test: ECS stress test.
//!
//! - 100K entity spawn/query/despawn throughput
//! - Measure time for each phase
//! - Assert reasonable performance (spawn 100K in <1s, query in <100ms, despawn <1s)

use arachne_ecs::World;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq)]
struct Position {
    x: f32,
    y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Velocity {
    x: f32,
    y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Health(i32);

/// Spawn 100K entities, query them all, then despawn them all.
/// Measures and asserts timing for each phase.
#[test]
fn ecs_stress_100k_spawn_query_despawn() {
    let count = 100_000u32;
    let mut world = World::new();

    // Phase 1: Spawn 100K entities with 3 components each.
    let spawn_start = Instant::now();
    let mut entities = Vec::with_capacity(count as usize);
    for i in 0..count {
        let e = world.spawn((
            Position {
                x: i as f32,
                y: i as f32 * 2.0,
            },
            Velocity {
                x: i as f32 * 0.1,
                y: i as f32 * 0.2,
            },
            Health(100),
        ));
        entities.push(e);
    }
    let spawn_elapsed = spawn_start.elapsed();
    eprintln!(
        "Spawn 100K entities: {:.2}ms ({:.0} entities/sec)",
        spawn_elapsed.as_secs_f64() * 1000.0,
        count as f64 / spawn_elapsed.as_secs_f64()
    );
    assert!(
        spawn_elapsed.as_secs_f64() < 1.0,
        "Spawning 100K entities took {:.2}ms, expected <1000ms",
        spawn_elapsed.as_secs_f64() * 1000.0
    );

    assert_eq!(world.entity_count(), count);

    // Phase 2: Query all entities with Position + Velocity.
    let query_start = Instant::now();
    let mut query_count = 0u64;
    for (pos, vel) in world.query::<(&Position, &Velocity)>() {
        let _ = (
            std::hint::black_box(pos),
            std::hint::black_box(vel),
        );
        query_count += 1;
    }
    let query_elapsed = query_start.elapsed();
    eprintln!(
        "Query 100K entities: {:.2}ms ({:.0}M entities/sec)",
        query_elapsed.as_secs_f64() * 1000.0,
        query_count as f64 / query_elapsed.as_secs_f64() / 1_000_000.0
    );
    assert_eq!(query_count, count as u64);
    assert!(
        query_elapsed.as_secs_f64() < 0.1,
        "Querying 100K entities took {:.2}ms, expected <100ms",
        query_elapsed.as_secs_f64() * 1000.0
    );

    // Phase 3: Despawn all 100K entities.
    let despawn_start = Instant::now();
    for e in &entities {
        world.despawn(*e);
    }
    let despawn_elapsed = despawn_start.elapsed();
    eprintln!(
        "Despawn 100K entities: {:.2}ms ({:.0} entities/sec)",
        despawn_elapsed.as_secs_f64() * 1000.0,
        count as f64 / despawn_elapsed.as_secs_f64()
    );
    assert!(
        despawn_elapsed.as_secs_f64() < 1.0,
        "Despawning 100K entities took {:.2}ms, expected <1000ms",
        despawn_elapsed.as_secs_f64() * 1000.0
    );

    assert_eq!(world.entity_count(), 0, "Entity count must be 0 after despawn");
}

/// Mutable query iteration over 100K entities -- modifies velocities.
#[test]
fn ecs_stress_100k_mutable_query() {
    let count = 100_000u32;
    let mut world = World::new();

    for i in 0..count {
        world.spawn((
            Position {
                x: i as f32,
                y: 0.0,
            },
            Velocity { x: 1.0, y: 0.0 },
        ));
    }

    let start = Instant::now();
    for vel in world.query_mut::<&mut Velocity>() {
        vel.x += 1.0;
    }
    let elapsed = start.elapsed();

    eprintln!(
        "Mutable query 100K: {:.2}ms ({:.0}M entities/sec)",
        elapsed.as_secs_f64() * 1000.0,
        count as f64 / elapsed.as_secs_f64() / 1_000_000.0
    );
    assert!(
        elapsed.as_secs_f64() < 0.1,
        "Mutable query took {:.2}ms, expected <100ms",
        elapsed.as_secs_f64() * 1000.0
    );

    // Verify mutations persisted.
    let vels: Vec<_> = world.query::<&Velocity>().collect();
    assert_eq!(vels.len(), count as usize);
    assert!(
        vels.iter().all(|v| (v.x - 2.0).abs() < 1e-6),
        "All velocities should be 2.0 after mutation"
    );
}
