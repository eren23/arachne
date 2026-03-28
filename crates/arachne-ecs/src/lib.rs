pub mod archetype;
pub mod bundle;
pub mod change_detection;
pub mod commands;
pub mod component;
pub mod entity;
pub mod event;
pub mod parallel;
pub mod query;
pub mod resource;
pub mod schedule;
pub mod system;
pub mod world;

pub use archetype::{ArchetypeId, EntityLocation};
pub use bundle::Bundle;
pub use change_detection::{Added, Changed, ChangedWithin, ChangeTrackers, ChangeTrackersItem, ComponentTicks, Or, Tick};
pub use commands::{CommandQueue, Commands, EntityCommands};
pub use component::{Component, ComponentId, ComponentInfo, ComponentRegistry};
pub use entity::{Entity, EntityAllocator};
pub use event::{EventQueue, EventReader, EventStorage, EventWriter};
pub use query::{QueryFilter, QueryIter, ReadOnlyWorldQuery, With, Without, WorldQuery};
pub use resource::{Res, ResMut, ResourceMap};
pub use schedule::{apply_deferred, Schedule, Stage};
pub use system::{FunctionSystem, IntoSystem, Query, System, SystemParam};
pub use world::World;
pub use parallel::{
    SystemAccessInfo, ThreadPool, ParallelScheduler,
    compute_parallel_batches, par_for_each, par_for_each_mut, par_map,
};

