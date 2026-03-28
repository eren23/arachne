//! Benchmark tests: ECS operations.
//!
//! - Entity spawn with 3 components (target: >=500K/sec)
//! - Query iteration over 1M entities (target: >=10M entities/sec)
//! - Archetype move via insert_component (target: >=200K/sec)

use arachne_ecs::World;
use std::hint::black_box;
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

#[derive(Debug, Clone, Copy, PartialEq)]
struct Marker;

/// Entity spawn throughput with 3 components.
/// Target: >= 500K/sec.
#[test]
fn bench_entity_spawn_3_components() {
    let count = 500_000u32;
    let mut world = World::new();

    let start = Instant::now();
    for i in 0..count {
        world.spawn((
            Position {
                x: i as f32,
                y: i as f32,
            },
            Velocity { x: 1.0, y: 0.0 },
            Health(100),
        ));
    }
    let elapsed = start.elapsed();

    let rate = count as f64 / elapsed.as_secs_f64();
    eprintln!(
        "Spawn {} entities (3 components): {:.2}ms ({:.0}K entities/sec)",
        count,
        elapsed.as_secs_f64() * 1000.0,
        rate / 1000.0
    );
    assert!(
        rate >= 500_000.0,
        "Entity spawn rate {:.0}K/sec is below 500K/sec threshold",
        rate / 1000.0
    );
}

/// Query iteration over 1M entities.
/// Target: >= 10M entities/sec.
#[test]
fn bench_query_iteration_1m() {
    let count = 1_000_000u32;
    let mut world = World::new();

    // Pre-spawn entities (not timed).
    for i in 0..count {
        world.spawn((
            Position {
                x: i as f32,
                y: 0.0,
            },
            Velocity { x: 1.0, y: 0.0 },
        ));
    }

    // Timed: iterate all entities and access two components.
    let start = Instant::now();
    let mut total = 0u64;
    for (pos, vel) in world.query::<(&Position, &Velocity)>() {
        let _ = black_box(pos);
        let _ = black_box(vel);
        total += 1;
    }
    let elapsed = start.elapsed();
    let _ = black_box(total);

    assert_eq!(total, count as u64);

    let rate = total as f64 / elapsed.as_secs_f64();
    eprintln!(
        "Query iterate {} entities (2 components): {:.2}ms ({:.0}M entities/sec)",
        count,
        elapsed.as_secs_f64() * 1000.0,
        rate / 1_000_000.0
    );
    assert!(
        rate >= 10_000_000.0,
        "Query iteration rate {:.0}M/sec is below 10M/sec threshold",
        rate / 1_000_000.0
    );
}

/// Archetype move throughput via insert_component.
/// Target: >= 200K/sec.
#[test]
fn bench_archetype_move() {
    let count = 200_000u32;
    let mut world = World::new();

    // Spawn entities with Position only.
    let mut entities = Vec::with_capacity(count as usize);
    for i in 0..count {
        let e = world.spawn((Position {
            x: i as f32,
            y: 0.0,
        },));
        entities.push(e);
    }

    // Timed: insert a new component on each entity (triggers archetype move).
    let start = Instant::now();
    for e in &entities {
        world.insert_component(*e, Marker);
    }
    let elapsed = start.elapsed();

    let rate = count as f64 / elapsed.as_secs_f64();
    eprintln!(
        "Archetype move {} entities: {:.2}ms ({:.0}K moves/sec)",
        count,
        elapsed.as_secs_f64() * 1000.0,
        rate / 1000.0
    );
    assert!(
        rate >= 200_000.0,
        "Archetype move rate {:.0}K/sec is below 200K/sec threshold",
        rate / 1000.0
    );

    // Verify all entities now have both components.
    let query_count = world.query::<(&Position, &Marker)>().count();
    assert_eq!(query_count, count as usize);
}

/// Mutable query iteration over 1M entities.
/// Target: >= 10M entities/sec.
#[test]
fn bench_query_mut_iteration_1m() {
    let count = 1_000_000u32;
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
    let mut total = 0u64;
    for vel in world.query_mut::<&mut Velocity>() {
        vel.x += 0.1;
        total += 1;
    }
    let elapsed = start.elapsed();
    let _ = black_box(total);

    assert_eq!(total, count as u64);

    let rate = total as f64 / elapsed.as_secs_f64();
    eprintln!(
        "Mutable query iterate {} entities: {:.2}ms ({:.0}M entities/sec)",
        count,
        elapsed.as_secs_f64() * 1000.0,
        rate / 1_000_000.0
    );
    assert!(
        rate >= 10_000_000.0,
        "Mutable query iteration rate {:.0}M/sec is below 10M/sec threshold",
        rate / 1_000_000.0
    );
}
