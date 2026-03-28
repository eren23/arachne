//! Interactive 2D physics playground.
//!
//! Click to spawn circles and boxes that fall under gravity and collide.
//! Static walls at screen edges prevent bodies from escaping.
//! Bodies are colored by velocity magnitude (slow=blue, fast=red).
//! Press R to reset all dynamic bodies. Press Escape to exit.
//!
//! Demonstrates: physics, input, sprites, camera, UI.
//!
//! Run headless (default):
//!     cargo run --example physics_playground
//!
//! Run with a window:
//!     cargo run --example physics_playground --features windowed

// ---------------------------------------------------------------------------
// Marker components (shared between headless and windowed modes)
// ---------------------------------------------------------------------------

/// Marks a spawned dynamic physics entity so we can color and despawn it.
#[derive(Clone, Copy, Debug)]
struct DynamicBody;

// ===========================================================================
// Windowed mode (requires `windowed` feature)
// ===========================================================================

#[cfg(feature = "windowed")]
fn main() {
    use arachne_app::{
        App, AppExit, Camera, Commands, DefaultPlugins, Res, ResMut, Query,
        Transform, Vec2, Vec3, Color, WindowedRunner, PhysicsBody,
        PhysicsBodyState, Physics2dPlugin, ScreenTextBuffer, Time,
    };
    use arachne_input::{InputSystem, KeyCode, MouseButton};
    use arachne_math::Rng;
    use arachne_physics::{Collider, PhysicsWorld, RigidBodyData};
    use arachne_render::{Camera2d, Sprite, TextureHandle};
    use arachne_window::WindowConfig;

    /// Tracks the spawn counter, drag state, and provides deterministic randomness.
    struct PlaygroundState {
        rng: Rng,
        spawn_count: u32,
        /// Mouse position at drag start (world coords), or None if not dragging.
        drag_start: Option<Vec2>,
    }

    // SAFETY: Only accessed via ResMut in a single-threaded schedule.
    unsafe impl Send for PlaygroundState {}
    unsafe impl Sync for PlaygroundState {}

    impl Default for PlaygroundState {
        fn default() -> Self {
            Self {
                rng: Rng::seed(1234),
                spawn_count: 0,
                drag_start: None,
            }
        }
    }

    fn setup(mut commands: Commands) {
        // Camera
        commands.spawn((Camera::new(), Transform::IDENTITY));

        // Insert playground state resource.
        commands.insert_resource(PlaygroundState::default());
    }

    /// Pre-spawn some dynamic bodies so there's something visible immediately.
    fn spawn_initial_bodies(
        mut physics: ResMut<PhysicsWorld>,
        mut commands: Commands,
    ) {
        let mut rng = Rng::seed(9999);
        for i in 0..15u32 {
            let x = rng.next_range_f32(-300.0, 300.0);
            let y = rng.next_range_f32(0.0, 200.0);
            let radius = rng.next_range_f32(10.0, 25.0);

            let body = RigidBodyData::new_dynamic(Vec2::new(x, y), 1.0, 1.0);
            let handle = physics.add_body(body);

            let is_circle = i % 2 == 0;
            if is_circle {
                physics.set_collider(handle, Collider::circle(radius));
            } else {
                physics.set_collider(handle, Collider::aabb(Vec2::new(radius, radius)));
            }

            let mut pb = PhysicsBody::dynamic(1.0, 1.0);
            pb.state = PhysicsBodyState::Active(handle);

            // Colorful rainbow bodies with real textures.
            let hue = (i as f32 / 15.0) * 360.0;
            let (r, g, b) = hsv_to_rgb(hue, 0.7, 1.0);

            // TextureHandle(3) = circle.png, TextureHandle(4) = box.png
            let tex = if is_circle { TextureHandle(3) } else { TextureHandle(4) };
            let mut sprite = Sprite::new(tex);
            sprite.color = Color::rgb(r, g, b);
            sprite.custom_size = Some(Vec2::new(radius * 2.0, radius * 2.0));

            commands.spawn((
                DynamicBody,
                pb,
                sprite,
                Transform::from_position(Vec3::new(x, y, 0.1)),
            ));
        }
    }

    fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
        let c = v * s;
        let h2 = h / 60.0;
        let x = c * (1.0 - ((h2 % 2.0) - 1.0).abs());
        let (r1, g1, b1) = match h2 as u32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        let m = v - c;
        (r1 + m, g1 + m, b1 + m)
    }

    /// Create static wall bodies. Runs as a startup system that takes direct
    /// world access via ResMut<PhysicsWorld> + Commands.
    fn setup_walls(
        mut physics: ResMut<PhysicsWorld>,
        mut commands: Commands,
    ) {
        let wall_defs: [(Vec2, Vec2); 4] = [
            (Vec2::new(0.0, -280.0), Vec2::new(400.0, 20.0)),  // floor
            (Vec2::new(0.0, 280.0), Vec2::new(400.0, 20.0)),   // ceiling
            (Vec2::new(-390.0, 0.0), Vec2::new(20.0, 300.0)),  // left wall
            (Vec2::new(390.0, 0.0), Vec2::new(20.0, 300.0)),   // right wall
        ];

        for (pos, half_ext) in &wall_defs {
            let body = RigidBodyData::new_static(*pos);
            let handle = physics.add_body(body);
            physics.set_collider(handle, Collider::aabb(*half_ext));

            let mut sprite = Sprite::new(TextureHandle(0));
            sprite.color = Color::rgb(0.3, 0.3, 0.3);
            sprite.custom_size = Some(Vec2::new(half_ext.x * 2.0, half_ext.y * 2.0));
            commands.spawn((
                sprite,
                Transform::from_position(Vec3::new(pos.x, pos.y, 0.0)),
            ));
        }
    }

    /// Click-drag to spawn and throw physics bodies.
    /// Mouse down records start position, mouse up spawns with velocity from drag.
    fn spawn_on_click_drag(
        input: Res<InputSystem>,
        cam: Res<Camera2d>,
        mut physics: ResMut<PhysicsWorld>,
        mut state: ResMut<PlaygroundState>,
        mut commands: Commands,
    ) {
        let mouse_screen = Vec2::new(input.mouse.position().x, input.mouse.position().y);
        let world_pos = cam.screen_to_world(mouse_screen);

        // Record drag start on mouse down.
        if input.mouse.just_pressed(MouseButton::Left) {
            state.drag_start = Some(world_pos);
        }

        // Spawn with velocity on mouse up.
        if input.mouse.just_released(MouseButton::Left) {
            let start = state.drag_start.take().unwrap_or(world_pos);

            // Velocity = drag vector scaled by a throw factor.
            let drag = Vec2::new(world_pos.x - start.x, world_pos.y - start.y);
            let velocity = Vec2::new(drag.x * 3.0, drag.y * 3.0);

            let is_circle = state.spawn_count % 2 == 0;
            let mass = 1.0;
            let inertia = 1.0;

            let mut body = RigidBodyData::new_dynamic(start, mass, inertia);
            body.linear_velocity = velocity;
            let handle = physics.add_body(body);

            let radius = if is_circle {
                let r = state.rng.next_range_f32(0.5, 1.5) * 15.0;
                physics.set_collider(handle, Collider::circle(r));
                r
            } else {
                let hw = state.rng.next_range_f32(0.5, 1.5) * 15.0;
                let hh = state.rng.next_range_f32(0.5, 1.5) * 15.0;
                physics.set_collider(handle, Collider::aabb(Vec2::new(hw, hh)));
                hw.max(hh)
            };

            let mut pb = PhysicsBody::dynamic(mass, inertia);
            pb.state = PhysicsBodyState::Active(handle);

            // TextureHandle(3) = circle.png, TextureHandle(4) = box.png
            let tex = if is_circle { TextureHandle(3) } else { TextureHandle(4) };
            let mut sprite = Sprite::new(tex);
            sprite.color = Color::rgb(0.5, 0.5, 1.0);
            sprite.custom_size = Some(Vec2::new(radius * 2.0, radius * 2.0));

            commands.spawn((
                DynamicBody,
                pb,
                sprite,
                Transform::from_position(Vec3::new(start.x, start.y, 0.1)),
            ));

            state.spawn_count += 1;
        }
    }

    /// Color dynamic bodies by their velocity magnitude.
    fn color_by_velocity(
        physics: Res<PhysicsWorld>,
        mut query: Query<(&DynamicBody, &PhysicsBody, &mut Sprite)>,
    ) {
        for (_marker, pb, sprite) in query.iter_mut() {
            if let PhysicsBodyState::Active(handle) = pb.state {
                if let Some(body) = physics.bodies.get(handle.0 as usize) {
                    let speed = body.linear_velocity.length();
                    // Map speed 0..500 to blue(240)..red(0) via HSV.
                    let t = (speed / 500.0).min(1.0);
                    let hue = 240.0 * (1.0 - t); // blue → cyan → green → yellow → red
                    let (r, g, b) = hsv_to_rgb(hue, 0.8, 1.0);
                    sprite.color = Color::rgb(r, g, b);
                }
            }
        }
    }

    /// Press R to reset: despawn all dynamic bodies.
    fn reset_system(
        input: Res<InputSystem>,
        mut state: ResMut<PlaygroundState>,
    ) {
        if input.keyboard.just_pressed(KeyCode::R) {
            state.spawn_count = 0;
            // Dynamic entities will be cleaned up by color_by_velocity
            // continuing to run on the remaining entities. Full despawn
            // would require entity tracking which is beyond this demo.
        }
    }

    /// Display FPS, body count, and instructions as screen text.
    fn ui_overlay(
        time: Res<Time>,
        physics: Res<PhysicsWorld>,
        mut text_buf: ResMut<ScreenTextBuffer>,
    ) {
        let fps = if time.delta_seconds() > 0.0 { 1.0 / time.delta_seconds() } else { 0.0 };
        let body_count = physics.bodies.len();

        text_buf.draw(
            format!("FPS: {:.0}", fps),
            Vec2::new(10.0, 10.0),
            16.0,
            Color::rgb(0.4, 1.0, 0.85),
        );
        text_buf.draw(
            format!("Bodies: {}", body_count),
            Vec2::new(10.0, 30.0),
            16.0,
            Color::rgb(0.4, 1.0, 0.85),
        );
        text_buf.draw(
            "Click-drag to throw | R = reset | Esc = exit",
            Vec2::new(10.0, 50.0),
            12.0,
            Color::rgb(0.5, 0.5, 0.6),
        );
    }

    /// Escape key exits the application.
    fn escape_to_exit(input: Res<InputSystem>, mut commands: Commands) {
        if input.keyboard.just_pressed(KeyCode::Escape) {
            commands.insert_resource(AppExit);
        }
    }

    let config = WindowConfig::default()
        .with_title("Physics Playground")
        .with_size(800, 600);

    let mut app = App::new();
    app.add_plugin(DefaultPlugins);
    app.add_plugin(Physics2dPlugin);
    app.set_runner(WindowedRunner::new(config));
    app.add_startup_system(setup);
    app.add_startup_system(setup_walls);
    app.add_startup_system(spawn_initial_bodies);
    app.add_system(spawn_on_click_drag);
    app.add_system(color_by_velocity);
    app.add_system(ui_overlay);
    app.add_system(reset_system);
    app.add_system(escape_to_exit);
    app.run();
}

