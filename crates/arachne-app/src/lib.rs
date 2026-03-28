pub mod app;
pub mod components;
pub mod default_plugins;
pub mod diagnostic;
pub mod gpu_init;
pub mod plugin;
pub mod runner;
pub mod systems;
pub mod time;

// Re-export key types for ergonomic imports.
pub use app::App;
pub use components::{Camera, ColliderComponent, DrawCallCount, GlobalTransform, PhysicsBody, PhysicsBodyState};
pub use default_plugins::DefaultPlugins;
pub use diagnostic::{DiagnosticChannel, DiagnosticPlugin, Diagnostics, SystemTiming};
pub use plugin::{
    AnimationPlugin, AudioPlugin, NetworkPlugin, ParticlePlugin, Physics2dPlugin, Plugin,
    UIPlugin,
};
pub use runner::{AppExit, HeadlessRunner, NativeRunner, Runner};
pub use gpu_init::{
    init_gpu_resources, GpuResources, RenderContextResource, SpritePipelineResource,
    TilemapPipelineResource,
};
#[cfg(feature = "windowed")]
pub use runner::WindowedRunner;
pub use systems::{ScreenTextBuffer, TextRendererResource, TilemapRendererResource, TextureStorageResource};
pub use time::{Stopwatch, Time, Timer};

// Re-export core ECS types for convenience.
pub use arachne_ecs::{
    Bundle, Commands, Entity, IntoSystem, Query, Res, ResMut, Schedule, Stage, System, World,
    With, Without,
};

// Re-export math types commonly used in app code.
pub use arachne_math::{Color, Mat4, Quat, Rect, Transform, Vec2, Vec3};

