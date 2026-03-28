//! Eren Game -- a 2D top-down tile-based adventure demo.
//!
//! Three rooms connected by doors. The player moves with WASD/arrow keys,
//! interacts with objects using E, and exits with Escape.
//!
//! Demonstrates: tilemaps, room transitions, collision, HUD text, input.
//!
//! Run:
//!     cargo run --example eren_game --features windowed

// ---------------------------------------------------------------------------
// Marker components
// ---------------------------------------------------------------------------

/// Marks the player entity.
#[derive(Clone, Copy, Debug)]
struct Player;

// ===========================================================================
// Windowed mode (the real game)
// ===========================================================================

#[cfg(feature = "windowed")]
fn main() {
    use arachne_app::{
        App, AppExit, Camera, Commands, DefaultPlugins, Res, ResMut, Query,
        ScreenTextBuffer, Time, Transform, Vec2, Vec3, Color, WindowedRunner,
    };
    use arachne_input::{InputSystem, KeyCode};
    use arachne_render::{Sprite, TextureHandle, TilemapLayer, Tile};
    use arachne_window::WindowConfig;

    // -----------------------------------------------------------------------
    // Constants
    // -----------------------------------------------------------------------

    const ROOM_W: usize = 20;
    const ROOM_H: usize = 15;
    const TILE_SIZE: f32 = 32.0;
    const PLAYER_SPEED: f32 = 150.0;
    const CAMERA_LERP: f32 = 0.08;

    // Tile indices
    const EMPTY: u16 = 0;
    const GRASS: u16 = 1;
    const DIRT: u16 = 2;
    const STONE_WALL: u16 = 3;
    const WATER: u16 = 4;
    const WOOD_FLOOR: u16 = 5;
    const DOOR: u16 = 6;
    const BRICK_WALL: u16 = 7;

    // -----------------------------------------------------------------------
    // Room data -- 3 rooms as const 2D arrays (row-major, y then x)
    // -----------------------------------------------------------------------

    /// Room 1 "Lobby": walls around the edge, wood floor inside, door on right.
    const ROOM_1: [[u16; ROOM_W]; ROOM_H] = {
        const W: u16 = STONE_WALL;
        const F: u16 = WOOD_FLOOR;
        const D: u16 = DOOR;
        const G: u16 = GRASS;
        [
            [W, W, W, W, W, W, W, W, W, W, W, W, W, W, W, W, W, W, W, W],
            [W, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, W],
            [W, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, W],
            [W, F, F, G, G, F, F, F, F, F, F, F, F, F, G, G, F, F, F, W],
            [W, F, F, G, G, F, F, F, F, F, F, F, F, F, G, G, F, F, F, W],
            [W, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, W],
            [W, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, W],
            [W, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, D],
            [W, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, W],
            [W, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, W],
            [W, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, W],
            [W, F, F, G, G, F, F, F, F, F, F, F, F, F, G, G, F, F, F, W],
            [W, F, F, G, G, F, F, F, F, F, F, F, F, F, G, G, F, F, F, W],
            [W, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, W],
            [W, W, W, W, W, W, W, W, W, W, W, W, W, W, W, W, W, W, W, W],
        ]
    };

    /// Room 2 "Projects": walls, display objects, doors on left and right.
    const ROOM_2: [[u16; ROOM_W]; ROOM_H] = {
        const W: u16 = STONE_WALL;
        const B: u16 = BRICK_WALL;
        const F: u16 = WOOD_FLOOR;
        const D: u16 = DOOR;
        const T: u16 = DIRT;
        const A: u16 = WATER;
        [
            [W, W, W, W, W, W, W, W, W, W, W, W, W, W, W, W, W, W, W, W],
            [W, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, W],
            [W, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, W],
            [W, F, F, B, B, B, F, F, F, F, F, F, F, F, B, B, B, F, F, W],
            [W, F, F, B, T, B, F, F, F, F, F, F, F, F, B, A, B, F, F, W],
            [W, F, F, B, B, B, F, F, F, F, F, F, F, F, B, B, B, F, F, W],
            [W, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, W],
            [D, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, D],
            [W, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, W],
            [W, F, F, B, B, B, F, F, F, F, F, F, F, F, B, B, B, F, F, W],
            [W, F, F, B, T, B, F, F, F, F, F, F, F, F, B, A, B, F, F, W],
            [W, F, F, B, B, B, F, F, F, F, F, F, F, F, B, B, B, F, F, W],
            [W, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, W],
            [W, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, W],
            [W, W, W, W, W, W, W, W, W, W, W, W, W, W, W, W, W, W, W, W],
        ]
    };

    /// Room 3 "About": walls, interactable objects, door on left.
    const ROOM_3: [[u16; ROOM_W]; ROOM_H] = {
        const W: u16 = STONE_WALL;
        const B: u16 = BRICK_WALL;
        const F: u16 = WOOD_FLOOR;
        const D: u16 = DOOR;
        const G: u16 = GRASS;
        const A: u16 = WATER;
        [
            [W, W, W, W, W, W, W, W, W, W, W, W, W, W, W, W, W, W, W, W],
            [W, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, W],
            [W, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, W],
            [W, F, F, F, F, F, F, F, B, B, B, B, F, F, F, F, F, F, F, W],
            [W, F, F, F, F, F, F, F, B, G, G, B, F, F, F, F, F, F, F, W],
            [W, F, F, F, F, F, F, F, B, G, G, B, F, F, F, F, F, F, F, W],
            [W, F, F, F, F, F, F, F, B, B, B, B, F, F, F, F, F, F, F, W],
            [D, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, W],
            [W, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, W],
            [W, F, F, A, A, A, F, F, F, F, F, F, F, F, A, A, A, F, F, W],
            [W, F, F, A, A, A, F, F, F, F, F, F, F, F, A, A, A, F, F, W],
            [W, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, W],
            [W, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, W],
            [W, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, W],
            [W, W, W, W, W, W, W, W, W, W, W, W, W, W, W, W, W, W, W, W],
        ]
    };

    // -----------------------------------------------------------------------
    // Room names and dialogue
    // -----------------------------------------------------------------------

    const ROOM_NAMES: [&str; 3] = ["Lobby", "Projects", "About"];

    const ROOM_DIALOGUES: [&str; 3] = [
        "Welcome to Eren's World! Use arrow keys to move.",
        "Project: Arachne Engine - Sub-1MB embeddable runtime",
        "Built with Rust, WASM-first, runs in your browser",
    ];

    // -----------------------------------------------------------------------
    // Game state resource
    // -----------------------------------------------------------------------

    struct GameState {
        current_room: usize,
        show_dialogue: bool,
        dialogue_text: String,
        near_interactable: bool,
        transition_cooldown: f32,
    }

    impl Default for GameState {
        fn default() -> Self {
            Self {
                current_room: 0,
                show_dialogue: false,
                dialogue_text: String::new(),
                near_interactable: false,
                transition_cooldown: 0.0,
            }
        }
    }

    // Use the engine's TilemapRendererResource — just modify its .layers field.
    use arachne_app::TilemapRendererResource;

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn get_room_data(room: usize) -> &'static [[u16; ROOM_W]; ROOM_H] {
        match room {
            0 => &ROOM_1,
            1 => &ROOM_2,
            2 => &ROOM_3,
            _ => &ROOM_1,
        }
    }

    fn is_solid(tile_index: u16) -> bool {
        tile_index == STONE_WALL || tile_index == BRICK_WALL
    }

    fn is_door(tile_index: u16) -> bool {
        tile_index == DOOR
    }

    /// Check if a tile index is one the player can interact with for dialogue.
    fn is_interactable(tile_index: u16) -> bool {
        tile_index == GRASS || tile_index == WATER || tile_index == DIRT
    }

    fn build_tilemap_layer(room: usize) -> TilemapLayer {
        let data = get_room_data(room);
        let mut layer = TilemapLayer::new(
            ROOM_W as u32,
            ROOM_H as u32,
            Vec2::new(TILE_SIZE, TILE_SIZE),
            8, // atlas columns (not used for colored tiles, but required)
            8, // atlas rows
        );
        for y in 0..ROOM_H {
            for x in 0..ROOM_W {
                let idx = data[y][x];
                if idx != EMPTY {
                    layer.set_tile(x as u32, y as u32, Some(Tile::new(idx)));
                }
            }
        }
        layer
    }

    /// Convert pixel position to tile coordinates.
    fn pixel_to_tile(px: f32, py: f32) -> (usize, usize) {
        let tx = (px / TILE_SIZE).floor() as isize;
        let ty = (py / TILE_SIZE).floor() as isize;
        (
            tx.clamp(0, ROOM_W as isize - 1) as usize,
            ty.clamp(0, ROOM_H as isize - 1) as usize,
        )
    }

    /// Check if a pixel position would collide with a solid tile.
    fn would_collide(room: usize, px: f32, py: f32) -> bool {
        let data = get_room_data(room);
        // Check corners of the player bounding box (24x24 centered on position)
        let half = 10.0; // slightly smaller than 12 for forgiveness
        let corners = [
            (px - half, py - half),
            (px + half, py - half),
            (px - half, py + half),
            (px + half, py + half),
        ];
        for (cx, cy) in corners {
            let (tx, ty) = pixel_to_tile(cx, cy);
            if tx < ROOM_W && ty < ROOM_H && is_solid(data[ty][tx]) {
                return true;
            }
        }
        false
    }

    // -----------------------------------------------------------------------
    // Systems
    // -----------------------------------------------------------------------

    /// Setup: spawn camera, spawn player sprite, build initial room tilemap.
    fn setup(mut commands: Commands) {
        // Camera at player start position.
        let start_x = 3.0 * TILE_SIZE + TILE_SIZE * 0.5;
        let start_y = 7.0 * TILE_SIZE + TILE_SIZE * 0.5;
        commands.spawn((
            Camera::new(),
            Transform::from_position(Vec3::new(start_x, start_y, 0.0)),
        ));

        // Player sprite: green square, 24x24 pixels.
        let mut player_sprite = Sprite::new(TextureHandle(0));
        player_sprite.color = Color::rgb(0.2, 0.85, 0.3);
        player_sprite.custom_size = Some(Vec2::new(24.0, 24.0));
        commands.spawn((
            Player,
            player_sprite,
            Transform::from_position(Vec3::new(start_x, start_y, 0.1)),
        ));

        // Game state resource.
        commands.insert_resource(GameState::default());
    }

    /// Load the initial room tilemap into the engine's TilemapRendererResource.
    fn setup_tilemap(mut tilemap: ResMut<TilemapRendererResource>) {
        let layer = build_tilemap_layer(0);
        tilemap.layers = vec![layer];
    }

    /// Player movement with tile collision.
    fn player_movement(
        input: Res<InputSystem>,
        time: Res<Time>,
        state: Res<GameState>,
        mut query: Query<(&Player, &mut Transform)>,
    ) {
        let dt = time.delta_seconds();
        let speed = PLAYER_SPEED * dt;

        for (_player, transform) in query.iter_mut() {
            let mut dx = 0.0f32;
            let mut dy = 0.0f32;

            if input.keyboard.pressed(KeyCode::Right) || input.keyboard.pressed(KeyCode::D) {
                dx += 1.0;
            }
            if input.keyboard.pressed(KeyCode::Left) || input.keyboard.pressed(KeyCode::A) {
                dx -= 1.0;
            }
            if input.keyboard.pressed(KeyCode::Up) || input.keyboard.pressed(KeyCode::W) {
                dy -= 1.0; // top-down: up is negative Y in tile space
            }
            if input.keyboard.pressed(KeyCode::Down) || input.keyboard.pressed(KeyCode::S) {
                dy += 1.0;
            }

            // Normalize diagonal movement.
            let len_sq = dx * dx + dy * dy;
            if len_sq > 0.0 {
                let inv_len = 1.0 / len_sq.sqrt();
                dx *= inv_len * speed;
                dy *= inv_len * speed;
            }

            // Try horizontal movement.
            let new_x = transform.position.x + dx;
            if !would_collide(state.current_room, new_x, transform.position.y) {
                transform.position.x = new_x;
            }

            // Try vertical movement.
            let new_y = transform.position.y + dy;
            if !would_collide(state.current_room, transform.position.x, new_y) {
                transform.position.y = new_y;
            }
        }
    }

    /// Smooth camera follow.
    fn camera_follow(
        player_query: Query<(&Player, &Transform)>,
        mut cam_query: Query<(&Camera, &mut Transform)>,
    ) {
        let mut player_pos = Vec3::ZERO;
        for (_p, t) in player_query.iter() {
            player_pos = t.position;
        }

        for (_cam, cam_transform) in cam_query.iter_mut() {
            cam_transform.position.x +=
                (player_pos.x - cam_transform.position.x) * CAMERA_LERP;
            cam_transform.position.y +=
                (player_pos.y - cam_transform.position.y) * CAMERA_LERP;
        }
    }

    /// Room transition: check if player is on a door tile.
    fn room_transition(
        time: Res<Time>,
        mut state: ResMut<GameState>,
        mut tilemap: ResMut<TilemapRendererResource>,
        mut query: Query<(&Player, &mut Transform)>,
    ) {
        // Cooldown to prevent rapid toggling.
        if state.transition_cooldown > 0.0 {
            state.transition_cooldown -= time.delta_seconds();
            return;
        }

        for (_player, transform) in query.iter_mut() {
            let (tx, ty) = pixel_to_tile(transform.position.x, transform.position.y);
            let data = get_room_data(state.current_room);

            if tx < ROOM_W && ty < ROOM_H && is_door(data[ty][tx]) {
                let old_room = state.current_room;
                let new_room;
                let spawn_x;
                let spawn_y;

                match old_room {
                    0 => {
                        // Room 1 door is on the right -> go to room 2, spawn at left door.
                        new_room = 1;
                        spawn_x = 1;
                        spawn_y = 7;
                    }
                    1 => {
                        if tx == 0 {
                            // Left door -> back to room 1, spawn at right-side interior.
                            new_room = 0;
                            spawn_x = 18;
                            spawn_y = 7;
                        } else {
                            // Right door -> go to room 3, spawn at left door.
                            new_room = 2;
                            spawn_x = 1;
                            spawn_y = 7;
                        }
                    }
                    2 => {
                        // Room 3 left door -> back to room 2, spawn at right-side interior.
                        new_room = 1;
                        spawn_x = 18;
                        spawn_y = 7;
                    }
                    _ => continue,
                }

                state.current_room = new_room;
                state.transition_cooldown = 0.5;
                state.show_dialogue = false;

                // Teleport player.
                transform.position.x = spawn_x as f32 * TILE_SIZE + TILE_SIZE * 0.5;
                transform.position.y = spawn_y as f32 * TILE_SIZE + TILE_SIZE * 0.5;

                // Rebuild tilemap layer.
                let layer = build_tilemap_layer(new_room);
                if tilemap.layers.is_empty() {
                    tilemap.layers.push(layer);
                } else {
                    tilemap.layers[0] = layer;
                }
            }
        }
    }

    /// Interaction: check if player is near an interactable tile, handle E press.
    fn interaction(
        input: Res<InputSystem>,
        mut state: ResMut<GameState>,
        query: Query<(&Player, &Transform)>,
    ) {
        let mut near = false;

        for (_player, transform) in query.iter() {
            let (tx, ty) = pixel_to_tile(transform.position.x, transform.position.y);
            let data = get_room_data(state.current_room);

            // Check tiles in the 3x3 neighborhood around the player.
            let min_x = if tx > 0 { tx - 1 } else { 0 };
            let max_x = if tx + 1 < ROOM_W { tx + 1 } else { ROOM_W - 1 };
            let min_y = if ty > 0 { ty - 1 } else { 0 };
            let max_y = if ty + 1 < ROOM_H { ty + 1 } else { ROOM_H - 1 };

            for ny in min_y..=max_y {
                for nx in min_x..=max_x {
                    if is_interactable(data[ny][nx]) {
                        near = true;
                    }
                }
            }
        }

        state.near_interactable = near;

        if near && input.keyboard.just_pressed(KeyCode::E) {
            state.show_dialogue = !state.show_dialogue;
            if state.show_dialogue {
                state.dialogue_text = ROOM_DIALOGUES[state.current_room].to_string();
            }
        }

        // Dismiss dialogue if player moves away.
        if !near {
            state.show_dialogue = false;
        }
    }

    /// HUD: draw room name, interaction prompt, and dialogue via ScreenTextBuffer.
    fn hud(
        state: Res<GameState>,
        mut text_buf: ResMut<ScreenTextBuffer>,
    ) {
        // Room name at top-center.
        let room_name = format!("Room: {}", ROOM_NAMES[state.current_room]);
        text_buf.draw(
            room_name,
            Vec2::new(320.0, 16.0),
            24.0,
            Color::WHITE,
        );

        // Navigation hint.
        text_buf.draw(
            "WASD/Arrows: Move | E: Interact | ESC: Quit",
            Vec2::new(160.0, 560.0),
            14.0,
            Color::rgb(0.7, 0.7, 0.7),
        );

        // Interaction prompt.
        if state.near_interactable && !state.show_dialogue {
            text_buf.draw(
                "Press E to interact",
                Vec2::new(300.0, 520.0),
                18.0,
                Color::rgb(1.0, 1.0, 0.4),
            );
        }

        // Dialogue box at bottom.
        if state.show_dialogue {
            text_buf.draw(
                &state.dialogue_text,
                Vec2::new(100.0, 480.0),
                20.0,
                Color::rgb(0.9, 0.95, 1.0),
            );
        }
    }

    /// Escape key exits the application.
    fn escape_to_exit(input: Res<InputSystem>, mut commands: Commands) {
        if input.keyboard.just_pressed(KeyCode::Escape) {
            commands.insert_resource(AppExit);
        }
    }

    // -----------------------------------------------------------------------
    // Entry point
    // -----------------------------------------------------------------------

    let config = WindowConfig::default()
        .with_title("Eren Game - Arachne Engine")
        .with_size(800, 600);

    let mut app = App::new();
    app.add_plugin(DefaultPlugins);
    app.set_runner(WindowedRunner::new(config));
    app.add_startup_system(setup);
    app.add_startup_system(setup_tilemap);
    app.add_system(player_movement);
    app.add_system(camera_follow);
    app.add_system(room_transition);
    app.add_system(interaction);
    app.add_system(hud);
    app.add_system(escape_to_exit);
    app.run();
}

// ===========================================================================
// Non-windowed fallback
// ===========================================================================

#[cfg(not(feature = "windowed"))]
fn main() {
    println!("The Eren Game requires --features windowed");
    println!("Run with: cargo run --example eren_game --features windowed");
}
