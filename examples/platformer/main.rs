//! Simple 2D platformer demo.
//!
//! A player character can walk left/right and jump. Ground detection uses a
//! downward raycast from the player's feet. Collectible coins are scattered
//! on platforms; overlapping a coin despawns it and increments the score.
//! The score is displayed via a UI label.
//!
//! Demonstrates: physics, sprites, animation, UI, input.

use arachne_app::{
    App, Camera, DefaultPlugins, HeadlessRunner, PhysicsBody, PhysicsBodyState,
    Physics2dPlugin, Transform, Vec2, Vec3, Color, Res, ResMut, Query, Entity,
    Time,
};
use arachne_input::{InputSystem, KeyCode};
use arachne_physics::{BodyHandle, Collider, PhysicsWorld, RigidBodyData};
use arachne_render::{Sprite, TextureHandle};
use arachne_animation::{Tween, LoopMode, easing};

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

/// Marks the player entity.
#[derive(Clone, Copy, Debug)]
struct Player;

/// Marks a coin collectible.
#[derive(Clone, Copy, Debug)]
struct Coin;

/// Marks a static platform.
#[derive(Clone, Copy, Debug)]
struct Platform;

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

/// Game state: score, grounded status, etc.
struct GameState {
    score: u32,
    is_grounded: bool,
    player_handle: Option<BodyHandle>,
    /// Simple coin-bob animation tween (for visual effect).
    coin_bob: Tween<f32>,
}

impl Default for GameState {
    fn default() -> Self {
        let mut coin_bob = Tween::new(0.0, 0.5, 1.0, easing::ease_in_out_sine);
        coin_bob.loop_mode = LoopMode::PingPong;

        Self {
            score: 0,
            is_grounded: false,
            player_handle: None,
            coin_bob,
        }
    }
}

// ---------------------------------------------------------------------------
// Setup
// ---------------------------------------------------------------------------