// ---------------------------------------------------------------------------
// Integration tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use arachne_physics::{BodyHandle, BodyType, Collider, PhysicsWorld, RigidBodyData};
    use arachne_physics::world::PhysicsConfig;
    use arachne_render::Sprite;
    use arachne_render::TextureHandle;
    extern crate wgpu;

    // -----------------------------------------------------------------------
    // App basics
    // -----------------------------------------------------------------------

    #[test]
    fn app_create_add_plugin_add_system_run_one_frame() {
        #[derive(Debug, Default)]
        struct Counter(u32);

        struct TestPlugin;
        impl crate::plugin::Plugin for TestPlugin {
            fn build(&self, app: &mut App) {
                app.world.insert_resource(Counter(0));
            }
        }

        fn increment_system(mut counter: ResMut<Counter>) {
            counter.0 += 1;
        }

        let mut app = App::new();
        app.add_plugin(TestPlugin);
        app.add_system(increment_system);
        app.set_runner(HeadlessRunner::new(1, 1.0 / 60.0));
        app.run();

        let counter = app.world.get_resource::<Counter>();
        assert_eq!(counter.0, 1, "System should have executed once");
    }

    #[test]
    fn plugin_registers_resource_available_to_systems() {
        #[derive(Debug)]
        struct MyResource(String);

        struct ResPlugin;
        impl crate::plugin::Plugin for ResPlugin {
            fn build(&self, app: &mut App) {
                app.world.insert_resource(MyResource("hello".to_string()));
            }
        }

        #[derive(Debug, Default)]
        struct Verified(bool);

        fn verify_system(res: Res<MyResource>, mut v: ResMut<Verified>) {
            if res.0 == "hello" {
                v.0 = true;
            }
        }

        let mut app = App::new();
        app.insert_resource(Verified(false));
        app.add_plugin(ResPlugin);
        app.add_system(verify_system);
        app.set_runner(HeadlessRunner::new(1, 1.0 / 60.0));
        app.run();

        assert!(app.world.get_resource::<Verified>().0);
    }

    // -----------------------------------------------------------------------
    // Time
    // -----------------------------------------------------------------------

    #[test]
    fn time_after_5_frames_at_60fps() {
        let mut app = App::new();
        app.set_runner(HeadlessRunner::new(5, 1.0 / 60.0));
        app.run();

        let time = app.world.get_resource::<Time>();
        let expected_elapsed = 5.0 / 60.0;
        assert!(
            (time.elapsed_seconds() - expected_elapsed).abs() < 1e-5,
            "elapsed={} expected={}",
            time.elapsed_seconds(),
            expected_elapsed
        );
        assert!(
            (time.delta_seconds() - 1.0 / 60.0).abs() < 1e-6,
            "delta={}",
            time.delta_seconds()
        );
    }

    // -----------------------------------------------------------------------
    // Runner: frame count
    // -----------------------------------------------------------------------

    #[test]
    fn runner_10_frames_verify_frame_count() {
        let mut app = App::new();
        app.set_runner(HeadlessRunner::new(10, 1.0 / 60.0));
        app.run();

        let time = app.world.get_resource::<Time>();
        assert_eq!(time.frame_count(), 10);
    }

    // -----------------------------------------------------------------------
    // Stage ordering
    // -----------------------------------------------------------------------

    #[test]
    fn stage_ordering_preupdate_before_update() {
        #[derive(Debug, Default)]
        struct Log(Vec<&'static str>);

        fn pre_update_sys(mut log: ResMut<Log>) {
            log.0.push("pre_update");
        }
        fn update_sys(mut log: ResMut<Log>) {
            log.0.push("update");
        }
        fn post_update_sys(mut log: ResMut<Log>) {
            log.0.push("post_update");
        }
        fn render_sys(mut log: ResMut<Log>) {
            log.0.push("render");
        }

        let mut app = App::new();
        app.insert_resource(Log::default());
        // Add in reverse order to verify ordering is by stage, not insertion.
        app.schedule.add_system(Stage::Render, render_sys);
        app.schedule.add_system(Stage::PostUpdate, post_update_sys);
        app.schedule.add_system(Stage::Update, update_sys);
        app.schedule.add_system(Stage::PreUpdate, pre_update_sys);
        app.set_runner(HeadlessRunner::new(1, 1.0 / 60.0));
        app.run();

        let log = app.world.get_resource::<Log>();
        assert_eq!(
            log.0,
            vec!["pre_update", "update", "post_update", "render"],
            "Stages must execute in order: {:?}",
            log.0
        );
    }

    // -----------------------------------------------------------------------
    // Diagnostics
    // -----------------------------------------------------------------------

    #[test]
    fn diagnostics_fps_at_60fps() {
        let mut app = App::new();
        app.add_plugin(DiagnosticPlugin);
        app.set_runner(HeadlessRunner::new(120, 1.0 / 60.0));
        app.run();

        let diag = app.world.get_resource::<Diagnostics>();
        let fps = diag.fps();
        assert!(
            (fps - 60.0).abs() < 1.0,
            "Expected ~60 FPS, got {}",
            fps
        );
    }

    // -----------------------------------------------------------------------
    // Integration: Sprite + Transform → draw call count
    // -----------------------------------------------------------------------

    #[test]
    fn sprite_transform_produces_draw_calls() {
        let mut app = App::new();
        app.add_plugin(DefaultPlugins);
        app.set_runner(HeadlessRunner::new(1, 1.0 / 60.0));

        // Build plugins first so resources exist.
        app.build_plugins();

        // Spawn entities with Sprite + Transform.
        let sprite = Sprite::new(TextureHandle(0));
        for i in 0..5 {
            app.world.spawn((
                sprite.clone(),
                Transform::from_position(Vec3::new(i as f32, 0.0, 0.0)),
            ));
        }

        app.run();

        let count = app.world.get_resource::<DrawCallCount>();
        assert_eq!(count.0, 5, "Should count 5 draw calls, got {}", count.0);
    }

    // -----------------------------------------------------------------------
    // Integration: Physics — gravity moves transform
    // -----------------------------------------------------------------------

    #[test]
    fn physics_gravity_changes_transform_y() {
        let mut app = App::new();
        app.add_plugin(DefaultPlugins);
        app.add_plugin(Physics2dPlugin);
        app.set_runner(HeadlessRunner::new(10, 1.0 / 60.0));

        app.build_plugins();

        // Add a dynamic body to the physics world and track it.
        let initial_y = 10.0;
        let handle = {
            let physics = app.world.get_resource_mut::<PhysicsWorld>();
            let body = RigidBodyData::new_dynamic(Vec2::new(0.0, initial_y), 1.0, 1.0);
            let h = physics.add_body(body);
            physics.set_collider(h, Collider::circle(0.5));
            h
        };

        // Spawn an entity with PhysicsBody + Transform.
        let mut pb = PhysicsBody::dynamic(1.0, 1.0);
        pb.state = PhysicsBodyState::Active(handle);
        app.world.spawn((
            pb,
            Transform::from_position(Vec3::new(0.0, initial_y, 0.0)),
        ));

        app.run();

        // After 10 frames of gravity, y should have decreased.
        let physics = app.world.get_resource::<PhysicsWorld>();
        let body = &physics.bodies[handle.0 as usize];
        assert!(
            body.position.y < initial_y,
            "Body should have fallen: y={} (initial={})",
            body.position.y,
            initial_y
        );
    }

    // -----------------------------------------------------------------------
    // Despawn physics entity → body count verification
    // -----------------------------------------------------------------------

    #[test]
    fn despawn_entity_no_crash() {
        let mut app = App::new();
        app.add_plugin(DefaultPlugins);
        app.set_runner(HeadlessRunner::new(1, 1.0 / 60.0));
        app.build_plugins();

        let e = app.world.spawn((
            Sprite::new(TextureHandle(0)),
            Transform::IDENTITY,
        ));

        assert_eq!(app.world.entity_count(), 1);
        app.world.despawn(e);
        assert_eq!(app.world.entity_count(), 0);

        // Run a frame after despawn — should not crash.
        app.run();
    }

    // -----------------------------------------------------------------------
    // Full lifecycle: spawn many, run, despawn all
    // -----------------------------------------------------------------------

    #[test]
    fn full_lifecycle_spawn_run_despawn() {
        let mut app = App::new();
        app.add_plugin(DefaultPlugins);
        app.add_plugin(Physics2dPlugin);
        app.set_runner(HeadlessRunner::new(60, 1.0 / 60.0));
        app.build_plugins();

        let mut entities = Vec::new();

        // Spawn 50 sprite-only entities.
        for i in 0..50 {
            let e = app.world.spawn((
                Sprite::new(TextureHandle(0)),
                Transform::from_position(Vec3::new(i as f32, 0.0, 0.0)),
            ));
            entities.push(e);
        }

        // Spawn 50 physics entities.
        for i in 0..50 {
            let handle = {
                let physics = app.world.get_resource_mut::<PhysicsWorld>();
                let body = RigidBodyData::new_dynamic(
                    Vec2::new(i as f32, 10.0),
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
                Transform::from_position(Vec3::new(i as f32, 10.0, 0.0)),
            ));
            entities.push(e);
        }

        assert_eq!(app.world.entity_count(), 100);

        // Run 60 frames.
        app.run();

        // Despawn all.
        for e in &entities {
            app.world.despawn(*e);
        }

        assert_eq!(app.world.entity_count(), 0, "All entities should be despawned");
    }

    // -----------------------------------------------------------------------
    // Camera movement affects Camera2d resource
    // -----------------------------------------------------------------------

    #[test]
    fn camera_movement_updates_camera2d() {
        let mut app = App::new();
        app.add_plugin(DefaultPlugins);
        app.set_runner(HeadlessRunner::new(1, 1.0 / 60.0));
        app.build_plugins();

        // Spawn a camera entity.
        app.world.spawn((
            Camera::new(),
            Transform::from_position(Vec3::new(100.0, 200.0, 0.0)),
        ));

        app.run();

        let cam2d = app.world.get_resource::<arachne_render::Camera2d>();
        assert!(
            (cam2d.position.x - 100.0).abs() < 1e-3,
            "Camera2d x={} expected 100",
            cam2d.position.x
        );
        assert!(
            (cam2d.position.y - 200.0).abs() < 1e-3,
            "Camera2d y={} expected 200",
            cam2d.position.y
        );
    }

    // -----------------------------------------------------------------------
    // Physics transform sync precision
    // -----------------------------------------------------------------------

    #[test]
    fn physics_transform_sync_precision() {
        let mut app = App::new();
        app.add_plugin(DefaultPlugins);
        app.add_plugin(Physics2dPlugin);
        app.set_runner(HeadlessRunner::new(1, 1.0 / 60.0));
        app.build_plugins();

        let px = 42.5;
        let py = -17.3;
        let handle = {
            let physics = app.world.get_resource_mut::<PhysicsWorld>();
            let body = RigidBodyData::new_static(Vec2::new(px, py));
            physics.add_body(body)
        };

        let mut pb = PhysicsBody::static_body();
        pb.state = PhysicsBodyState::Active(handle);
        let entity = app.world.spawn((
            pb,
            Transform::IDENTITY,
        ));

        app.run();

        let t = app.world.get::<Transform>(entity).unwrap();
        assert!(
            (t.position.x - px).abs() < 1e-6,
            "x sync: {} vs {}",
            t.position.x,
            px
        );
        assert!(
            (t.position.y - py).abs() < 1e-6,
            "y sync: {} vs {}",
            t.position.y,
            py
        );
    }

    // -----------------------------------------------------------------------
    // Startup system runs once
    // -----------------------------------------------------------------------

    #[test]
    fn startup_system_runs_once() {
        #[derive(Debug, Default)]
        struct StartupCount(u32);

        fn startup_sys(mut c: ResMut<StartupCount>) {
            c.0 += 1;
        }

        let mut app = App::new();
        app.insert_resource(StartupCount(0));
        app.add_startup_system(startup_sys);
        app.set_runner(HeadlessRunner::new(5, 1.0 / 60.0));
        app.run();

        assert_eq!(app.world.get_resource::<StartupCount>().0, 1);
    }

    // -----------------------------------------------------------------------
    // Time drift check over many frames
    // -----------------------------------------------------------------------

    #[test]
    fn time_drift_under_1ms_over_1000_frames() {
        let mut app = App::new();
        let dt = 1.0 / 60.0;
        app.set_runner(HeadlessRunner::new(1000, dt));
        app.run();

        let time = app.world.get_resource::<Time>();
        let expected = dt * 1000.0;
        let drift = (time.elapsed_seconds() - expected).abs();
        assert!(
            drift < 0.001,
            "Time drift {:.6}s exceeds 1ms over 1000 frames (expected={}, actual={})",
            drift,
            expected,
            time.elapsed_seconds()
        );
    }

    // -----------------------------------------------------------------------
    // Integration: SpriteRendererResource batches draw calls
    // -----------------------------------------------------------------------

    #[test]
    fn sprite_renderer_resource_batches_draw_calls() {
        use crate::systems::SpriteRendererResource;

        let (device, queue) = pollster::block_on(async {
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::LowPower,
                    ..Default::default()
                })
                .await
                .unwrap();
            adapter
                .request_device(&wgpu::DeviceDescriptor::default(), None)
                .await
                .unwrap()
        });

        let renderer = arachne_render::SpriteRenderer::new(&device);

        let mut app = App::new();
        app.add_plugin(DefaultPlugins);
        app.set_runner(HeadlessRunner::new(1, 1.0 / 60.0));
        app.build_plugins();

        app.world.insert_resource(SpriteRendererResource {
            renderer,
            device,
            queue,
            last_batches: Vec::new(),
        });

        // 5 sprites with the same texture → 1 batched draw call.
        let sprite = Sprite::new(TextureHandle(0));
        for i in 0..5 {
            app.world.spawn((
                sprite.clone(),
                Transform::from_position(Vec3::new(i as f32, 0.0, 0.0)),
            ));
        }

        app.run();

        let count = app.world.get_resource::<DrawCallCount>();
        assert_eq!(
            count.0, 1,
            "5 sprites with same texture should batch to 1 draw call, got {}",
            count.0
        );
    }

    // -----------------------------------------------------------------------
    // Benchmark: empty app overhead
    // -----------------------------------------------------------------------

    #[test]
    fn bench_empty_app_overhead() {
        use std::time::Instant;

        let mut app = App::new();
        // Single frame warm-up.
        app.set_runner(HeadlessRunner::new(1, 1.0 / 60.0));
        app.run();

        let iterations = 1000u64;
        let start = Instant::now();
        {
            let time = app.world.get_resource_mut::<Time>();
            time.reset();
        }
        app.set_runner(HeadlessRunner::new(iterations, 1.0 / 60.0));
        app.run();
        let elapsed = start.elapsed();

        let avg_ms = elapsed.as_secs_f64() * 1000.0 / iterations as f64;
        eprintln!("Empty app frame: {:.4}ms avg ({} frames)", avg_ms, iterations);
        assert!(
            avg_ms < 0.5,
            "Empty app overhead {:.4}ms exceeds 0.5ms",
            avg_ms
        );
    }

    // -----------------------------------------------------------------------
    // Benchmark: 1000 sprites + 100 physics bodies
    // -----------------------------------------------------------------------

    #[test]
    fn bench_full_frame_1000_sprites_100_physics() {
        use std::time::Instant;

        let mut app = App::new();
        app.add_plugin(DefaultPlugins);
        app.add_plugin(Physics2dPlugin);
        app.set_runner(HeadlessRunner::new(1, 1.0 / 60.0));
        app.build_plugins();

        // Spawn 1000 sprites.
        for i in 0..1000 {
            app.world.spawn((
                Sprite::new(TextureHandle(0)),
                Transform::from_position(Vec3::new(
                    (i % 100) as f32,
                    (i / 100) as f32,
                    0.0,
                )),
            ));
        }

        // Spawn 100 physics bodies.
        for i in 0..100 {
            let handle = {
                let physics = app.world.get_resource_mut::<PhysicsWorld>();
                let body = RigidBodyData::new_dynamic(
                    Vec2::new(i as f32 * 2.0, 50.0),
                    1.0,
                    1.0,
                );
                let h = physics.add_body(body);
                physics.set_collider(h, Collider::circle(0.5));
                h
            };
            let mut pb = PhysicsBody::dynamic(1.0, 1.0);
            pb.state = PhysicsBodyState::Active(handle);
            app.world.spawn((
                pb,
                Transform::from_position(Vec3::new(i as f32 * 2.0, 50.0, 0.0)),
            ));
        }

        // Warm up.
        app.set_runner(HeadlessRunner::new(1, 1.0 / 60.0));
        app.run();

        let iterations = 60u64;
        let start = Instant::now();
        app.set_runner(HeadlessRunner::new(iterations, 1.0 / 60.0));
        app.run();
        let elapsed = start.elapsed();

        let avg_ms = elapsed.as_secs_f64() * 1000.0 / iterations as f64;
        eprintln!(
            "Full frame (1000 sprites + 100 physics): {:.2}ms avg ({} frames)",
            avg_ms, iterations
        );
        assert!(
            avg_ms < 16.6,
            "Full frame {:.2}ms exceeds 16.6ms (60fps budget)",
            avg_ms
        );
    }
}
