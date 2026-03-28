//! 2D sprite demo with input-driven movement.
//!
//! Loads placeholder sprite handles via the asset system, spawns a player sprite
//! controllable with arrow keys, and scatters 100 background sprites at random
//! positions.
//!
//! Demonstrates: sprites, input, asset loading (simulated), 2D camera, Rng.
//!
//! Run in headless mode (default):
//!     cargo run --example sprite_demo
//!
//! Run in windowed mode:
//!     cargo run --example sprite_demo --features windowed

// ---------------------------------------------------------------------------
// Marker components (shared between headless and windowed modes)
// ---------------------------------------------------------------------------

/// Marks the player-controlled entity.
#[derive(Clone, Copy, Debug)]
struct Player;

/// Marks a background decoration sprite.
#[derive(Clone, Copy, Debug)]
struct Background;

// ===========================================================================
// Headless mode
// ===========================================================================

#[cfg(not(feature = "windowed"))]
fn main() {
    use arachne_app::{
        App, Camera, DefaultPlugins, HeadlessRunner, Transform, Vec2, Vec3, Color,
        Res, Query,
    };
    use arachne_input::{InputSystem, KeyCode};
    use arachne_math::Rng;
    use arachne_render::{Sprite, TextureHandle};

    fn setup(world: &mut arachne_ecs::World) {
        // Camera
        world.spawn((Camera::new(), Transform::IDENTITY));

        // Player sprite (green tint, at origin).
        let mut player_sprite = Sprite::new(TextureHandle(1));
        player_sprite.color = Color::GREEN;
        player_sprite.custom_size = Some(Vec2::new(2.0, 2.0));
        world.spawn((
            Player,
            player_sprite,
            Transform::from_position(Vec3::new(0.0, 0.0, 0.1)),
        ));

        // 100 background sprites at random positions.
        let mut rng = Rng::seed(42);
        for _ in 0..100 {
            let x = rng.next_range_f32(-50.0, 50.0);
            let y = rng.next_range_f32(-50.0, 50.0);
            let r = rng.next_f32();
            let g = rng.next_f32();
            let b = rng.next_f32();

            let mut sprite = Sprite::new(TextureHandle(0));
            sprite.color = Color::rgb(r, g, b);
            world.spawn((
                Background,
                sprite,
                Transform::from_position(Vec3::new(x, y, 0.0)),
            ));
        }
    }

    fn move_player(
        input: Res<InputSystem>,
        mut query: Query<(&Player, &mut Transform)>,
    ) {
        let speed = 5.0; // units per second (applied per frame for simplicity)

        for (_player, transform) in query.iter_mut() {
            let mut delta = Vec2::ZERO;

            if input.keyboard.pressed(KeyCode::Right) || input.keyboard.pressed(KeyCode::D) {
                delta.x += speed;
            }
            if input.keyboard.pressed(KeyCode::Left) || input.keyboard.pressed(KeyCode::A) {
                delta.x -= speed;
            }
            if input.keyboard.pressed(KeyCode::Up) || input.keyboard.pressed(KeyCode::W) {
                delta.y += speed;
            }
            if input.keyboard.pressed(KeyCode::Down) || input.keyboard.pressed(KeyCode::S) {
                delta.y -= speed;
            }

            // Normalize diagonal movement so it is not faster.
            if delta.length_squared() > 0.0 {
                let normalized = delta.normalize() * speed * (1.0 / 60.0);
                transform.position.x += normalized.x;
                transform.position.y += normalized.y;
            }
        }
    }

    fn camera_follow(
        player_query: Query<(&Player, &Transform)>,
        mut cam_query: Query<(&Camera, &mut Transform)>,
    ) {
        let mut player_pos = Vec3::ZERO;
        for (_p, t) in player_query.iter() {
            player_pos = t.position;
        }

        for (_cam, cam_transform) in cam_query.iter_mut() {
            // Smooth follow: lerp toward player position.
            let lerp_speed = 0.1;
            cam_transform.position.x += (player_pos.x - cam_transform.position.x) * lerp_speed;
            cam_transform.position.y += (player_pos.y - cam_transform.position.y) * lerp_speed;
        }
    }

    let mut app = App::new();
    app.add_plugin(DefaultPlugins);

    // Register per-frame systems.
    app.add_system(move_player);
    app.add_system(camera_follow);

    // Headless runner: 120 frames at 60fps (2 seconds of simulation).
    app.set_runner(HeadlessRunner::new(120, 1.0 / 60.0));
    app.build_plugins();

    // Run startup logic directly.
    setup(&mut app.world);

    app.run();

    // Report results.
    let entity_count = app.world.entity_count();
    println!(
        "Sprite Demo: {} entities (1 camera + 1 player + 100 bg sprites), 120 frames complete.",
        entity_count
    );
}