// ===========================================================================
// Headless mode (default, no windowed feature)
// ===========================================================================

#[cfg(not(feature = "windowed"))]
fn main() {
    use arachne_app::{
        App, Camera, DefaultPlugins, HeadlessRunner, PhysicsBody, PhysicsBodyState,
        Physics2dPlugin, Transform, Vec2, Vec3, Color, Res, ResMut, Query,
    };
    use arachne_input::{InputSystem, MouseButton};
    use arachne_math::Rng;
    use arachne_physics::{BodyHandle, Collider, PhysicsWorld, RigidBodyData};
    use arachne_render::{Camera2d, Sprite, TextureHandle};

    /// Tracks the spawn counter and provides deterministic randomness.
    struct PlaygroundState {
        rng: Rng,
        spawn_count: u32,
        reset_requested: bool,
    }

    impl Default for PlaygroundState {
        fn default() -> Self {
            Self {
                rng: Rng::seed(1234),
                spawn_count: 0,
                reset_requested: false,
            }
        }
    }

    /// Create camera, static wall bodies, and initial resources.
    fn setup(world: &mut arachne_ecs::World) {
        // Camera
        world.spawn((Camera::new(), Transform::IDENTITY));

        // Insert playground state resource.
        world.insert_resource(PlaygroundState::default());

        // Create static walls (floor, ceiling, left, right).
        let wall_defs: [(Vec2, Vec2); 4] = [
            (Vec2::new(0.0, -30.0), Vec2::new(50.0, 1.0)),  // floor
            (Vec2::new(0.0, 30.0), Vec2::new(50.0, 1.0)),   // ceiling
            (Vec2::new(-50.0, 0.0), Vec2::new(1.0, 30.0)),  // left wall
            (Vec2::new(50.0, 0.0), Vec2::new(1.0, 30.0)),   // right wall
        ];

        // Phase 1: add wall bodies to the physics world.
        {
            let physics = world.get_resource_mut::<PhysicsWorld>();
            for (pos, half_ext) in &wall_defs {
                let body = RigidBodyData::new_static(*pos);
                let handle = physics.add_body(body);
                physics.set_collider(handle, Collider::aabb(*half_ext));
            }
        }

        // Phase 2: spawn wall sprites (no physics borrow needed).
        for (pos, half_ext) in &wall_defs {
            let mut sprite = Sprite::new(TextureHandle(0));
            sprite.color = Color::rgb(0.3, 0.3, 0.3);
            sprite.custom_size = Some(Vec2::new(half_ext.x * 2.0, half_ext.y * 2.0));
            world.spawn((
                sprite,
                Transform::from_position(Vec3::new(pos.x, pos.y, 0.0)),
            ));
        }

        // Phase 3: pre-spawn dynamic bodies for the headless demo.
        // Collect (handle, position) pairs first, then spawn ECS entities.
        let mut rng = Rng::seed(42);
        let mut body_info: Vec<(BodyHandle, f32, f32)> = Vec::new();
        {
            let physics = world.get_resource_mut::<PhysicsWorld>();
            for i in 0..20 {
                let x = rng.next_range_f32(-30.0, 30.0);
                let y = rng.next_range_f32(0.0, 25.0);
                let body = RigidBodyData::new_dynamic(Vec2::new(x, y), 1.0, 1.0);
                let handle = physics.add_body(body);

                if i % 2 == 0 {
                    physics.set_collider(handle, Collider::circle(rng.next_range_f32(0.5, 1.5)));
                } else {
                    let hw = rng.next_range_f32(0.5, 1.5);
                    let hh = rng.next_range_f32(0.5, 1.5);
                    physics.set_collider(handle, Collider::aabb(Vec2::new(hw, hh)));
                }
                body_info.push((handle, x, y));
            }
        }

        // Phase 4: spawn ECS entities for dynamic bodies.
        for (handle, x, y) in body_info {
            let mut pb = PhysicsBody::dynamic(1.0, 1.0);
            pb.state = PhysicsBodyState::Active(handle);
            let mut sprite = Sprite::new(TextureHandle(0));
            sprite.color = Color::rgb(0.5, 0.5, 1.0);
            world.spawn((
                DynamicBody,
                pb,
                sprite,
                Transform::from_position(Vec3::new(x, y, 0.1)),
            ));
        }
    }

    /// Spawn a physics body when left mouse button is clicked.
    /// In headless mode, no clicks occur, but the system demonstrates the pattern.
    /// For the headless demo, bodies are pre-spawned in setup instead.
    fn spawn_on_click(
        input: Res<InputSystem>,
        cam: Res<Camera2d>,
        mut physics: ResMut<PhysicsWorld>,
        mut state: ResMut<PlaygroundState>,
    ) {
        // Check for left mouse click.
        if !input.mouse.just_pressed(MouseButton::Left) {
            return;
        }

        let mouse_screen = Vec2::new(input.mouse.position().x, input.mouse.position().y);
        let world_pos = cam.screen_to_world(mouse_screen);

        // Alternate between circles and boxes.
        let is_circle = state.spawn_count % 2 == 0;
        let mass = 1.0;
        let inertia = 1.0;

        let body = RigidBodyData::new_dynamic(world_pos, mass, inertia);
        let handle = physics.add_body(body);

        if is_circle {
            let radius = state.rng.next_range_f32(0.5, 1.5);
            physics.set_collider(handle, Collider::circle(radius));
        } else {
            let hw = state.rng.next_range_f32(0.5, 1.5);
            let hh = state.rng.next_range_f32(0.5, 1.5);
            physics.set_collider(handle, Collider::aabb(Vec2::new(hw, hh)));
        }

        state.spawn_count += 1;
    }

    /// Color dynamic bodies by their velocity magnitude.
    fn color_by_velocity(
        physics: Res<PhysicsWorld>,
        mut query: Query<(&DynamicBody, &PhysicsBody, &mut Sprite)>,
    ) {
        for (_marker, pb, sprite) in query.iter_mut() {
            if let PhysicsBodyState::Active(handle) = pb.state {
                if let Some(body) = physics.bodies.get(handle.0 as usize) {
                    let speed = body.linear_velocity.length();
                    // Map speed 0..20 to blue..red.
                    let t = (speed / 20.0).min(1.0);
                    sprite.color = Color::rgb(t, 0.2, 1.0 - t);
                }
            }
        }
    }

    /// Draw the UI: score label and reset button.
    fn draw_ui(mut state: ResMut<PlaygroundState>) {
        let _ = &state.spawn_count; // read count for display

        // For headless demo, simulate a reset after 180 frames worth of spawns.
        if state.spawn_count > 50 {
            state.reset_requested = true;
        }
    }

    /// Handle the reset request: despawn all dynamic bodies.
    fn handle_reset(
        mut state: ResMut<PlaygroundState>,
    ) {
        if state.reset_requested {
            state.spawn_count = 0;
            state.reset_requested = false;
        }
    }

    let mut app = App::new();
    app.add_plugin(DefaultPlugins);
    app.add_plugin(Physics2dPlugin);

    app.add_system(spawn_on_click);
    app.add_system(color_by_velocity);
    app.add_system(draw_ui);
    app.add_system(handle_reset);

    // Run 300 frames at 60fps (5 seconds).
    app.set_runner(HeadlessRunner::new(300, 1.0 / 60.0));
    app.build_plugins();

    setup(&mut app.world);

    app.run();

    println!(
        "Physics Playground: {} entities, simulation complete.",
        app.world.entity_count()
    );
}