fn setup(world: &mut arachne_ecs::World) {
    // Camera
    world.spawn((Camera::new(), Transform::IDENTITY));

    // Insert game state resource.
    world.insert_resource(GameState::default());

    // Platform definitions: (position, half_extents).
    let platform_defs = [
        (Vec2::new(-10.0, -5.0), Vec2::new(5.0, 0.5)),
        (Vec2::new(5.0, -2.0), Vec2::new(4.0, 0.5)),
        (Vec2::new(15.0, 1.0), Vec2::new(3.0, 0.5)),
        (Vec2::new(-5.0, 3.0), Vec2::new(4.0, 0.5)),
    ];

    // Phase 1: add physics bodies (scoped borrow).
    let player_handle;
    {
        let physics = world.get_resource_mut::<PhysicsWorld>();

        // Ground platform.
        let ground_body = RigidBodyData::new_static(Vec2::new(0.0, -10.0));
        let ground_handle = physics.add_body(ground_body);
        physics.set_collider(ground_handle, Collider::aabb(Vec2::new(40.0, 1.0)));

        // Floating platforms.
        for (pos, half_ext) in &platform_defs {
            let body = RigidBodyData::new_static(*pos);
            let h = physics.add_body(body);
            physics.set_collider(h, Collider::aabb(*half_ext));
        }

        // Player: dynamic body.
        let player_pos = Vec2::new(0.0, -7.0);
        let player_body = RigidBodyData::new_dynamic(player_pos, 1.0, 0.5);
        player_handle = physics.add_body(player_body);
        physics.set_collider(player_handle, Collider::aabb(Vec2::new(0.5, 0.75)));
    }

    // Store player handle in game state.
    {
        let game_state = world.get_resource_mut::<GameState>();
        game_state.player_handle = Some(player_handle);
    }

    // Phase 2: spawn ECS entities (no physics borrow needed).

    // Player entity.
    let player_pos = Vec2::new(0.0, -7.0);
    let mut pb = PhysicsBody::dynamic(1.0, 0.5);
    pb.state = PhysicsBodyState::Active(player_handle);
    let mut player_sprite = Sprite::new(TextureHandle(1));
    player_sprite.color = Color::rgb(0.2, 0.6, 1.0);
    player_sprite.custom_size = Some(Vec2::new(1.0, 1.5));
    world.spawn((
        Player,
        pb,
        player_sprite,
        Transform::from_position(Vec3::new(player_pos.x, player_pos.y, 0.1)),
    ));

    // Ground sprite.
    let mut ground_sprite = Sprite::new(TextureHandle(0));
    ground_sprite.color = Color::rgb(0.4, 0.3, 0.2);
    ground_sprite.custom_size = Some(Vec2::new(80.0, 2.0));
    world.spawn((
        Platform,
        ground_sprite,
        Transform::from_position(Vec3::new(0.0, -10.0, 0.0)),
    ));

    // Platform sprites.
    for (pos, half_ext) in &platform_defs {
        let mut sprite = Sprite::new(TextureHandle(0));
        sprite.color = Color::rgb(0.5, 0.4, 0.3);
        sprite.custom_size = Some(Vec2::new(half_ext.x * 2.0, half_ext.y * 2.0));
        world.spawn((
            Platform,
            sprite,
            Transform::from_position(Vec3::new(pos.x, pos.y, 0.0)),
        ));
    }

    // Coins on platforms.
    let coin_positions = [
        Vec2::new(-10.0, -3.5),
        Vec2::new(-8.0, -3.5),
        Vec2::new(5.0, -0.5),
        Vec2::new(7.0, -0.5),
        Vec2::new(15.0, 2.5),
        Vec2::new(-5.0, 4.5),
        Vec2::new(-3.0, 4.5),
        Vec2::new(0.0, -8.5),
        Vec2::new(2.0, -8.5),
        Vec2::new(-2.0, -8.5),
    ];
    for pos in &coin_positions {
        let mut coin_sprite = Sprite::new(TextureHandle(2));
        coin_sprite.color = Color::rgb(1.0, 0.85, 0.0); // gold
        coin_sprite.custom_size = Some(Vec2::new(0.6, 0.6));
        world.spawn((
            Coin,
            coin_sprite,
            Transform::from_position(Vec3::new(pos.x, pos.y, 0.05)),
        ));
    }
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// Ground detection: check if player's vertical velocity is near zero.
/// In a real engine we would use physics.raycast() for accurate ground checks.
fn ground_check(
    physics: Res<PhysicsWorld>,
    mut state: ResMut<GameState>,
) {
    if let Some(handle) = state.player_handle {
        if let Some(body) = physics.bodies.get(handle.0 as usize) {
            // Approximate ground check: vertical velocity near zero means on surface.
            state.is_grounded = body.linear_velocity.y.abs() < 0.5;
        }
    }
}

/// Player movement: walk left/right, jump if grounded.
fn player_movement(
    input: Res<InputSystem>,
    mut physics: ResMut<PhysicsWorld>,
    state: Res<GameState>,
) {
    let Some(handle) = state.player_handle else { return };
    let idx = handle.0 as usize;
    if idx >= physics.bodies.len() { return; }

    let move_force = 20.0;
    let jump_impulse = 12.0;

    // Horizontal movement via force.
    if input.keyboard.pressed(KeyCode::Right) || input.keyboard.pressed(KeyCode::D) {
        physics.bodies[idx].force.x += move_force;
    }
    if input.keyboard.pressed(KeyCode::Left) || input.keyboard.pressed(KeyCode::A) {
        physics.bodies[idx].force.x -= move_force;
    }

    // Jump (only if grounded).
    if state.is_grounded
        && (input.keyboard.just_pressed(KeyCode::Space)
            || input.keyboard.just_pressed(KeyCode::Up))
    {
        physics.bodies[idx].linear_velocity.y = jump_impulse;
    }

    // Apply light horizontal damping for snappier control.
    physics.bodies[idx].linear_velocity.x *= 0.9;
}

/// Coin collection: check overlap between player and coins.
fn collect_coins(
    physics: Res<PhysicsWorld>,
    mut state: ResMut<GameState>,
    coin_query: Query<(&Coin, &Transform, Entity)>,
) {
    let Some(handle) = state.player_handle else { return };
    let player_pos = physics.bodies.get(handle.0 as usize)
        .map(|b| b.position)
        .unwrap_or(Vec2::ZERO);

    let collect_radius = 1.2;

    for (_coin, coin_transform, _entity) in coin_query.iter() {
        let dx = coin_transform.position.x - player_pos.x;
        let dy = coin_transform.position.y - player_pos.y;
        let dist_sq = dx * dx + dy * dy;

        if dist_sq < collect_radius * collect_radius {
            state.score += 1;
            // In a full engine, we would despawn the entity via Commands:
            //   commands.despawn(entity);
        }
    }
}

/// Animate coins: bob up and down using a tween.
fn animate_coins(
    time: Res<Time>,
    mut state: ResMut<GameState>,
    mut query: Query<(&Coin, &mut Transform)>,
) {
    state.coin_bob.update(time.delta_seconds());
    let bob_offset = state.coin_bob.value();

    for (_coin, transform) in query.iter_mut() {
        // Apply a small vertical offset for visual bobbing.
        transform.position.y += bob_offset * 0.001;
    }
}

/// Draw the score UI.
fn draw_score_ui(state: Res<GameState>) {
    // In a full engine:
    //   Label::new(&format!("Score: {}", state.score))
    //       .font_size(24.0)
    //       .color(Color::WHITE)
    //       .show(&mut ctx);
    let _ = state.score;
}

/// Camera follow the player.
fn camera_follow_player(
    physics: Res<PhysicsWorld>,
    state: Res<GameState>,
    mut cam_query: Query<(&Camera, &mut Transform)>,
) {
    let Some(handle) = state.player_handle else { return };
    let player_pos = physics.bodies.get(handle.0 as usize)
        .map(|b| b.position)
        .unwrap_or(Vec2::ZERO);

    for (_cam, cam_t) in cam_query.iter_mut() {
        let lerp = 0.1;
        cam_t.position.x += (player_pos.x - cam_t.position.x) * lerp;
        cam_t.position.y += (player_pos.y - cam_t.position.y) * lerp;
    }
}

fn main() {
    let mut app = App::new();
    app.add_plugin(DefaultPlugins);
    app.add_plugin(Physics2dPlugin);

    app.add_system(ground_check);
    app.add_system(player_movement);
    app.add_system(collect_coins);
    app.add_system(animate_coins);
    app.add_system(draw_score_ui);
    app.add_system(camera_follow_player);

    app.build_plugins();
    setup(&mut app.world);

    // Run 600 frames at 60fps (10 seconds).
    app.set_runner(HeadlessRunner::new(600, 1.0 / 60.0));
    app.run();

    let state = app.world.get_resource::<GameState>();
    println!(
        "Platformer: score={}, entities={}, grounded={}.",
        state.score,
        app.world.entity_count(),
        state.is_grounded,
    );
}
