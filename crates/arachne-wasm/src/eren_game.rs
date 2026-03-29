//! Eren's World: narrative-driven tile adventure portfolio game.
//!
//! 8 rooms connected as a spider's web. Each room tells a chapter of Eren's
//! builder story. Spider companion narrates. Silk thread collectibles unlock
//! a secret ending.

#[cfg(all(target_arch = "wasm32", feature = "wasm"))]
pub fn run(canvas_id: &str) {
    use arachne_app::{
        App, Camera, Commands, DefaultPlugins, Entity, Query, Res, ResMut, Runner,
        ScreenTextBuffer, Time, Transform, Vec2, Vec3, Color,
        TilemapRendererResource,
    };
    use arachne_input::{InputSystem, KeyCode};
    use arachne_render::{Sprite, TextureHandle, TilemapLayer, Tile};
    use crate::WasmRunner;

    // ===================================================================
    // Constants
    // ===================================================================

    const ROOM_W: usize = 20;
    const ROOM_H: usize = 15;
    const TILE_SIZE: f32 = 32.0;
    const PLAYER_SPEED: f32 = 150.0;
    const CAMERA_LERP: f32 = 0.08;
    const NUM_ROOMS: usize = 8;
    const NUM_SILK: usize = 8;
    const TRANSITION_COOLDOWN: f32 = 0.5;

    // Tile indices (match builtin_tiles atlas)
    const EMPTY: u16 = 0;
    const GRASS: u16 = 1;
    const _DIRT: u16 = 2;
    const STONE_WALL: u16 = 3;
    const WATER: u16 = 4;
    const WOOD_FLOOR: u16 = 5;
    const DOOR: u16 = 6;
    const BRICK_WALL: u16 = 7;
    const DARK_GRASS: u16 = 8;
    const LIGHT_DIRT: u16 = 9;
    const DARK_STONE: u16 = 10;
    const DEEP_WATER: u16 = 11;
    const _LIGHT_WOOD: u16 = 12;
    const METAL_DOOR: u16 = 13;
    const MOSSY_BRICK: u16 = 14;
    const SAND: u16 = 15;
    const _SNOW: u16 = 16;
    const ICE: u16 = 17;
    const LAVA: u16 = 18;
    const COBBLESTONE: u16 = 19;
    const GRAVEL: u16 = 20;
    const PLANKS: u16 = 21;
    const MARBLE: u16 = 22;
    const DARK_BRICK: u16 = 23;
    const RED: u16 = 24;
    const GREEN: u16 = 25;
    const BLUE: u16 = 26;
    const YELLOW: u16 = 27;
    const MAGENTA: u16 = 28;
    const CYAN: u16 = 29;
    const _LIGHT_GRAY: u16 = 30;
    const DARK_GRAY: u16 = 31;

    // ===================================================================
    // Room tile grids (8 rooms, each 20x15)
    // ===================================================================

    // Room 0: Welcome Screen — spider web on void
    const ROOM_0: [[u16; ROOM_W]; ROOM_H] = [
        [ 0, 0, 0,31, 0, 0, 0, 0, 0,31, 0, 0, 0, 0, 0, 0,31, 0, 0, 0],
        [ 0, 0, 0, 0,31, 0, 0, 0, 0,31, 0, 0, 0, 0, 0,31, 0, 0, 0, 0],
        [ 0, 0, 0, 0, 0,31, 0, 0, 0,31, 0, 0, 0, 0,31, 0, 0, 0, 0, 0],
        [ 0, 0, 0, 0, 0, 0,31, 0, 0,31, 0, 0, 0,31, 0, 0, 0, 0, 0, 0],
        [ 0, 0, 0, 0, 0, 0, 0,31, 0,31, 0, 0,31, 0, 0, 0, 0, 0, 0, 0],
        [ 0, 0, 0, 0, 0, 0, 0, 0,31,31, 0,31, 0, 0, 0, 0, 0, 0, 0, 0],
        [ 0, 0, 0, 0, 0, 0, 0, 0, 0,31,31, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        [31,31,31,31,31,31,31,31,31,24,24,31,31,31,31,31,31,31,31,31],
        [ 0, 0, 0, 0, 0, 0, 0, 0, 0,31,31, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        [ 0, 0, 0, 0, 0, 0, 0, 0,31,31, 0,31, 0, 0, 0, 0, 0, 0, 0, 0],
        [ 0, 0, 0, 0, 0, 0, 0,31, 0,31, 0, 0,31, 0, 0, 0, 0, 0, 0, 0],
        [ 0, 0, 0, 0, 0, 0,31, 0, 0,31, 0, 0, 0,31, 0, 0, 0, 0, 0, 0],
        [ 0, 0, 0, 0, 0,31, 0, 0, 0,31, 0, 0, 0, 0,31, 0, 0, 0, 0, 0],
        [ 0, 0, 0, 0,31, 0, 0, 0, 0,31, 0, 0, 0, 0, 0,31, 0, 0, 0, 0],
        [ 0, 0, 0,31, 0, 0, 0, 0, 0,31, 0, 0, 0, 0, 0, 0,31, 0, 0, 0],
    ];

    // Room 1: The Nexus — cobblestone + gravel web, 4 doors
    const ROOM_1: [[u16; ROOM_W]; ROOM_H] = [
        [ 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 6, 3, 3, 3, 3, 3, 3, 3, 3, 3],
        [ 3,19,19,20,19,19,19,19,19,20,20,20,19,19,19,19,20,19,19, 3],
        [ 3,19,19,19,20,19,19,19,19,20,20,19,19,19,19,20,19,19,19, 3],
        [ 3,19,19,19,19,20,19,19,19,20,20,19,19,19,20,19,19,19,19, 3],
        [ 3,19,19,19,19,19,20,19,19,20,20,19,19,20,19,19,19,19,19, 3],
        [ 3,19,19,19,19,19,19,20,19,20,20,19,20,19,19,19,19,19,19, 3],
        [ 3,19,19,19,19,19,19,19,20,20,20,20,19,19,19,19,19,19,19, 3],
        [ 6,20,20,20,20,20,20,20,20,24,24,20,20,20,20,20,20,20,20, 6],
        [ 3,19,19,19,19,19,19,19,20,20,20,20,19,19,19,19,19,19,19, 3],
        [ 3,19,19,19,19,19,19,20,19,20,20,19,20,19,19,19,19,19,19, 3],
        [ 3,19,19,19,19,19,20,19,19,20,20,19,19,20,19,19,19,19,19, 3],
        [ 3,19,19,19,19,20,19,19,19,20,20,19,19,19,20,19,19,19,19, 3],
        [ 3,19,19,19,20,19,19,19,19,20,20,19,19,19,19,20,19,19,19, 3],
        [ 3,19,19,20,19,19,19,19,19,20,20,19,19,19,19,19,20,19,19, 3],
        [ 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 6, 3, 3, 3, 3, 3, 3, 3, 3, 3],
    ];

    // Room 2: The Forge — dark stone, lava river, bridge
    const ROOM_2: [[u16; ROOM_W]; ROOM_H] = [
        [10,10,10,10,10,10,10,10,10,10,10,10,10,10,10,10,10,10,10,10],
        [10, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5,10],
        [10, 5,23,23,23, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5,23,23,23, 5,10],
        [10, 5,23,29,23, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5,23,27,23, 5,10],
        [10, 5,23,23,23, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5,23,23,23, 5,10],
        [10, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5,10],
        [10,18,18,18,18,18,18,18,18, 5, 5,18,18,18,18,18,18,18,18,10],
        [10,18,18,18,18,18,18,18,18, 5, 5,18,18,18,18,18,18,18,18,10],
        [10, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5,10],
        [10, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5,10],
        [10, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5,10],
        [10, 5, 5, 5, 5, 5, 5,23,23,23,23,23,23, 5, 5, 5, 5, 5, 5,10],
        [10, 5, 5, 5, 5, 5, 5,23,24,31,31,24,23, 5, 5, 5, 5, 5, 5,10],
        [10, 5, 5, 5, 5, 5, 5,23,23,23,23,23,23, 5, 5, 5, 5, 5, 5,10],
        [10,10,10,10,10,10,10,10,10,10, 6,10,10,10,10,10,10,10,10,10],
    ];

    // Room 3: The Hive — planks, server racks, workstations
    const ROOM_3: [[u16; ROOM_W]; ROOM_H] = [
        [ 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3],
        [ 3,21,21,21,21,21,21,21,21,21,21,21,21,21,21,21,21,21,21, 3],
        [ 3,21,13,29,13,21,13,29,13,21,21,13,29,13,21,13,29,13,21, 3],
        [ 3,21,21,21,21,21,21,21,21,21,21,21,21,21,21,21,21,21,21, 3],
        [ 3,21,21,21,21,21,21,21,21,21,21,21,21,21,21,21,21,21,21, 3],
        [ 3,21,21,21, 3, 3, 3,21,21,21,21,21,21, 3, 3, 3,21,21,21, 3],
        [ 3,21,21,21, 3,31, 3,21,21,21,21,21,21, 3,29, 3,21,21,21, 3],
        [ 3,21,21,21, 3, 3, 3,21,21,21,21,21,21, 3, 3, 3,21,21,21, 6],
        [ 3,21,21,21,21,21,21,21,21,21,21,21,21,21,21,21,21,21,21, 3],
        [ 3,21,21,21,21,21,21,21,21,21,21,21,21,21,21,21,21,21,21, 3],
        [ 3,21,21,21,21,21,21,21,21,21,21,21,21,21,21,21,21,21,21, 3],
        [ 3,21, 3, 3, 3,21,21,21,21,21,21,21,21,21,21, 3, 3, 3,21, 3],
        [ 3,21, 3,28, 3,21,21,21,21,21,21,21,21,21,21, 3,27, 3,21, 3],
        [ 3,21, 3, 3, 3,21,21,21,21,21,21,21,21,21,21, 3, 3, 3,21, 3],
        [ 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3],
    ];

    // Room 4: The Lens — marble, ice crystal formation
    const ROOM_4: [[u16; ROOM_W]; ROOM_H] = [
        [ 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3],
        [ 3,22,22,22,22,22,22,22,22,22,22,22,22,22,22,22,22,22,22, 3],
        [ 3,22, 3, 3, 3,22,22,22,22,22,22,22,22,22,22, 3, 3, 3,22, 3],
        [ 3,22, 3,26, 3,22,22,22,22,22,22,22,22,22,22, 3,29, 3,22, 3],
        [ 3,22, 3, 3, 3,22,22,22,22,22,22,22,22,22,22, 3, 3, 3,22, 3],
        [ 3,22,22,22,22,22,22,22,17,17,17,17,22,22,22,22,22,22,22, 3],
        [ 3,22,22,22,22,22,22,17,17,17,17,17,17,22,22,22,22,22,22, 3],
        [ 6,22,22,22,22,22,17,17,17,29,29,17,17,17,22,22,22,22,22, 3],
        [ 3,22,22,22,22,22,22,17,17,17,17,17,17,22,22,22,22,22,22, 3],
        [ 3,22,22,22,22,22,22,22,17,17,17,17,22,22,22,22,22,22,22, 3],
        [ 3,22,22,22,22,22,22,22,22,22,22,22,22,22,22,22,22,22,22, 3],
        [ 3,22, 3, 3, 3,22,22,22,22,22,22,22,22,22,22, 3, 3, 3,22, 3],
        [ 3,22, 3,25, 3,22,22,22,22,22,22,22,22,22,22, 3,28, 3,22, 3],
        [ 3,22, 3, 3, 3,22,22,22,22,22,22,22,22,22,22, 3, 3, 3,22, 3],
        [ 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3],
    ];

    // Room 5: The Lab — dark grass, deep water pools, mossy brick exhibits
    const ROOM_5: [[u16; ROOM_W]; ROOM_H] = [
        [ 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 6, 3, 3, 3, 3, 3, 3, 3, 3, 3],
        [ 3, 8, 8, 8, 8, 8, 8, 8, 8, 1, 1, 8, 8, 8, 8, 8, 8, 8, 8, 3],
        [ 3, 8,14,14,14, 8, 8, 8, 8, 1, 1, 8, 8, 8, 8,14,14,14, 8, 3],
        [ 3, 8,14,28,14, 8, 8, 8, 8, 1, 1, 8, 8, 8, 8,14,27,14, 8, 3],
        [ 3, 8,14,14,14, 8, 8, 8, 8, 1, 1, 8, 8, 8, 8,14,14,14, 8, 3],
        [ 3, 8, 8, 8, 8, 8, 8, 8, 8, 1, 1, 8, 8, 8, 8, 8, 8, 8, 8, 3],
        [ 3,11,11,11,11,11, 8, 8, 8, 1, 1, 8, 8, 8,11,11,11,11,11, 3],
        [ 3, 8, 8, 8, 8, 8, 8, 8, 8, 1, 1, 8, 8, 8, 8, 8, 8, 8, 8, 3],
        [ 3, 8, 8, 8, 8, 8, 8, 8, 8, 1, 1, 8, 8, 8, 8, 8, 8, 8, 8, 3],
        [ 3, 8,14,14,14, 8, 8, 8, 8, 1, 1, 8, 8, 8, 8,14,14,14, 8, 3],
        [ 3, 8,14,24,14, 8, 8, 8, 8, 1, 1, 8, 8, 8, 8,14,25,14, 8, 3],
        [ 3, 8,14,14,14, 8, 8, 8,14,14,14,14, 8, 8, 8,14,14,14, 8, 3],
        [ 3, 8, 8, 8, 8, 8, 8, 8,14,29,29,14, 8, 8, 8, 8, 8, 8, 8, 3],
        [ 3, 8, 8, 8, 8, 8, 8, 8,14,14,14,14, 8, 8, 8, 8, 8, 8, 8, 3],
        [ 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 6, 3, 3, 3, 3, 3, 3, 3, 3, 3],
    ];

    // Room 6: Eren's Room — sand, wood desk, garden
    const ROOM_6: [[u16; ROOM_W]; ROOM_H] = [
        [ 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 6, 3, 3, 3, 3, 3, 3, 3, 3, 3],
        [ 3,15,15,15,15,15,15,15,15,15,15,15,15,15,15,15,15,15,15, 3],
        [ 3,15, 5, 5, 5, 5,15,15,15,15,15,15,15,15, 1, 1, 1, 1,15, 3],
        [ 3,15, 5,23,31, 5,15,15,15,15,15,15,15,15, 1, 8, 4, 1,15, 3],
        [ 3,15, 5, 5, 5, 5,15,15,15,15,15,15,15,15, 1, 1, 1, 1,15, 3],
        [ 3,15,15,15,15,15,15,15,15,15,15,15,15,15,15,15,15,15,15, 3],
        [ 3,15,15,15,15,15,15,15,15,15,15,15,15,15,15,15,15,15,15, 3],
        [ 3,15,15,15,15,15,15, 9, 9, 9, 9, 9, 9,15,15,15,15,15,15, 3],
        [ 3,15,15,15,15,15,15, 9,15,15,15,15, 9,15,15,15,15,15,15, 3],
        [ 3,15,15,15,15,15,15, 9, 9, 9, 9, 9, 9,15,15,15,15,15,15, 3],
        [ 3,15, 7, 7, 7,15,15,15,15,15,15,15,15,15,15, 5, 5, 5,15, 3],
        [ 3,15, 7,23, 7,15,15,15,15,15,15,15,15,15,15, 5,29, 5,15, 3],
        [ 3,15, 7, 7, 7,15,15,15,15,15,15,15,15,15,15, 5, 5, 5,15, 3],
        [ 3,15,15,15,15,15,15,15,15,15,15,15,15,15,15,15,15,15,15, 3],
        [ 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3],
    ];

    // Room 7: The Void — lava border, red diamond, void
    const ROOM_7: [[u16; ROOM_W]; ROOM_H] = [
        [ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        [ 0,18,18,18,18,18,18,18,18,18,18,18,18,18,18,18,18,18,18, 0],
        [ 0,18, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,18, 0],
        [ 0,18, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,18, 0],
        [ 0,18, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,18, 0],
        [ 0,18, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,18, 0],
        [ 0,18, 0, 0, 0, 0, 0, 0, 0,24,24, 0, 0, 0, 0, 0, 0, 0,18, 0],
        [ 0,18, 0, 0, 0, 0, 0, 0,24,24,24,24, 0, 0, 0, 0, 0, 0,18, 0],
        [ 0,18, 0, 0, 0, 0, 0, 0, 0,24,24, 0, 0, 0, 0, 0, 0, 0,18, 0],
        [ 0,18, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,18, 0],
        [ 0,18, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,18, 0],
        [ 0,18, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,18, 0],
        [ 0,18, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,18, 0],
        [ 0,18,18,18,18,18,18,18,18,18,18,18,18,18,18,18,18,18,18, 0],
        [ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ];

    // ===================================================================
    // Room metadata
    // ===================================================================

    const ROOM_NAMES: [&str; NUM_ROOMS] = [
        "", "The Nexus", "The Forge", "The Hive",
        "The Lens", "The Lab", "Eren's Room", "The Void",
    ];

    // Door connections: (from_room, door_col, door_row) → (to_room, spawn_col, spawn_row)
    const DOORS: &[(u8, u8, u8, u8, u8, u8)] = &[
        // Nexus doors
        (1, 10, 0,  2, 10, 13), // Nexus N → Forge
        (1,  0, 7,  3, 18,  7), // Nexus W → Hive
        (1, 19, 7,  4,  1,  7), // Nexus E → Lens
        (1, 10, 14, 5, 10,  1), // Nexus S → Lab
        // Forge back
        (2, 10, 14, 1, 10,  1), // Forge S → Nexus
        // Hive back
        (3, 19, 7,  1,  1,  7), // Hive E → Nexus
        // Lens back
        (4,  0, 7,  1, 18,  7), // Lens W → Nexus
        // Lab doors
        (5, 10, 0,  1, 10, 13), // Lab N → Nexus
        (5, 10, 14, 6, 10,  1), // Lab S → Eren's Room
        // Eren's Room
        (6, 10, 0,  5, 10, 13), // Eren N → Lab
        (6, 10, 14, 7, 10,  1), // Eren S → Void (hidden)
        // Void
        (7, 10, 0,  6, 10, 13), // Void N → Eren's Room
    ];

    // ===================================================================
    // Interactable data: (col, row, dialogue_id) per room
    // ===================================================================

    const INTERACTABLES: &[&[(u8, u8, u8)]] = &[
        &[],  // Room 0: Welcome (no interactables)
        &[(9, 7, 0), (10, 7, 0)],  // Room 1: Nexus — Web Center
        &[(3, 3, 1), (16, 3, 2), (9, 12, 3), (10, 12, 3)],  // Room 2: Forge
        &[(5, 6, 4), (14, 6, 5), (3, 12, 6), (16, 12, 7)],  // Room 3: Hive
        &[(3, 3, 8), (16, 3, 9), (3, 12, 10), (16, 12, 11)],  // Room 4: Lens
        &[(3, 3, 12), (16, 3, 13), (3, 10, 14), (16, 10, 15), (9, 12, 16), (10, 12, 16)],  // Room 5: Lab
        &[(3, 3, 17), (16, 3, 18), (3, 11, 19), (16, 11, 20), (9, 8, 99)],  // Room 6: Eren — 99 = rug
        &[(9, 7, 21), (10, 7, 21)],  // Room 7: Void — The Core
    ];

    // ===================================================================
    // Dialogue content: indexed by dialogue_id
    // Each entry is &[(speaker, text)]
    // ===================================================================

    const DLG_WEB_CENTER: &[(&str, &str)] = &[
        ("Web Center", "This game runs on Arachne."),
        ("Web Center", "A game engine. 389 kilobytes. Built from scratch in Rust."),
        ("Web Center", "Compiles to WebAssembly. Runs in your browser."),
        ("Web Center", "No Unity. No Godot. No Bevy. Just math and stubbornness."),
        ("Web Center", "Four doors. Four stories. Walk the web."),
    ];
    const DLG_ARACHNE: &[(&str, &str)] = &[
        ("Arachne Engine", "Everyone uses Unity. 300MB runtime for a 2D game."),
        ("Arachne Engine", "I thought: what if I just... didn't?"),
        ("Arachne Engine", "Sub-1MB. 15 crates. WebGPU rendering."),
        ("Arachne Engine", "The game you're playing right now? That's the proof."),
        ("Arachne Engine", "github.com/eren23/arachne"),
    ];
    const DLG_SYNAPSE: &[(&str, &str)] = &[
        ("Synapse", "ML models shouldn't need a data center."),
        ("Synapse", "Synapse: inference runtime in Rust and Zig."),
        ("Synapse", "18M-param JEPA world model. Client-side. In your browser."),
        ("Synapse", "Also a ViT encoder and a 2.4M-param diffusion model."),
        ("Synapse", "~60k lines of code. Explored ESP32-P4 deployment."),
        ("Synapse", "github.com/eren23/synapse"),
    ];
    const DLG_SQLD: &[(&str, &str)] = &[
        ("sqld", "My AI agent wrote a database engine."),
        ("sqld", "26,500 lines of Rust. PostgreSQL-compatible."),
        ("sqld", "I watched it happen. The AttoCode swarm, autonomously."),
        ("sqld", "Then a former employer claimed they owned it."),
        ("sqld", "They didn't. I proved it. In court."),
        ("sqld", "That was a fun Tuesday."),
    ];
    const DLG_ATTOCODE: &[(&str, &str)] = &[
        ("AttoCode", "Started as a 26-lesson TypeScript course."),
        ("AttoCode", "Teach yourself AI coding agents from scratch."),
        ("AttoCode", "Then it became a platform. FastAPI, React 19, PostgreSQL."),
        ("AttoCode", "Then I gave it a swarm system."),
        ("AttoCode", "Multiple agents. One goal. Build something."),
        ("AttoCode", "github.com/eren23/attocode"),
    ];
    const DLG_SPIDER_CHAT: &[(&str, &str)] = &[
        ("Spider Chat", "Conversations shouldn't be linear."),
        ("Spider Chat", "What if chat branched like git? Fork, explore, merge."),
        ("Spider Chat", "Built it. MCP integration. Conversation navigation."),
        ("Spider Chat", "Non-linear thinking for non-linear problems."),
    ];
    const DLG_VYX: &[(&str, &str)] = &[
        ("Vyx", "The swarm built a programming language."),
        ("Vyx", "In Go. From a spec. Autonomously."),
        ("Vyx", "I mostly just watched and debugged edge cases."),
        ("Vyx", "The future of programming is weird."),
    ];
    const DLG_SWARM: &[(&str, &str)] = &[
        ("Swarm Terminal", "38 MCP tools. Redis pub/sub. pgvector."),
        ("Swarm Terminal", "The swarm orchestrates multiple agents."),
        ("Swarm Terminal", "Give it a goal. It figures out the plan."),
        ("Swarm Terminal", "It built sqld. It built Vyx."),
        ("Swarm Terminal", "What do you want it to build?"),
    ];
    const DLG_VISIONBOT: &[(&str, &str)] = &[
        ("Visionbot", "Browsers are the universal interface."),
        ("Visionbot", "So I built agents that can see and use them."),
        ("Visionbot", "Containerized Debian/VNC per session."),
        ("Visionbot", "Hierarchical planner/executor with verification loops."),
        ("Visionbot", "Production-grade. Shipped to real users at Neuland AI."),
    ];
    const DLG_SAM_CLIP: &[(&str, &str)] = &[
        ("SAM-CLIP-Diffusion", "Point at something. Describe what you want."),
        ("SAM-CLIP-Diffusion", "SAM segments it. CLIP understands it. Diffusion transforms it."),
        ("SAM-CLIP-Diffusion", "Three models. One sentence. Text-based image editing."),
        ("SAM-CLIP-Diffusion", "github.com/eren23/sam-clip-diffusion"),
    ];
    const DLG_GEO_SPY: &[(&str, &str)] = &[
        ("open_geo_spy", "Show me a photo. I'll tell you where it was taken."),
        ("open_geo_spy", "Open-source geolocation AI."),
        ("open_geo_spy", "Like GeoGuessr, but the AI plays."),
    ];
    const DLG_NEO_UNIFY: &[(&str, &str)] = &[
        ("neo-unify", "Encoder-free multimodal model."),
        ("neo-unify", "Mixture-of-Transformers on Apple MLX."),
        ("neo-unify", "Toy scale, but the architecture is real."),
        ("neo-unify", "Sometimes you build things to understand things."),
    ];
    const DLG_CRUCIBLE: &[(&str, &str)] = &[
        ("Crucible", "OpenAI ran a competition. Parameter Golf."),
        ("Crucible", "I built a system that designs its own ML experiments."),
        ("Crucible", "LLMs propose hypotheses. GPUs run them. Repeat."),
        ("Crucible", "Modality-agnostic. DDPM, JEPA, whatever fits."),
        ("Crucible", "865 tests. Rental GPUs. Autonomous science."),
    ];
    const DLG_ANTELLIGENCE: &[(&str, &str)] = &[
        ("Antelligence", "I simulated ant colonies."),
        ("Antelligence", "Simple rules. No central control."),
        ("Antelligence", "Intelligence emerged anyway."),
        ("Antelligence", "Then I got distracted by something shinier."),
        ("Antelligence", "That's kind of the theme here."),
    ];
    const DLG_MISC_1: &[(&str, &str)] = &[
        ("Projects", "AI Tamagotchi. Auto cover letters. Face recognition API."),
        ("Projects", "Morse code translator via blink detection."),
        ("Projects", "OCR tools. KnowledgeGPT. Chrome extensions."),
        ("Projects", "I build things the way other people doomscroll."),
    ];
    const DLG_MISC_2: &[(&str, &str)] = &[
        ("Projects", "one_layer_image_gen: the FAE paper in PyTorch."),
        ("Projects", "39 Python repos. 5 Rust. Growing."),
        ("Projects", "JavaScript, TypeScript, Go. Whatever fits the problem."),
        ("Projects", "The language doesn't matter. The thing you build does."),
    ];
    const DLG_ARCHIVE: &[(&str, &str)] = &[
        ("Archive", "This room is where experiments pile up."),
        ("Archive", "Not everything ships. Not everything should."),
        ("Archive", "But every project here taught me something"),
        ("Archive", "that made the next one better."),
    ];
    const DLG_DESK: &[(&str, &str)] = &[
        ("Eren's Desk", "M.Sc. Intelligent Systems."),
        ("Eren's Desk", "Lead AI Engineer. Based in Berlin."),
        ("Eren's Desk", "Mass producer of open source."),
        ("Eren's Desk", "Human, talking to machines."),
    ];
    const DLG_GARDEN: &[(&str, &str)] = &[
        ("The Garden", "Berlin. Good coffee, fast internet, no small talk."),
        ("The Garden", "Perfect for building things."),
        ("The Garden", "Also: spiders and octopuses are cool."),
        ("The Garden", "That's not a metaphor. I just think they're neat."),
    ];
    const DLG_BOOKSHELF: &[(&str, &str)] = &[
        ("Bookshelf", "blog.akbuluteren.com"),
        ("Bookshelf", "Where I write about the things I break"),
        ("Bookshelf", "and occasionally fix."),
    ];
    const DLG_LINKS: &[(&str, &str)] = &[
        ("Terminal", "github.com/eren23"),
        ("Terminal", "That's where the code lives."),
        ("Terminal", "All of it. Open source."),
        ("Terminal", "If you're reading this, you're already here."),
    ];
    const DLG_VOID: &[(&str, &str)] = &[
        ("The Core", "This game is 389 kilobytes."),
        ("The Core", "The engine running it was built from scratch. In Rust."),
        ("The Core", "2,100 lines of spec. 15 crates. WebGPU."),
        ("The Core", "You just played a portfolio"),
        ("The Core", "built on the portfolio piece."),
        ("The Core", "That's the kind of thing I build."),
        ("The Core", "Want to build something together?"),
        ("The Core", "github.com/eren23 | blog.akbuluteren.com"),
    ];

    fn get_dialogue(id: u8) -> &'static [(&'static str, &'static str)] {
        match id {
            0 => DLG_WEB_CENTER, 1 => DLG_ARACHNE, 2 => DLG_SYNAPSE, 3 => DLG_SQLD,
            4 => DLG_ATTOCODE, 5 => DLG_SPIDER_CHAT, 6 => DLG_VYX, 7 => DLG_SWARM,
            8 => DLG_VISIONBOT, 9 => DLG_SAM_CLIP, 10 => DLG_GEO_SPY, 11 => DLG_NEO_UNIFY,
            12 => DLG_CRUCIBLE, 13 => DLG_ANTELLIGENCE, 14 => DLG_MISC_1, 15 => DLG_MISC_2,
            16 => DLG_ARCHIVE, 17 => DLG_DESK, 18 => DLG_GARDEN, 19 => DLG_BOOKSHELF,
            20 => DLG_LINKS, 21 => DLG_VOID,
            _ => &[],
        }
    }

    // Spider companion entry dialogue per room (auto on first visit)
    const SPIDER_ENTRY: [&[&str]; NUM_ROOMS] = [
        &[],  // 0: Welcome
        &["You found the web. Good.",
          "Most people just scroll past. You walked in.",
          "Let me show you what Eren built."],
        &["Careful here. This is where Eren decided",
          "colored rectangles weren't enough."],
        &["The agents in here built a database.",
          "Autonomously. 26,500 lines.",
          "Eren just watched. Terrifying, honestly."],
        &["Eyes everywhere in here.",
          "Don't worry, they're friendly."],
        &["This is where ideas go to be tested.",
          "Some survive."],
        &["He doesn't sleep enough.",
          "Don't tell him I said that."],
        &["You made it.",
          "Most don't look this deep."],
    ];

    // Silk thread positions: (room, col, row)
    const SILK_POSITIONS: [(u8, u8, u8); NUM_SILK] = [
        (1,  2,  2),  // Nexus corner
        (2, 17,  9),  // Forge south of lava
        (2,  2, 12),  // Forge near sqld
        (3, 10,  1),  // Hive near server racks
        (4, 17, 10),  // Lens past ice
        (5,  1, 13),  // Lab bottom-left
        (5, 18,  7),  // Lab east side
        (6, 18, 13),  // Eren's Room bottom-right
    ];

    // ===================================================================
    // Game state
    // ===================================================================

    #[derive(Clone, Copy, PartialEq)]
    enum GamePhase {
        Welcome,
        Playing,
    }

    struct GameState {
        phase: GamePhase,
        current_room: usize,
        transition_cooldown: f32,
        // Visited rooms tracking
        visited: [bool; NUM_ROOMS],
        // Dialogue state
        active_dialogue: bool,
        dialogue_id: u8,
        dialogue_line: usize,
        dialogue_speaker: &'static str,
        dialogue_text: &'static str,
        // Spider entry dialogue
        spider_speaking: bool,
        spider_line: usize,
        // Interaction proximity
        near_interactable: bool,
        near_interact_id: u8,
        // Silk threads
        silk_collected: u8,
        silk_alive: [bool; NUM_SILK],
        // Hidden door (Room 6)
        hidden_door_open: bool,
        rug_interactions: u8,
        // Void transformation
        void_transformed: bool,
        // Welcome typewriter
        welcome_char_idx: usize,
        welcome_line: u8,
        welcome_timer: f32,
        // Elapsed time
        elapsed: f32,
    }
    unsafe impl Send for GameState {}
    unsafe impl Sync for GameState {}

    impl GameState {
        fn new() -> Self {
            Self {
                phase: GamePhase::Welcome,
                current_room: 0,
                transition_cooldown: 0.0,
                visited: [false; NUM_ROOMS],
                active_dialogue: false,
                dialogue_id: 0,
                dialogue_line: 0,
                dialogue_speaker: "",
                dialogue_text: "",
                spider_speaking: false,
                spider_line: 0,
                near_interactable: false,
                near_interact_id: 0,
                silk_collected: 0,
                silk_alive: [true; NUM_SILK],
                hidden_door_open: false,
                rug_interactions: 0,
                void_transformed: false,
                welcome_char_idx: 0,
                welcome_line: 0,
                welcome_timer: 0.0,
                elapsed: 0.0,
            }
        }
    }

    // ===================================================================
    // Component markers
    // ===================================================================

    #[derive(Clone, Copy)] struct Player;
    #[derive(Clone, Copy)] struct SpiderCompanion;
    #[derive(Clone, Copy)] struct SilkThread(u8); // index into SILK_POSITIONS
    #[derive(Clone, Copy)] struct DriftingBot(f32, f32, f32, f32); // base_x, base_y, freq, phase

    // ===================================================================
    // Helpers
    // ===================================================================

    fn get_room_data(room: usize) -> &'static [[u16; ROOM_W]; ROOM_H] {
        match room {
            0 => &ROOM_0, 1 => &ROOM_1, 2 => &ROOM_2, 3 => &ROOM_3,
            4 => &ROOM_4, 5 => &ROOM_5, 6 => &ROOM_6, 7 => &ROOM_7,
            _ => &ROOM_1,
        }
    }

    fn get_tile(state: &GameState, room: usize, col: usize, row: usize) -> u16 {
        // Hidden door override
        if room == 6 && col == 10 && row == 14 && state.hidden_door_open {
            return DOOR;
        }
        get_room_data(room)[row][col]
    }

    fn is_solid(tile: u16) -> bool {
        matches!(tile,
            STONE_WALL | BRICK_WALL | DARK_STONE | DEEP_WATER |
            METAL_DOOR | MOSSY_BRICK | LAVA | DARK_BRICK
        )
    }

    fn pixel_to_tile(px: f32, py: f32) -> (usize, usize) {
        let tx = (px / TILE_SIZE).floor() as isize;
        let ty = (py / TILE_SIZE).floor() as isize;
        (
            tx.clamp(0, ROOM_W as isize - 1) as usize,
            ty.clamp(0, ROOM_H as isize - 1) as usize,
        )
    }

    fn would_collide(state: &GameState, px: f32, py: f32) -> bool {
        let half = 10.0;
        let corners = [
            (px - half, py - half), (px + half, py - half),
            (px - half, py + half), (px + half, py + half),
        ];
        for (cx, cy) in corners {
            let (tx, ty) = pixel_to_tile(cx, cy);
            if tx < ROOM_W && ty < ROOM_H {
                let tile = get_tile(state, state.current_room, tx, ty);
                if is_solid(tile) { return true; }
            }
        }
        false
    }

    fn build_tilemap_layer(state: &GameState, room: usize) -> TilemapLayer {
        let mut layer = TilemapLayer::new(
            ROOM_W as u32, ROOM_H as u32,
            Vec2::new(TILE_SIZE, TILE_SIZE), 8, 8,
        );
        for y in 0..ROOM_H {
            for x in 0..ROOM_W {
                let idx = get_tile(state, room, x, y);
                if idx != EMPTY {
                    layer.set_tile(x as u32, y as u32, Some(Tile::new(idx)));
                }
            }
        }
        layer
    }

    fn tile_center(col: usize, row: usize) -> (f32, f32) {
        (col as f32 * TILE_SIZE + TILE_SIZE * 0.5,
         row as f32 * TILE_SIZE + TILE_SIZE * 0.5)
    }

    // ===================================================================
    // Systems
    // ===================================================================

    fn setup(mut commands: Commands) {
        // Camera at center of room
        let (cx, cy) = tile_center(10, 7);
        commands.spawn((Camera::new(), Transform::from_position(Vec3::new(cx, cy, 0.0))));

        // Player sprite (green square — will be replaced by pixel art later)
        let mut player_spr = Sprite::new(TextureHandle(0));
        player_spr.color = Color::rgb(0.2, 0.85, 0.3);
        player_spr.custom_size = Some(Vec2::new(24.0, 24.0));
        commands.spawn((
            Player,
            player_spr,
            Transform::from_position(Vec3::new(cx, cy, 0.2)),
        ));

        // Spider companion (small red square — will be replaced by pixel art)
        let mut spider_spr = Sprite::new(TextureHandle(0));
        spider_spr.color = Color::rgb(0.85, 0.15, 0.15);
        spider_spr.custom_size = Some(Vec2::new(12.0, 12.0));
        commands.spawn((
            SpiderCompanion,
            spider_spr,
            Transform::from_position(Vec3::new(cx - 32.0, cy, 0.15)),
        ));

        commands.insert_resource(GameState::new());
    }

    fn setup_tilemap(state: Res<GameState>, mut tilemap: ResMut<TilemapRendererResource>) {
        let layer = build_tilemap_layer(&state, 0);
        tilemap.layers = vec![layer];
    }

    fn spawn_silk_threads(state: Res<GameState>, mut commands: Commands) {
        // Spawn silk threads for current room
        for (i, &(room, col, row)) in SILK_POSITIONS.iter().enumerate() {
            if room as usize == state.current_room && state.silk_alive[i] {
                let (sx, sy) = tile_center(col as usize, row as usize);
                let mut spr = Sprite::new(TextureHandle(0));
                spr.color = Color::rgb(1.0, 0.84, 0.0); // Gold
                spr.custom_size = Some(Vec2::new(8.0, 8.0));
                commands.spawn((
                    SilkThread(i as u8),
                    spr,
                    Transform::from_position(Vec3::new(sx, sy, 0.12)),
                ));
            }
        }
    }

    // --- Welcome screen system ---
    fn welcome_screen(
        time: Res<Time>,
        input: Res<InputSystem>,
        mut state: ResMut<GameState>,
        mut tilemap: ResMut<TilemapRendererResource>,
        mut player_q: Query<(&Player, &mut Transform)>,
    ) {
        if state.phase != GamePhase::Welcome { return; }

        state.welcome_timer += time.delta_seconds();

        // Check for any key press to skip
        let any_key = input.keyboard.just_pressed(KeyCode::Space)
            || input.keyboard.just_pressed(KeyCode::Enter)
            || input.keyboard.just_pressed(KeyCode::E)
            || input.keyboard.just_pressed(KeyCode::W)
            || input.keyboard.just_pressed(KeyCode::A)
            || input.keyboard.just_pressed(KeyCode::S)
            || input.keyboard.just_pressed(KeyCode::D)
            || input.mouse.just_pressed(arachne_input::MouseButton::Left);

        if any_key && state.welcome_timer > 1.0 {
            // Transition to Nexus
            state.phase = GamePhase::Playing;
            state.current_room = 1;
            state.visited[1] = true;
            state.spider_speaking = true;
            state.spider_line = 0;

            let layer = build_tilemap_layer(&state, 1);
            if tilemap.layers.is_empty() { tilemap.layers.push(layer); }
            else { tilemap.layers[0] = layer; }

            let (sx, sy) = tile_center(10, 7);
            for (_p, t) in player_q.iter_mut() {
                t.position.x = sx;
                t.position.y = sy;
            }
        }
    }

    // --- Player movement ---
    fn player_movement(
        input: Res<InputSystem>,
        time: Res<Time>,
        state: Res<GameState>,
        mut query: Query<(&Player, &mut Transform)>,
    ) {
        if state.phase != GamePhase::Playing { return; }
        if state.active_dialogue || state.spider_speaking { return; }

        let dt = time.delta_seconds();
        let mut speed = PLAYER_SPEED;

        for (_player, transform) in query.iter_mut() {
            // Ice sliding: check current tile
            let (tx, ty) = pixel_to_tile(transform.position.x, transform.position.y);
            let current_tile = get_tile(&state, state.current_room, tx, ty);
            if current_tile == ICE {
                speed = PLAYER_SPEED * 1.8; // faster on ice
            }

            let mut dx = 0.0f32;
            let mut dy = 0.0f32;

            if input.keyboard.pressed(KeyCode::Right) || input.keyboard.pressed(KeyCode::D) { dx += 1.0; }
            if input.keyboard.pressed(KeyCode::Left) || input.keyboard.pressed(KeyCode::A) { dx -= 1.0; }
            if input.keyboard.pressed(KeyCode::Up) || input.keyboard.pressed(KeyCode::W) { dy -= 1.0; }
            if input.keyboard.pressed(KeyCode::Down) || input.keyboard.pressed(KeyCode::S) { dy += 1.0; }

            // Touch input: use first touch position relative to player as direction
            if dx == 0.0 && dy == 0.0 && input.touch.any_touch_active() {
                for touch in input.touch.active_touches() {
                    let tdx = touch.position.x - 320.0; // relative to screen center
                    let tdy = touch.position.y - 240.0;
                    if tdx.abs() > 20.0 || tdy.abs() > 20.0 {
                        dx = tdx;
                        dy = tdy;
                    }
                    break;
                }
            }

            let len_sq = dx * dx + dy * dy;
            if len_sq > 0.0 {
                let inv_len = 1.0 / len_sq.sqrt();
                dx *= inv_len * speed * dt;
                dy *= inv_len * speed * dt;
            }

            let new_x = transform.position.x + dx;
            if !would_collide(&state, new_x, transform.position.y) {
                transform.position.x = new_x;
            }
            let new_y = transform.position.y + dy;
            if !would_collide(&state, transform.position.x, new_y) {
                transform.position.y = new_y;
            }
        }
    }

    // --- Camera follow ---
    fn camera_follow(
        player_q: Query<(&Player, &Transform)>,
        mut cam_q: Query<(&Camera, &mut Transform)>,
    ) {
        let mut pp = Vec3::ZERO;
        for (_, t) in player_q.iter() { pp = t.position; }
        for (_, ct) in cam_q.iter_mut() {
            ct.position.x += (pp.x - ct.position.x) * CAMERA_LERP;
            ct.position.y += (pp.y - ct.position.y) * CAMERA_LERP;
        }
    }

    // --- Room transition ---
    fn room_transition(
        time: Res<Time>,
        mut state: ResMut<GameState>,
        mut tilemap: ResMut<TilemapRendererResource>,
        mut player_q: Query<(&Player, &mut Transform)>,
        silk_q: Query<(Entity, &SilkThread)>,
        bot_q: Query<(Entity, &DriftingBot)>,
        mut commands: Commands,
    ) {
        if state.phase != GamePhase::Playing { return; }
        if state.transition_cooldown > 0.0 {
            state.transition_cooldown -= time.delta_seconds();
            return;
        }

        for (_, transform) in player_q.iter_mut() {
            let (tx, ty) = pixel_to_tile(transform.position.x, transform.position.y);
            let tile = get_tile(&state, state.current_room, tx, ty);

            if tile == DOOR {
                // Find matching door connection
                let mut found = false;
                for &(from, dc, dr, to, sc, sr) in DOORS {
                    if from as usize == state.current_room
                        && dc as usize == tx && dr as usize == ty
                    {
                        // Check hidden door restriction
                        if state.current_room == 6 && ty == 14 && !state.hidden_door_open {
                            continue;
                        }

                        let new_room = to as usize;
                        state.current_room = new_room;
                        state.transition_cooldown = TRANSITION_COOLDOWN;
                        state.active_dialogue = false;
                        state.near_interactable = false;

                        // Trigger spider entry if first visit
                        if !state.visited[new_room] {
                            state.visited[new_room] = true;
                            if !SPIDER_ENTRY[new_room].is_empty() {
                                state.spider_speaking = true;
                                state.spider_line = 0;
                            }
                        }

                        let (sx, sy) = tile_center(sc as usize, sr as usize);
                        transform.position.x = sx;
                        transform.position.y = sy;

                        let layer = build_tilemap_layer(&state, new_room);
                        if tilemap.layers.is_empty() { tilemap.layers.push(layer); }
                        else { tilemap.layers[0] = layer; }

                        // Despawn old silk threads and bots
                        for (e, _) in silk_q.iter() { commands.despawn(e); }
                        for (e, _) in bot_q.iter() { commands.despawn(e); }

                        // Spawn silk threads for new room
                        for (i, &(room, col, row)) in SILK_POSITIONS.iter().enumerate() {
                            if room as usize == new_room && state.silk_alive[i] {
                                let (sx2, sy2) = tile_center(col as usize, row as usize);
                                let mut spr = Sprite::new(TextureHandle(0));
                                spr.color = Color::rgb(1.0, 0.84, 0.0);
                                spr.custom_size = Some(Vec2::new(8.0, 8.0));
                                commands.spawn((
                                    SilkThread(i as u8),
                                    spr,
                                    Transform::from_position(Vec3::new(sx2, sy2, 0.12)),
                                ));
                            }
                        }

                        // Spawn drifting bots for Hive (room 3)
                        if new_room == 3 {
                            let bot_positions: [(f32, f32, f32, f32); 4] = [
                                (8.0, 5.0, 1.2, 0.0),
                                (12.0, 9.0, 0.8, 1.5),
                                (15.0, 4.0, 1.0, 3.0),
                                (6.0, 10.0, 1.4, 4.5),
                            ];
                            for (bx, by, freq, phase) in bot_positions {
                                let (px, py) = tile_center(bx as usize, by as usize);
                                let mut spr = Sprite::new(TextureHandle(0));
                                spr.color = Color::rgb(0.0, 0.9, 1.0);
                                spr.custom_size = Some(Vec2::new(8.0, 8.0));
                                commands.spawn((
                                    DriftingBot(px, py, freq, phase),
                                    spr,
                                    Transform::from_position(Vec3::new(px, py, 0.11)),
                                ));
                            }
                        }

                        found = true;
                        break;
                    }
                }
                if !found { continue; }
            }
        }
    }

    // --- Interaction system ---
    fn interaction(
        input: Res<InputSystem>,
        mut state: ResMut<GameState>,
        player_q: Query<(&Player, &Transform)>,
    ) {
        if state.phase != GamePhase::Playing { return; }

        // Handle spider entry dialogue (E or tap to advance)
        if state.spider_speaking {
            let pressed = input.keyboard.just_pressed(KeyCode::E)
                || input.keyboard.just_pressed(KeyCode::Space)
                || input.mouse.just_pressed(arachne_input::MouseButton::Left);
            if pressed {
                state.spider_line += 1;
                let lines = SPIDER_ENTRY[state.current_room];
                if state.spider_line >= lines.len() {
                    state.spider_speaking = false;
                }
            }
            return;
        }

        // Handle active dialogue (E to advance)
        if state.active_dialogue {
            let pressed = input.keyboard.just_pressed(KeyCode::E)
                || input.keyboard.just_pressed(KeyCode::Space)
                || input.mouse.just_pressed(arachne_input::MouseButton::Left);
            if pressed {
                state.dialogue_line += 1;
                let dlg = get_dialogue(state.dialogue_id);
                if state.dialogue_line >= dlg.len() {
                    // Dialogue finished
                    state.active_dialogue = false;

                    // Void transformation check
                    if state.dialogue_id == 21 {
                        state.void_transformed = true;
                    }
                } else {
                    let (speaker, text) = dlg[state.dialogue_line];
                    state.dialogue_speaker = speaker;
                    state.dialogue_text = text;
                }
            }
            return;
        }

        // Check proximity to interactables
        let mut near = false;
        let mut near_id = 0u8;

        for (_, transform) in player_q.iter() {
            let (tx, ty) = pixel_to_tile(transform.position.x, transform.position.y);
            let room_interactables = INTERACTABLES[state.current_room];

            for &(ic, ir, did) in room_interactables {
                let dx = (tx as i32 - ic as i32).abs();
                let dy = (ty as i32 - ir as i32).abs();
                if dx <= 1 && dy <= 1 {
                    near = true;
                    near_id = did;
                    break;
                }
            }
        }

        state.near_interactable = near;
        state.near_interact_id = near_id;

        if near && (input.keyboard.just_pressed(KeyCode::E)
            || input.mouse.just_pressed(arachne_input::MouseButton::Left))
        {
            // Special: rug easter egg (dialogue_id 99)
            if near_id == 99 {
                state.rug_interactions += 1;
                match state.rug_interactions {
                    1 => {
                        state.active_dialogue = true;
                        state.dialogue_id = 99;
                        state.dialogue_line = 0;
                        state.dialogue_speaker = "Rug";
                        state.dialogue_text = "Nice rug.";
                    }
                    2 => {
                        state.active_dialogue = true;
                        state.dialogue_id = 99;
                        state.dialogue_line = 0;
                        state.dialogue_speaker = "Rug";
                        state.dialogue_text = "...really nice rug.";
                    }
                    _ => {
                        state.active_dialogue = true;
                        state.dialogue_id = 99;
                        state.dialogue_line = 0;
                        state.dialogue_speaker = "Rug";
                        state.dialogue_text = "Oh. You found it.";
                        state.hidden_door_open = true;
                    }
                }
                return;
            }

            // Normal dialogue
            let dlg = get_dialogue(near_id);
            if !dlg.is_empty() {
                state.active_dialogue = true;
                state.dialogue_id = near_id;
                state.dialogue_line = 0;
                let (speaker, text) = dlg[0];
                state.dialogue_speaker = speaker;
                state.dialogue_text = text;
            }
        }
    }

    // --- Silk thread collection ---
    fn collect_silk(
        mut state: ResMut<GameState>,
        player_q: Query<(&Player, &Transform)>,
        silk_q: Query<(Entity, &SilkThread, &Transform)>,
        mut commands: Commands,
    ) {
        if state.phase != GamePhase::Playing { return; }

        for (_, pt) in player_q.iter() {
            for (entity, silk, st) in silk_q.iter() {
                let dx = pt.position.x - st.position.x;
                let dy = pt.position.y - st.position.y;
                if dx * dx + dy * dy < 16.0 * 16.0 {
                    state.silk_collected += 1;
                    state.silk_alive[silk.0 as usize] = false;
                    commands.despawn(entity);

                    // Check if all collected → open hidden door
                    if state.silk_collected >= NUM_SILK as u8 {
                        state.hidden_door_open = true;
                    }
                }
            }
        }
    }

    // --- Spider companion follow ---
    fn spider_follow(
        state: Res<GameState>,
        time: Res<Time>,
        player_q: Query<(&Player, &Transform)>,
        mut spider_q: Query<(&SpiderCompanion, &mut Transform)>,
    ) {
        if state.phase != GamePhase::Playing { return; }

        let mut pp = Vec3::ZERO;
        for (_, t) in player_q.iter() { pp = t.position; }

        for (_, st) in spider_q.iter_mut() {
            // Follow with delay
            st.position.x += (pp.x - 48.0 - st.position.x) * 0.03;
            st.position.y += (pp.y - st.position.y) * 0.03;
            // Subtle bob
            let elapsed = time.elapsed_seconds();
            st.position.y += (elapsed * 3.0).sin() * 0.3;
        }
    }

    // --- Drifting bots (Hive room) ---
    fn drift_bots(
        state: Res<GameState>,
        time: Res<Time>,
        mut q: Query<(&DriftingBot, &mut Transform)>,
    ) {
        if state.current_room != 3 { return; }
        let t = time.elapsed_seconds();
        for (bot, tf) in q.iter_mut() {
            tf.position.x = bot.0 + (t * bot.2 + bot.3).sin() * 40.0;
            tf.position.y = bot.1 + (t * bot.2 * 0.7 + bot.3 + 1.0).cos() * 30.0;
        }
    }

    // --- Lava pulse (Forge room) ---
    fn lava_pulse(
        state: Res<GameState>,
        time: Res<Time>,
        mut tilemap: ResMut<TilemapRendererResource>,
    ) {
        if state.current_room != 2 || state.phase != GamePhase::Playing { return; }
        if tilemap.layers.is_empty() { return; }

        let t = time.elapsed_seconds();
        let pulse = (t * 2.0).sin() > 0.0;
        let lava_tile = if pulse { LAVA } else { YELLOW };

        let data = get_room_data(2);
        for y in 0..ROOM_H {
            for x in 0..ROOM_W {
                if data[y][x] == LAVA {
                    tilemap.layers[0].set_tile(
                        x as u32, y as u32,
                        Some(Tile::new(lava_tile)),
                    );
                }
            }
        }
    }

    // --- Void transformation ---
    fn void_transform(
        state: Res<GameState>,
        mut tilemap: ResMut<TilemapRendererResource>,
    ) {
        if state.current_room != 7 || !state.void_transformed { return; }
        if tilemap.layers.is_empty() { return; }

        // Fill void with web pattern (same as welcome screen)
        let web = &ROOM_0;
        for y in 0..ROOM_H {
            for x in 0..ROOM_W {
                if web[y][x] == DARK_GRAY && ROOM_7[y][x] == EMPTY {
                    tilemap.layers[0].set_tile(
                        x as u32, y as u32,
                        Some(Tile::new(DARK_GRAY)),
                    );
                }
            }
        }
    }

    // --- HUD system ---
    fn hud(
        state: Res<GameState>,
        time: Res<Time>,
        mut tb: ResMut<ScreenTextBuffer>,
    ) {
        // Welcome screen
        if state.phase == GamePhase::Welcome {
            let lines = ["389 kilobytes.", "That's all this needs.", "Press any key to enter the web."];
            let elapsed = state.welcome_timer;

            let mut y = 200.0;
            for (i, line) in lines.iter().enumerate() {
                let line_start = i as f32 * 2.0; // 2s per line
                if elapsed > line_start {
                    let chars_elapsed = ((elapsed - line_start) / 0.04) as usize;
                    let visible: String = line.chars().take(chars_elapsed).collect();
                    if !visible.is_empty() {
                        tb.draw(
                            visible,
                            Vec2::new(180.0, y), 22.0,
                            Color::rgb(0.9, 0.3, 0.3),
                        );
                    }
                }
                y += 40.0;
            }

            tb.draw(
                "Built on Arachne",
                Vec2::new(240.0, 440.0), 12.0,
                Color::rgb(0.3, 0.3, 0.35),
            );
            return;
        }

        // Room name
        let name = ROOM_NAMES[state.current_room];
        if !name.is_empty() {
            tb.draw(
                name.to_string(),
                Vec2::new(260.0, 12.0), 22.0,
                Color::WHITE,
            );
        }

        // Silk counter
        tb.draw(
            format!("Silk: {}/{}", state.silk_collected, NUM_SILK),
            Vec2::new(520.0, 12.0), 14.0,
            Color::rgb(1.0, 0.84, 0.0),
        );

        // Built on Arachne footer
        tb.draw(
            "Built on Arachne",
            Vec2::new(500.0, 460.0), 10.0,
            Color::rgb(0.3, 0.3, 0.35),
        );

        // Spider entry dialogue
        if state.spider_speaking {
            let lines = SPIDER_ENTRY[state.current_room];
            if state.spider_line < lines.len() {
                // Dark background for dialogue
                tb.draw(
                    "Arachne",
                    Vec2::new(70.0, 370.0), 14.0,
                    Color::rgb(1.0, 0.85, 0.2),
                );
                tb.draw_wrapped(
                    lines[state.spider_line].to_string(),
                    Vec2::new(70.0, 390.0), 18.0,
                    Color::rgb(0.95, 0.95, 1.0),
                    500.0,
                );
                tb.draw(
                    "[E] Next",
                    Vec2::new(530.0, 440.0), 12.0,
                    Color::rgb(0.5, 0.5, 0.6),
                );
            }
            return;
        }

        // Active dialogue
        if state.active_dialogue {
            tb.draw(
                state.dialogue_speaker.to_string(),
                Vec2::new(70.0, 370.0), 14.0,
                Color::rgb(1.0, 0.85, 0.2),
            );
            tb.draw_wrapped(
                state.dialogue_text.to_string(),
                Vec2::new(70.0, 390.0), 18.0,
                Color::rgb(0.95, 0.95, 1.0),
                500.0,
            );
            tb.draw(
                "[E] Next",
                Vec2::new(530.0, 440.0), 12.0,
                Color::rgb(0.5, 0.5, 0.6),
            );
            return;
        }

        // Interaction hint
        if state.near_interactable {
            let blink = (time.elapsed_seconds() * 3.0).sin() > -0.3;
            if blink {
                tb.draw(
                    "[E] Interact",
                    Vec2::new(270.0, 420.0), 16.0,
                    Color::rgb(1.0, 1.0, 0.4),
                );
            }
        }

        // Controls hint
        tb.draw(
            "WASD: Move | E: Interact",
            Vec2::new(190.0, 460.0), 12.0,
            Color::rgb(0.4, 0.4, 0.45),
        );
    }

    // ===================================================================
    // Entry point
    // ===================================================================

    let app = Box::leak(Box::new(App::new()));
    app.add_plugin(DefaultPlugins);
    app.build_plugins();

    app.add_startup_system(setup);
    app.add_startup_system(setup_tilemap);
    app.add_startup_system(spawn_silk_threads);
    app.add_system(welcome_screen);
    app.add_system(player_movement);
    app.add_system(camera_follow);
    app.add_system(room_transition);
    app.add_system(interaction);
    app.add_system(collect_silk);
    app.add_system(spider_follow);
    app.add_system(drift_bots);
    app.add_system(lava_pulse);
    app.add_system(void_transform);
    app.add_system(hud);

    let mut runner = WasmRunner::with_canvas_id(canvas_id);
    runner.run(&mut app.world, &mut app.schedule);
}

#[cfg(not(all(target_arch = "wasm32", feature = "wasm")))]
pub fn run(_canvas_id: &str) {}