// ===========================================================================
// Windowed mode
// ===========================================================================

#[cfg(feature = "windowed")]
fn main() {
    use arachne_app::{
        App, AppExit, Camera, Commands, DefaultPlugins, Res, Query, Transform,
        Vec2, Vec3, Color, Time, WindowedRunner,
    };
    use arachne_input::{InputSystem, KeyCode};
    use arachne_math::Rng;
    use arachne_render::{Sprite, TextureHandle};
    use arachne_window::WindowConfig;

    fn setup(mut commands: Commands) {
        // Camera at origin.
        commands.spawn((Camera::new(), Transform::IDENTITY));

        // Player sprite (green tint, at origin).
        let mut player_sprite = Sprite::new(TextureHandle(1));
        player_sprite.color = Color::GREEN;
        player_sprite.custom_size = Some(Vec2::new(2.0, 2.0));
        commands.spawn((
            Player,
            player_sprite,
            Transform::from_position(Vec3::new(0.0, 0.0, 0.1)),
        ));

        // 100 background sprites at random positions.
        let mut rng = Rng::seed(42);
        for _ in 0..100 {
            let x = rng.next_range_f32(-50.0, 50.0);
            let y = rng.next_range_f32(-50.0, 50.0);
            let r = rng.next_f32();
            let g = rng.next_f32();
            let b = rng.next_f32();

            let mut sprite = Sprite::new(TextureHandle(0));
            sprite.color = Color::rgb(r, g, b);
            commands.spawn((
                Background,
                sprite,
                Transform::from_position(Vec3::new(x, y, 0.0)),
            ));
        }
    }

    fn move_player(
        input: Res<InputSystem>,
        time: Res<Time>,
        mut query: Query<(&Player, &mut Transform)>,
    ) {
        let speed = 200.0 * time.delta_seconds();

        for (_player, transform) in query.iter_mut() {
            let mut delta = Vec2::ZERO;

            if input.keyboard.pressed(KeyCode::Right) || input.keyboard.pressed(KeyCode::D) {
                delta.x += speed;
            }
            if input.keyboard.pressed(KeyCode::Left) || input.keyboard.pressed(KeyCode::A) {
                delta.x -= speed;
            }
            if input.keyboard.pressed(KeyCode::Up) || input.keyboard.pressed(KeyCode::W) {
                delta.y += speed;
            }
            if input.keyboard.pressed(KeyCode::Down) || input.keyboard.pressed(KeyCode::S) {
                delta.y -= speed;
            }

            // Normalize diagonal movement so it is not faster.
            if delta.length_squared() > 0.0 {
                let normalized = delta.normalize() * speed;
                transform.position.x += normalized.x;
                transform.position.y += normalized.y;
            }
        }
    }

    fn camera_follow(
        player_query: Query<(&Player, &Transform)>,
        mut cam_query: Query<(&Camera, &mut Transform)>,
    ) {
        let mut player_pos = Vec3::ZERO;
        for (_p, t) in player_query.iter() {
            player_pos = t.position;
        }

        for (_cam, cam_transform) in cam_query.iter_mut() {
            let lerp_speed = 0.1;
            cam_transform.position.x += (player_pos.x - cam_transform.position.x) * lerp_speed;
            cam_transform.position.y += (player_pos.y - cam_transform.position.y) * lerp_speed;
        }
    }

    fn escape_to_exit(input: Res<InputSystem>, mut commands: Commands) {
        if input.keyboard.just_pressed(KeyCode::Escape) {
            commands.insert_resource(AppExit);
        }
    }

    let config = WindowConfig::default()
        .with_title("Sprite Demo")
        .with_size(800, 600);

    let mut app = App::new();
    app.add_plugin(DefaultPlugins);
    app.set_runner(WindowedRunner::new(config));
    app.add_startup_system(setup);
    app.add_system(move_player);
    app.add_system(camera_follow);
    app.add_system(escape_to_exit);
    app.run();
}