// ===========================================================================
// Integration tests
// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    // -- Test component types ------------------------------------------------

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

    #[derive(Debug, Clone, PartialEq)]
    struct Name(String);

    #[derive(Debug, Clone, Copy, PartialEq)]
    struct Dead;

    #[derive(Debug, Clone, Copy, PartialEq)]
    struct A(u32);
    #[derive(Debug, Clone, Copy, PartialEq)]
    struct B(u32);
    #[derive(Debug, Clone, Copy, PartialEq)]
    struct C(u32);

    // -- Archetype tests -----------------------------------------------------

    #[test]
    fn archetype_add_10k_entities_and_read_back() {
        let mut registry = ComponentRegistry::new();
        let pos_id = registry.get_or_register::<Position>();
        let vel_id = registry.get_or_register::<Velocity>();
        let mut ids = vec![pos_id, vel_id];
        ids.sort();
        let mut arch =
            archetype::Archetype::new(ArchetypeId(0), ids.clone(), &registry);

        for i in 0..10_000u32 {
            let pos = Position {
                x: i as f32,
                y: i as f32 * 2.0,
            };
            let vel = Velocity {
                x: i as f32 * 0.1,
                y: i as f32 * 0.2,
            };
            let entity = Entity::from_raw(i, 0);
            let pos_col = arch.column_index(pos_id).unwrap();
            let vel_col = arch.column_index(vel_id).unwrap();
            unsafe {
                arch.push_component_data(pos_col, &pos as *const Position as *const u8, 0);
                arch.push_component_data(vel_col, &vel as *const Velocity as *const u8, 0);
            }
            arch.push_entity(entity);
        }

        assert_eq!(arch.len(), 10_000);

        // Read back and verify.
        for i in 0..10_000u32 {
            let pos_col = arch.column_index(pos_id).unwrap();
            let ptr = arch.column_data_ptr(pos_col);
            let pos = unsafe { &*(ptr.add(i as usize * std::mem::size_of::<Position>()) as *const Position) };
            assert_eq!(pos.x, i as f32);
            assert_eq!(pos.y, i as f32 * 2.0);
        }
    }

    #[test]
    fn archetype_column_alignment() {
        #[repr(align(32))]
        #[derive(Clone, Copy, PartialEq, Debug)]
        struct BigAlign(u64);

        let mut registry = ComponentRegistry::new();
        let id = registry.get_or_register::<BigAlign>();
        let arch_id = ArchetypeId(0);
        let mut arch = archetype::Archetype::new(arch_id, vec![id], &registry);

        for i in 0..128u64 {
            let val = BigAlign(i);
            let col = arch.column_index(id).unwrap();
            unsafe { arch.push_component_data(col, &val as *const BigAlign as *const u8, 0) };
            arch.push_entity(Entity::from_raw(i as u32, 0));
        }

        let col = arch.column_index(id).unwrap();
        let base = arch.column_data_ptr(col) as usize;
        assert_eq!(base % 32, 0, "base pointer not 32-byte aligned");
        for i in 0..128usize {
            let ptr = base + i * std::mem::size_of::<BigAlign>();
            assert_eq!(ptr % 32, 0, "element {i} not aligned");
        }
    }

    #[test]
    fn archetype_edge_add_component() {
        let mut world = World::new();
        let e = world.spawn((Position { x: 1.0, y: 2.0 }, Velocity { x: 0.0, y: 0.0 }));
        world.insert_component(e, Health(100));

        assert_eq!(world.get::<Position>(e), Some(&Position { x: 1.0, y: 2.0 }));
        assert_eq!(world.get::<Velocity>(e), Some(&Velocity { x: 0.0, y: 0.0 }));
        assert_eq!(world.get::<Health>(e), Some(&Health(100)));
    }

    #[test]
    fn archetype_edge_remove_component() {
        let mut world = World::new();
        let e = world.spawn((Position { x: 1.0, y: 2.0 }, Velocity { x: 3.0, y: 4.0 }));
        let removed = world.remove_component::<Velocity>(e);
        assert_eq!(removed, Some(Velocity { x: 3.0, y: 4.0 }));
        assert_eq!(world.get::<Position>(e), Some(&Position { x: 1.0, y: 2.0 }));
        assert!(world.get::<Velocity>(e).is_none());
    }

    // -- World tests ---------------------------------------------------------

    #[test]
    fn world_spawn_different_component_sets() {
        let mut world = World::new();
        let e1 = world.spawn((Position { x: 0.0, y: 0.0 },));
        let e2 = world.spawn((Position { x: 1.0, y: 1.0 }, Velocity { x: 2.0, y: 2.0 }));
        let e3 = world.spawn((Position { x: 3.0, y: 3.0 }, Health(50)));
        let e4 = world.spawn((Velocity { x: 4.0, y: 4.0 }, Health(100)));
        let e5 = world.spawn((
            Position { x: 5.0, y: 5.0 },
            Velocity { x: 5.0, y: 5.0 },
            Health(200),
        ));

        assert_eq!(world.entity_count(), 5);
        assert_eq!(world.get::<Position>(e1), Some(&Position { x: 0.0, y: 0.0 }));
        assert_eq!(world.get::<Velocity>(e2), Some(&Velocity { x: 2.0, y: 2.0 }));
        assert_eq!(world.get::<Health>(e3), Some(&Health(50)));
        assert!(world.get::<Position>(e4).is_none());
        assert_eq!(world.get::<Health>(e5), Some(&Health(200)));
    }

    #[test]
    fn world_despawn() {
        let mut world = World::new();
        let e = world.spawn((Position { x: 1.0, y: 2.0 },));
        assert!(world.despawn(e));
        assert!(!world.is_alive(e));
        assert!(world.get::<Position>(e).is_none());
        // Double despawn returns false.
        assert!(!world.despawn(e));
    }

    #[test]
    fn world_get_mut() {
        let mut world = World::new();
        let e = world.spawn((Position { x: 0.0, y: 0.0 },));
        if let Some(pos) = world.get_mut::<Position>(e) {
            pos.x = 42.0;
        }
        assert_eq!(world.get::<Position>(e), Some(&Position { x: 42.0, y: 0.0 }));
    }

    #[test]
    fn world_insert_overwrite() {
        let mut world = World::new();
        let e = world.spawn((Health(50),));
        world.insert_component(e, Health(100));
        assert_eq!(world.get::<Health>(e), Some(&Health(100)));
    }

    // -- Query tests ---------------------------------------------------------

    #[test]
    fn query_position_velocity() {
        let mut world = World::new();
        world.spawn((Position { x: 0.0, y: 0.0 },)); // only Position
        let e2 = world.spawn((Position { x: 1.0, y: 1.0 }, Velocity { x: 2.0, y: 2.0 }));
        world.spawn((Velocity { x: 3.0, y: 3.0 },)); // only Velocity
        let e4 = world.spawn((
            Position { x: 4.0, y: 4.0 },
            Velocity { x: 5.0, y: 5.0 },
            Health(100),
        ));

        let results: Vec<_> = world.query::<(&Position, &Velocity)>().collect();
        assert_eq!(results.len(), 2);

        // Check both results are present (order depends on archetype iteration).
        let has_e2 = results
            .iter()
            .any(|(p, v)| *p == &Position { x: 1.0, y: 1.0 } && *v == &Velocity { x: 2.0, y: 2.0 });
        let has_e4 = results
            .iter()
            .any(|(p, v)| *p == &Position { x: 4.0, y: 4.0 } && *v == &Velocity { x: 5.0, y: 5.0 });
        assert!(has_e2, "missing entity e2; results: {results:?}");
        assert!(has_e4, "missing entity e4; results: {results:?}");
        let _ = (e2, e4); // suppress unused warnings
    }

    #[test]
    fn query_filter_without() {
        let mut world = World::new();
        let e1 = world.spawn((Position { x: 1.0, y: 1.0 },));
        let e2 = world.spawn((Position { x: 2.0, y: 2.0 }, Dead));

        let results: Vec<_> = world
            .query_filtered::<&Position, Without<Dead>>()
            .collect();
        assert_eq!(results.len(), 1);
        assert_eq!(*results[0], Position { x: 1.0, y: 1.0 });
        let _ = (e1, e2);
    }

    #[test]
    fn query_filter_with() {
        let mut world = World::new();
        world.spawn((Position { x: 1.0, y: 1.0 },));
        world.spawn((Position { x: 2.0, y: 2.0 }, Dead));

        let results: Vec<_> = world
            .query_filtered::<&Position, With<Dead>>()
            .collect();
        assert_eq!(results.len(), 1);
        assert_eq!(*results[0], Position { x: 2.0, y: 2.0 });
    }

    #[test]
    fn query_mut_velocity() {
        let mut world = World::new();
        world.spawn((Position { x: 0.0, y: 0.0 }, Velocity { x: 1.0, y: 0.0 }));
        world.spawn((Position { x: 0.0, y: 0.0 }, Velocity { x: 2.0, y: 0.0 }));

        // Mutate all velocities.
        for vel in world.query_mut::<&mut Velocity>() {
            vel.x += 10.0;
        }

        // Verify changes persisted.
        let vels: Vec<_> = world.query::<&Velocity>().collect();
        assert!(vels.iter().all(|v| v.x >= 11.0));
    }

    #[test]
    fn query_optional() {
        let mut world = World::new();
        let e1 = world.spawn((Position { x: 1.0, y: 1.0 }, Name("Alice".into())));
        let e2 = world.spawn((Position { x: 2.0, y: 2.0 },));

        let results: Vec<_> = world.query::<(&Position, Option<&Name>)>().collect();
        assert_eq!(results.len(), 2);

        let named: Vec<_> = results.iter().filter(|(_, n)| n.is_some()).collect();
        let unnamed: Vec<_> = results.iter().filter(|(_, n)| n.is_none()).collect();
        assert_eq!(named.len(), 1);
        assert_eq!(unnamed.len(), 1);
        assert_eq!(named[0].1.unwrap(), &Name("Alice".into()));
        let _ = (e1, e2);
    }

    #[test]
    fn query_entity_id() {
        let mut world = World::new();
        let e1 = world.spawn((Position { x: 1.0, y: 1.0 },));
        let e2 = world.spawn((Position { x: 2.0, y: 2.0 },));

        let results: Vec<_> = world.query::<(Entity, &Position)>().collect();
        assert_eq!(results.len(), 2);

        let ids: HashSet<Entity> = results.iter().map(|(e, _)| *e).collect();
        assert!(ids.contains(&e1));
        assert!(ids.contains(&e2));
    }

    #[test]
    fn query_single() {
        let mut world = World::new();
        world.spawn((Position { x: 42.0, y: 0.0 }, Dead));
        world.spawn((Position { x: 0.0, y: 0.0 },)); // not Dead

        let pos = world
            .query_filtered::<&Position, With<Dead>>()
            .single();
        assert_eq!(*pos, Position { x: 42.0, y: 0.0 });
    }

    // -- Drop / leak tests ---------------------------------------------------

    #[test]
    fn components_with_drop_called_on_despawn() {
        use std::sync::atomic::{AtomicU32, Ordering};
        static DROPS: AtomicU32 = AtomicU32::new(0);

        #[derive(Debug)]
        struct Tracked(u32);
        impl Drop for Tracked {
            fn drop(&mut self) {
                DROPS.fetch_add(1, Ordering::Relaxed);
            }
        }

        DROPS.store(0, Ordering::Relaxed);
        let mut world = World::new();
        let e1 = world.spawn((Tracked(1),));
        let e2 = world.spawn((Tracked(2),));
        let e3 = world.spawn((Tracked(3),));

        world.despawn(e1);
        assert_eq!(DROPS.load(Ordering::Relaxed), 1);
        world.despawn(e2);
        assert_eq!(DROPS.load(Ordering::Relaxed), 2);
        world.despawn(e3);
        assert_eq!(DROPS.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn components_with_drop_called_on_world_drop() {
        use std::sync::atomic::{AtomicU32, Ordering};
        static DROPS: AtomicU32 = AtomicU32::new(0);

        #[derive(Debug)]
        struct Tracked(u32);
        impl Drop for Tracked {
            fn drop(&mut self) {
                DROPS.fetch_add(1, Ordering::Relaxed);
            }
        }

        DROPS.store(0, Ordering::Relaxed);
        {
            let mut world = World::new();
            world.spawn((Tracked(1),));
            world.spawn((Tracked(2),));
            world.spawn((Tracked(3),));
            // World drops here.
        }
        assert_eq!(DROPS.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn drop_on_archetype_move() {
        use std::sync::atomic::{AtomicU32, Ordering};
        static DROPS: AtomicU32 = AtomicU32::new(0);

        #[derive(Debug)]
        struct Tracked(u32);
        impl Drop for Tracked {
            fn drop(&mut self) {
                DROPS.fetch_add(1, Ordering::Relaxed);
            }
        }

        DROPS.store(0, Ordering::Relaxed);
        let mut world = World::new();
        let e = world.spawn((Tracked(1),));

        // Insert new component → archetype move, old Tracked should NOT be
        // double-dropped (it's moved, not destroyed).
        world.insert_component(e, Health(100));
        assert_eq!(DROPS.load(Ordering::Relaxed), 0, "move should not drop");

        // Overwrite Tracked → the old value IS dropped.
        world.insert_component(e, Tracked(2));
        assert_eq!(DROPS.load(Ordering::Relaxed), 1, "overwrite should drop old");

        // Remove Tracked → the returned value is still live; drop count
        // increments only when `removed` goes out of scope.
        let removed = world.remove_component::<Tracked>(e);
        assert!(removed.is_some());
        assert_eq!(DROPS.load(Ordering::Relaxed), 1, "remove returns ownership");
        drop(removed);
        assert_eq!(DROPS.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn swap_remove_preserves_other_entities() {
        let mut world = World::new();
        let e1 = world.spawn((A(1),));
        let e2 = world.spawn((A(2),));
        let e3 = world.spawn((A(3),));

        world.despawn(e1); // e3 swaps into row 0

        assert_eq!(world.get::<A>(e2), Some(&A(2)));
        assert_eq!(world.get::<A>(e3), Some(&A(3)));
        assert!(!world.is_alive(e1));
    }

    // -- Benchmarks (as timed tests with thresholds) --------------------------

    #[test]
    fn bench_entity_spawn() {
        let mut world = World::new();
        let start = std::time::Instant::now();
        let n = 500_000u32;
        for i in 0..n {
            world.spawn((A(i), B(i), C(i)));
        }
        let elapsed = start.elapsed();
        let per_sec = n as f64 / elapsed.as_secs_f64();
        eprintln!(
            "spawn: {n} entities in {:.2?} ({:.0} entities/sec)",
            elapsed, per_sec
        );
        assert!(
            per_sec >= 500_000.0,
            "spawn rate {per_sec:.0}/sec below 500K/sec threshold"
        );
    }

    #[test]
    fn bench_query_iteration() {
        let mut world = World::new();
        let n = 1_000_000u32;
        for i in 0..n {
            world.spawn((A(i), B(i)));
        }

        let start = std::time::Instant::now();
        let mut sum = 0u64;
        for (a, b) in world.query::<(&A, &B)>() {
            sum += a.0 as u64 + b.0 as u64;
        }
        let elapsed = start.elapsed();
        let per_sec = n as f64 / elapsed.as_secs_f64();
        // Prevent the compiler from optimizing the loop away.
        assert!(sum > 0);
        eprintln!(
            "query iter: {n} entities in {:.2?} ({:.0} entities/sec)",
            elapsed, per_sec
        );
        assert!(
            per_sec >= 10_000_000.0,
            "query rate {per_sec:.0}/sec below 10M/sec threshold"
        );
    }

    #[test]
    fn bench_archetype_move() {
        let mut world = World::new();
        let n = 200_000u32;
        let entities: Vec<Entity> = (0..n).map(|i| world.spawn((A(i),))).collect();

        let start = std::time::Instant::now();
        for &e in &entities {
            world.insert_component(e, B(0));
        }
        let elapsed = start.elapsed();
        let per_sec = n as f64 / elapsed.as_secs_f64();
        eprintln!(
            "archetype move: {n} entities in {:.2?} ({:.0} moves/sec)",
            elapsed, per_sec
        );
        assert!(
            per_sec >= 200_000.0,
            "archetype move rate {per_sec:.0}/sec below 200K/sec threshold"
        );
    }

    // -- Stress / edge cases --------------------------------------------------

    #[test]
    fn million_spawn_despawn_no_id_collisions() {
        let mut world = World::new();
        let mut seen = HashSet::new();

        // Cycle 1: spawn 1M.
        let mut entities: Vec<Entity> = (0..1_000_000u32)
            .map(|i| world.spawn((A(i),)))
            .collect();
        for &e in &entities {
            assert!(seen.insert((e.index(), e.generation())), "collision in cycle 1");
        }

        // Despawn all.
        for &e in &entities {
            assert!(world.despawn(e));
        }
        assert_eq!(world.entity_count(), 0);

        // Cycle 2: spawn another 1M.
        seen.clear();
        entities.clear();
        for i in 0..1_000_000u32 {
            let e = world.spawn((A(i),));
            assert!(seen.insert((e.index(), e.generation())), "collision in cycle 2");
            entities.push(e);
        }
    }

    #[test]
    fn query_empty_world() {
        let world = World::new();
        let results: Vec<_> = world.query::<&Position>().collect();
        assert!(results.is_empty());
    }

    #[test]
    fn query_no_match() {
        let mut world = World::new();
        world.spawn((Velocity { x: 1.0, y: 2.0 },));
        let results: Vec<_> = world.query::<&Position>().collect();
        assert!(results.is_empty());
    }

    #[test]
    fn insert_then_query() {
        let mut world = World::new();
        let e = world.spawn((Position { x: 1.0, y: 2.0 },));
        world.insert_component(e, Velocity { x: 3.0, y: 4.0 });

        let results: Vec<_> = world.query::<(&Position, &Velocity)>().collect();
        assert_eq!(results.len(), 1);
        assert_eq!(*results[0].0, Position { x: 1.0, y: 2.0 });
        assert_eq!(*results[0].1, Velocity { x: 3.0, y: 4.0 });
    }

    #[test]
    fn remove_then_query() {
        let mut world = World::new();
        let e = world.spawn((Position { x: 1.0, y: 2.0 }, Velocity { x: 3.0, y: 4.0 }));
        world.remove_component::<Velocity>(e);

        let results: Vec<_> = world.query::<(&Position, &Velocity)>().collect();
        assert!(results.is_empty());

        // But Position-only query still finds it.
        let results: Vec<_> = world.query::<&Position>().collect();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn multiple_archetypes_with_shared_query() {
        let mut world = World::new();
        // Archetype (Pos)
        world.spawn((Position { x: 1.0, y: 0.0 },));
        // Archetype (Pos, Vel)
        world.spawn((Position { x: 2.0, y: 0.0 }, Velocity { x: 0.0, y: 0.0 }));
        // Archetype (Pos, Health)
        world.spawn((Position { x: 3.0, y: 0.0 }, Health(10)));
        // Archetype (Pos, Vel, Health)
        world.spawn((
            Position { x: 4.0, y: 0.0 },
            Velocity { x: 0.0, y: 0.0 },
            Health(20),
        ));

        // All four have Position.
        let positions: Vec<_> = world.query::<&Position>().collect();
        assert_eq!(positions.len(), 4);
    }

    #[test]
    fn query_mut_optional() {
        let mut world = World::new();
        world.spawn((Position { x: 0.0, y: 0.0 }, Velocity { x: 1.0, y: 0.0 }));
        world.spawn((Position { x: 0.0, y: 0.0 },)); // no velocity

        for (pos, vel) in world.query_mut::<(&mut Position, Option<&mut Velocity>)>() {
            pos.x += 100.0;
            if let Some(v) = vel {
                v.x += 50.0;
            }
        }

        let positions: Vec<_> = world.query::<&Position>().collect();
        assert!(positions.iter().all(|p| p.x == 100.0));

        let vels: Vec<_> = world.query::<&Velocity>().collect();
        assert_eq!(vels.len(), 1);
        assert_eq!(vels[0].x, 51.0);
    }

    // ========================================================================
    // New tests for task-3: Systems, Schedule, Resources, Events, Commands,
    // Bundles, Change Detection
    // ========================================================================

    // -- System tests --------------------------------------------------------

    #[test]
    fn function_system_reads_query() {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNT: AtomicU32 = AtomicU32::new(0);

        fn count_positions(q: Query<&Position>) {
            let mut n = 0u32;
            for _pos in q.iter() {
                n += 1;
            }
            COUNT.store(n, Ordering::Relaxed);
        }

        COUNT.store(0, Ordering::Relaxed);
        let mut world = World::new();
        world.spawn((Position { x: 1.0, y: 2.0 },));
        world.spawn((Position { x: 3.0, y: 4.0 },));

        let mut sys = count_positions.into_system();
        sys.run(&mut world);
        assert_eq!(COUNT.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn function_system_writes_query() {
        fn add_gravity(mut q: Query<&mut Velocity>) {
            for vel in q.iter_mut() {
                vel.y -= 9.8;
            }
        }

        let mut world = World::new();
        world.spawn((Velocity { x: 0.0, y: 0.0 },));
        world.spawn((Velocity { x: 1.0, y: 5.0 },));

        let mut sys = add_gravity.into_system();
        sys.run(&mut world);

        let vels: Vec<_> = world.query::<&Velocity>().collect();
        assert!((vels[0].y - (-9.8)).abs() < 0.01 || (vels[1].y - (-9.8)).abs() < 0.01);
    }

    #[test]
    fn system_reads_resource() {
        use std::sync::atomic::{AtomicU32, Ordering};
        static RESULT: AtomicU32 = AtomicU32::new(0);

        #[derive(Debug)]
        struct Time {
            frame: u32,
        }

        fn read_time(time: Res<Time>) {
            RESULT.store(time.frame, Ordering::Relaxed);
        }

        RESULT.store(0, Ordering::Relaxed);
        let mut world = World::new();
        world.insert_resource(Time { frame: 42 });

        let mut sys = read_time.into_system();
        sys.run(&mut world);
        assert_eq!(RESULT.load(Ordering::Relaxed), 42);
    }

    // -- System ordering tests -----------------------------------------------

    #[test]
    fn system_ordering_before_after() {
        use std::sync::{Arc, Mutex};

        struct NamedSystem {
            name_str: String,
            func: Box<dyn FnMut(&mut World) + Send>,
        }
        impl System for NamedSystem {
            fn run(&mut self, world: &mut World) { (self.func)(world); }
            fn name(&self) -> &str { &self.name_str }
        }
        impl IntoSystem<NamedSystem> for NamedSystem {
            type System = NamedSystem;
            fn into_system(self) -> Self::System { self }
        }

        let log = Arc::new(Mutex::new(Vec::<&'static str>::new()));
        let mut schedule = Schedule::new();

        let l = log.clone();
        schedule.add_system(Stage::Update, NamedSystem {
            name_str: "sys_a".into(),
            func: Box::new(move |_: &mut World| { l.lock().unwrap().push("A"); }),
        }).before("sys_b");

        let l = log.clone();
        schedule.add_system(Stage::Update, NamedSystem {
            name_str: "sys_b".into(),
            func: Box::new(move |_: &mut World| { l.lock().unwrap().push("B"); }),
        });

        let l = log.clone();
        schedule.add_system(Stage::Update, NamedSystem {
            name_str: "sys_c".into(),
            func: Box::new(move |_: &mut World| { l.lock().unwrap().push("C"); }),
        }).after("sys_b");

        let mut world = World::new();
        schedule.run(&mut world);

        let order = log.lock().unwrap().clone();
        let a_idx = order.iter().position(|&s| s == "A").unwrap();
        let b_idx = order.iter().position(|&s| s == "B").unwrap();
        let c_idx = order.iter().position(|&s| s == "C").unwrap();
        assert!(a_idx < b_idx, "A should run before B, got: {order:?}");
        assert!(b_idx < c_idx, "B should run before C, got: {order:?}");
    }

    #[test]
    #[should_panic(expected = "cycle detected")]
    fn cycle_detection_produces_clear_error() {
        struct NamedSystem {
            name_str: String,
            func: Box<dyn FnMut(&mut World) + Send>,
        }
        impl System for NamedSystem {
            fn run(&mut self, world: &mut World) { (self.func)(world); }
            fn name(&self) -> &str { &self.name_str }
        }
        impl IntoSystem<NamedSystem> for NamedSystem {
            type System = NamedSystem;
            fn into_system(self) -> Self::System { self }
        }

        let mut schedule = Schedule::new();
        schedule.add_system(Stage::Update, NamedSystem {
            name_str: "sys_a".into(),
            func: Box::new(|_| {}),
        }).before("sys_b");

        schedule.add_system(Stage::Update, NamedSystem {
            name_str: "sys_b".into(),
            func: Box::new(|_| {}),
        }).before("sys_a"); // circular!

        let mut world = World::new();
        schedule.run(&mut world);
    }

    #[test]
    fn startup_systems_run_once() {
        use std::sync::atomic::{AtomicU32, Ordering};
        static STARTUP_COUNT: AtomicU32 = AtomicU32::new(0);
        static UPDATE_COUNT: AtomicU32 = AtomicU32::new(0);

        STARTUP_COUNT.store(0, Ordering::Relaxed);
        UPDATE_COUNT.store(0, Ordering::Relaxed);

        fn startup_sys() {
            STARTUP_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        fn update_sys() {
            UPDATE_COUNT.fetch_add(1, Ordering::Relaxed);
        }

        let mut schedule = Schedule::new();
        schedule.add_system(Stage::Startup, startup_sys);
        schedule.add_system(Stage::Update, update_sys);

        let mut world = World::new();
        schedule.run(&mut world); // frame 1
        schedule.run(&mut world); // frame 2
        schedule.run(&mut world); // frame 3

        assert_eq!(STARTUP_COUNT.load(Ordering::Relaxed), 1, "startup should run once");
        assert_eq!(UPDATE_COUNT.load(Ordering::Relaxed), 3, "update should run every frame");
    }

    // -- Commands tests ------------------------------------------------------

    #[test]
    fn commands_spawn_visible_after_apply_deferred() {
        fn spawner(mut commands: Commands) {
            commands.spawn((Position { x: 99.0, y: 99.0 },));
        }

        let mut world = World::new();
        assert_eq!(world.entity_count(), 0);

        let mut sys = spawner.into_system();
        sys.run(&mut world);
        // Not yet applied.
        assert_eq!(world.entity_count(), 0);

        apply_deferred(&mut world);
        assert_eq!(world.entity_count(), 1);

        let results: Vec<_> = world.query::<&Position>().collect();
        assert_eq!(results.len(), 1);
        assert_eq!(*results[0], Position { x: 99.0, y: 99.0 });
    }

    #[test]
    fn commands_despawn_entity() {
        let mut world = World::new();
        let e = world.spawn((Health(100),));

        // Despawn via deferred command.
        world.command_queue.push({
            move |world: &mut World| {
                world.despawn(e);
            }
        });
        assert!(world.is_alive(e)); // Still alive before apply.
        apply_deferred(&mut world);
        assert!(!world.is_alive(e)); // Gone after apply.
    }

    // -- Resource tests ------------------------------------------------------

    #[test]
    fn resource_insert_read_mutate_remove() {
        #[derive(Debug, PartialEq)]
        struct Score(u32);

        let mut world = World::new();
        world.insert_resource(Score(0));

        assert_eq!(world.get_resource::<Score>().0, 0);

        world.get_resource_mut::<Score>().0 = 42;
        assert_eq!(world.get_resource::<Score>().0, 42);

        let removed = world.remove_resource::<Score>();
        assert_eq!(removed, Some(Score(42)));
        assert!(!world.has_resource::<Score>());
    }

    #[test]
    #[should_panic(expected = "not found")]
    fn missing_resource_panics_with_clear_message() {
        #[derive(Debug)]
        struct Missing;

        let world = World::new();
        let _ = world.get_resource::<Missing>();
    }

    // -- Event tests ---------------------------------------------------------

    #[test]
    fn events_write_read_swap() {
        #[derive(Debug, PartialEq)]
        struct MyEvent(u32);

        let mut world = World::new();
        world.add_event::<MyEvent>();

        // Write 3 events.
        world.events.get_mut::<MyEvent>().send(MyEvent(1));
        world.events.get_mut::<MyEvent>().send(MyEvent(2));
        world.events.get_mut::<MyEvent>().send(MyEvent(3));

        // Swap so they become readable.
        world.events.swap_all();

        // Reader sees all 3.
        let events = world.events.get::<MyEvent>().read();
        assert_eq!(events.len(), 3);
        assert_eq!(events[0], MyEvent(1));
        assert_eq!(events[1], MyEvent(2));
        assert_eq!(events[2], MyEvent(3));

        // Another reader sees the same events.
        let events2 = world.events.get::<MyEvent>().read();
        assert_eq!(events2.len(), 3);

        // Swap again (next frame). No new events written.
        world.events.swap_all();

        // Reader now sees 0.
        let events = world.events.get::<MyEvent>().read();
        assert_eq!(events.len(), 0);
    }

    // -- Bundle tests --------------------------------------------------------

    #[test]
    fn spawn_with_bundle_query_works() {
        let mut world = World::new();
        let e = world.spawn((
            Position { x: 1.0, y: 2.0 },
            Velocity { x: 3.0, y: 4.0 },
        ));

        let results: Vec<_> = world.query::<(&Position, &Velocity)>().collect();
        assert_eq!(results.len(), 1);
        assert_eq!(*results[0].0, Position { x: 1.0, y: 2.0 });
        assert_eq!(*results[0].1, Velocity { x: 3.0, y: 4.0 });
        let _ = e;
    }

    // -- Change detection tests ----------------------------------------------

    #[test]
    fn changed_filter_detects_modification() {
        let mut world = World::new();
        world.tick = 1; // Simulate frame 1.
        let e1 = world.spawn((Position { x: 0.0, y: 0.0 },));
        let e2 = world.spawn((Position { x: 1.0, y: 1.0 },));

        // Both spawned at tick 1, so Changed<Position> finds both.
        let changed: Vec<_> = world
            .query_filtered::<(Entity, &Position), Changed<Position>>()
            .collect();
        assert_eq!(changed.len(), 2);

        // Advance to tick 2.
        world.tick = 2;

        // Nothing changed at tick 2 yet.
        let changed: Vec<_> = world
            .query_filtered::<(Entity, &Position), Changed<Position>>()
            .collect();
        assert_eq!(changed.len(), 0);

        // Modify e1 via get_mut → marks change tick = 2.
        world.get_mut::<Position>(e1).unwrap().x = 99.0;

        // Now Changed<Position> should find only e1.
        let changed: Vec<_> = world
            .query_filtered::<(Entity, &Position), Changed<Position>>()
            .collect();
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].0, e1);
        assert_eq!(changed[0].1.x, 99.0);

        let _ = e2;
    }

    #[test]
    fn added_filter_detects_newly_inserted() {
        let mut world = World::new();
        world.tick = 1;
        let e = world.spawn((Position { x: 0.0, y: 0.0 },));

        // Position was added at tick 1.
        let added: Vec<_> = world
            .query_filtered::<Entity, Added<Position>>()
            .collect();
        assert_eq!(added.len(), 1);

        // Advance to tick 2, insert Velocity.
        world.tick = 2;
        world.insert_component(e, Velocity { x: 1.0, y: 0.0 });

        // Velocity was added at tick 2.
        let added_vel: Vec<_> = world
            .query_filtered::<Entity, Added<Velocity>>()
            .collect();
        assert_eq!(added_vel.len(), 1);

        // Position was NOT added at tick 2 (it was added at tick 1).
        let added_pos: Vec<_> = world
            .query_filtered::<Entity, Added<Position>>()
            .collect();
        assert_eq!(added_pos.len(), 0);
    }

    // -- apply_deferred integration tests ------------------------------------

    #[test]
    fn apply_deferred_between_stages() {
        use std::sync::atomic::{AtomicBool, Ordering};
        static FOUND: AtomicBool = AtomicBool::new(false);

        struct NamedSystem {
            name_str: String,
            func: Box<dyn FnMut(&mut World) + Send>,
        }
        impl System for NamedSystem {
            fn run(&mut self, world: &mut World) { (self.func)(world); }
            fn name(&self) -> &str { &self.name_str }
        }
        impl IntoSystem<NamedSystem> for NamedSystem {
            type System = NamedSystem;
            fn into_system(self) -> Self::System { self }
        }

        FOUND.store(false, Ordering::Relaxed);

        let mut schedule = Schedule::new();

        // PreUpdate: spawn entity via commands.
        schedule.add_system(Stage::PreUpdate, NamedSystem {
            name_str: "spawner".into(),
            func: Box::new(|world: &mut World| {
                world.command_queue.push(|w: &mut World| {
                    w.spawn((Health(42),));
                });
            }),
        });

        // Update: check entity exists (commands applied between stages).
        schedule.add_system(Stage::Update, NamedSystem {
            name_str: "checker".into(),
            func: Box::new(|world: &mut World| {
                let count: usize = world.query::<&Health>().count();
                if count > 0 {
                    FOUND.store(true, Ordering::Relaxed);
                }
            }),
        });

        let mut world = World::new();
        schedule.run(&mut world);

        assert!(FOUND.load(Ordering::Relaxed), "entity should be visible in Update after PreUpdate commands");
    }

    // -- Performance benchmarks for new features -----------------------------

    #[test]
    fn bench_schedule_overhead_100_empty_systems() {
        let mut schedule = Schedule::new();
        for _ in 0..100 {
            schedule.add_system(Stage::Update, || {});
        }

        let mut world = World::new();
        // Warm up.
        schedule.run(&mut world);

        let start = std::time::Instant::now();
        let iterations = 100;
        for _ in 0..iterations {
            schedule.run(&mut world);
        }
        let elapsed = start.elapsed();
        let per_frame = elapsed / iterations;
        eprintln!(
            "schedule overhead (100 empty systems): {per_frame:.2?} per frame"
        );
        assert!(
            per_frame.as_micros() < 100,
            "schedule overhead {per_frame:.2?} exceeds 0.1ms threshold"
        );
    }

    #[test]
    fn bench_event_throughput() {
        #[derive(Debug)]
        struct Evt(u64);

        let mut queue = EventQueue::<Evt>::new();
        let n = 1_000_000u64;
        let start = std::time::Instant::now();
        for i in 0..n {
            queue.send(Evt(i));
        }
        queue.swap();
        let mut sum = 0u64;
        for evt in queue.read() {
            sum += evt.0;
        }
        let elapsed = start.elapsed();
        assert!(sum > 0);
        let per_sec = n as f64 / elapsed.as_secs_f64();
        eprintln!(
            "event throughput: {n} events in {:.2?} ({:.0} events/sec)",
            elapsed, per_sec
        );
        assert!(
            per_sec >= 1_000_000.0,
            "event throughput {per_sec:.0}/sec below 1M/sec threshold"
        );
    }
}
