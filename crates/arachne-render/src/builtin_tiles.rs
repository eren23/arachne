//! Built-in procedural tile atlas for tilemap rendering without external assets.
//!
//! Generates a 16x16 pixel tile atlas arranged as 8 columns x 4 rows (32 tiles).
//! The atlas is returned as RGBA8 pixel data ready for GPU upload.

/// Tile size in pixels.
pub const TILE_SIZE: u32 = 16;
/// Number of tile columns in the atlas.
pub const TILE_COLS: u32 = 8;
/// Number of tile rows in the atlas.
pub const TILE_ROWS: u32 = 4;
/// Atlas width in pixels.
pub const ATLAS_W: u32 = TILE_COLS * TILE_SIZE; // 128
/// Atlas height in pixels.
pub const ATLAS_H: u32 = TILE_ROWS * TILE_SIZE; // 64

/// Generate the built-in tile atlas as RGBA8 pixel data.
///
/// Returns `(rgba_data, atlas_width, atlas_height, tile_cols, tile_rows)`.
pub fn generate_builtin_tiles() -> (Vec<u8>, u32, u32, u32, u32) {
    let mut pixels = vec![0u8; (ATLAS_W * ATLAS_H * 4) as usize];

    for tile_index in 0u32..32 {
        let col = tile_index % TILE_COLS;
        let row = tile_index / TILE_COLS;
        let origin_x = col * TILE_SIZE;
        let origin_y = row * TILE_SIZE;

        for py in 0..TILE_SIZE {
            for px in 0..TILE_SIZE {
                let [r, g, b, a] = tile_pixel(tile_index, px, py);
                let ax = origin_x + px;
                let ay = origin_y + py;
                let offset = ((ay * ATLAS_W + ax) * 4) as usize;
                pixels[offset] = r;
                pixels[offset + 1] = g;
                pixels[offset + 2] = b;
                pixels[offset + 3] = a;
            }
        }
    }

    (pixels, ATLAS_W, ATLAS_H, TILE_COLS, TILE_ROWS)
}

/// Compute the RGBA color for a pixel within a tile.
fn tile_pixel(tile_index: u32, px: u32, py: u32) -> [u8; 4] {
    match tile_index {
        0 => tile_empty(),
        1 => tile_grass(px, py),
        2 => tile_dirt(px, py),
        3 => tile_stone(px, py),
        4 => tile_water(px, py),
        5 => tile_wood(px, py),
        6 => tile_door(px, py),
        7 => tile_brick(px, py),
        // Tiles 8-15: variations of the above
        8 => tile_dark_grass(px, py),
        9 => tile_light_dirt(px, py),
        10 => tile_dark_stone(px, py),
        11 => tile_deep_water(px, py),
        12 => tile_light_wood(px, py),
        13 => tile_metal_door(px, py),
        14 => tile_mossy_brick(px, py),
        15 => tile_sand(px, py),
        // Tiles 16-23: more variations
        16 => tile_snow(px, py),
        17 => tile_ice(px, py),
        18 => tile_lava(px, py),
        19 => tile_cobblestone(px, py),
        20 => tile_gravel(px, py),
        21 => tile_planks(px, py),
        22 => tile_marble(px, py),
        23 => tile_dark_brick(px, py),
        // Tiles 24-31: fill with solid colors
        24 => [200, 30, 30, 255],   // red
        25 => [30, 200, 30, 255],   // green
        26 => [30, 30, 200, 255],   // blue
        27 => [200, 200, 30, 255],  // yellow
        28 => [200, 30, 200, 255],  // magenta
        29 => [30, 200, 200, 255],  // cyan
        30 => [200, 200, 200, 255], // light gray
        31 => [60, 60, 60, 255],    // dark gray
        _ => [0, 0, 0, 0],
    }
}

/// Simple hash for pseudo-random patterns.
fn hash(x: u32, y: u32, seed: u32) -> u32 {
    let mut h = x.wrapping_mul(374761393)
        .wrapping_add(y.wrapping_mul(668265263))
        .wrapping_add(seed.wrapping_mul(1274126177));
    h = (h ^ (h >> 13)).wrapping_mul(1103515245);
    h = h ^ (h >> 16);
    h
}

// ---- Tile generators ----

fn tile_empty() -> [u8; 4] {
    [0, 0, 0, 0]
}

fn tile_grass(px: u32, py: u32) -> [u8; 4] {
    let base_g: u8 = 140;
    let h = hash(px, py, 1) % 100;
    if h < 15 {
        // darker spots
        [30, (base_g - 30) as u8, 20, 255]
    } else if h < 25 {
        // lighter spots
        [50, (base_g + 20).min(255) as u8, 40, 255]
    } else {
        [40, base_g, 30, 255]
    }
}

fn tile_dirt(px: u32, py: u32) -> [u8; 4] {
    let h = hash(px, py, 2) % 100;
    let base = if h < 20 { 110u8 } else if h < 40 { 125 } else { 120 };
    [base, (base as u16 * 70 / 100) as u8, (base as u16 * 45 / 100) as u8, 255]
}

fn tile_stone(px: u32, py: u32) -> [u8; 4] {
    // Gray stone with darker mortar lines at edges
    if px == 0 || py == 0 || px == 15 || py == 15 {
        // mortar
        [80, 80, 75, 255]
    } else if px == 1 || py == 1 || px == 14 || py == 14 {
        [100, 100, 95, 255]
    } else {
        let h = hash(px, py, 3) % 30;
        let v = 140 + h as u8;
        [v, v, (v as u16 * 95 / 100) as u8, 255]
    }
}

fn tile_water(px: u32, py: u32) -> [u8; 4] {
    let wave = ((px.wrapping_add(py)) % 6) as u8;
    let base_b: u8 = 180;
    let b = base_b.saturating_add(wave * 8);
    let g = 80u8.saturating_add(wave * 4);
    [30, g, b, 220]
}

fn tile_wood(px: u32, py: u32) -> [u8; 4] {
    // Warm brown with horizontal grain lines
    let grain = (py % 4 == 0) || (py % 4 == 1 && hash(px, py, 5) % 3 == 0);
    if grain {
        [100, 65, 30, 255]
    } else {
        let h = hash(px, py, 5) % 15;
        [130 + h as u8, 85 + (h / 2) as u8, 40, 255]
    }
}

fn tile_door(px: u32, py: u32) -> [u8; 4] {
    // Dark brown rectangle with a handle dot
    if px < 2 || px > 13 || py < 1 || py > 14 {
        // frame
        [60, 40, 20, 255]
    } else if px >= 10 && px <= 11 && py >= 7 && py <= 8 {
        // handle
        [200, 180, 50, 255]
    } else {
        // door panel
        [90, 55, 25, 255]
    }
}

fn tile_brick(px: u32, py: u32) -> [u8; 4] {
    // Red-brown bricks with mortar lines
    let mortar_h = py % 8 == 0;
    let offset = if (py / 8) % 2 == 0 { 0 } else { 4 };
    let mortar_v = (px + offset) % 8 == 0;
    if mortar_h || mortar_v {
        [180, 175, 160, 255]
    } else {
        let h = hash(px, py, 7) % 20;
        [160 + h as u8, 70 + (h / 2) as u8, 50, 255]
    }
}

// ---- Variation tiles 8-15 ----

fn tile_dark_grass(px: u32, py: u32) -> [u8; 4] {
    let h = hash(px, py, 8) % 100;
    if h < 20 {
        [15, 80, 10, 255]
    } else {
        [25, 100, 20, 255]
    }
}

fn tile_light_dirt(px: u32, py: u32) -> [u8; 4] {
    let h = hash(px, py, 9) % 30;
    let base = 150 + h as u8;
    [base, (base as u16 * 75 / 100) as u8, (base as u16 * 50 / 100) as u8, 255]
}

fn tile_dark_stone(px: u32, py: u32) -> [u8; 4] {
    if px == 0 || py == 0 || px == 15 || py == 15 {
        [50, 50, 45, 255]
    } else {
        let h = hash(px, py, 10) % 25;
        let v = 90 + h as u8;
        [v, v, v, 255]
    }
}

fn tile_deep_water(px: u32, py: u32) -> [u8; 4] {
    let wave = ((px.wrapping_add(py.wrapping_mul(2))) % 5) as u8;
    [10, 30 + wave * 3, 120 + wave * 10, 230]
}

fn tile_light_wood(px: u32, py: u32) -> [u8; 4] {
    let grain = py % 3 == 0;
    if grain {
        [170, 140, 90, 255]
    } else {
        let h = hash(px, py, 12) % 15;
        [190 + (h / 2) as u8, 155 + (h / 3) as u8, 100, 255]
    }
}

fn tile_metal_door(px: u32, py: u32) -> [u8; 4] {
    if px < 2 || px > 13 || py < 1 || py > 14 {
        [50, 50, 55, 255]
    } else if px >= 10 && px <= 11 && py >= 7 && py <= 8 {
        [220, 220, 100, 255]
    } else {
        let h = hash(px, py, 13) % 10;
        [120 + h as u8, 120 + h as u8, 130 + h as u8, 255]
    }
}

fn tile_mossy_brick(px: u32, py: u32) -> [u8; 4] {
    let mortar_h = py % 8 == 0;
    let offset = if (py / 8) % 2 == 0 { 0 } else { 4 };
    let mortar_v = (px + offset) % 8 == 0;
    if mortar_h || mortar_v {
        [150, 155, 140, 255]
    } else {
        let h = hash(px, py, 14) % 100;
        if h < 30 {
            // moss
            [60, 120, 50, 255]
        } else {
            [140 + (h % 20) as u8, 70, 50, 255]
        }
    }
}

fn tile_sand(px: u32, py: u32) -> [u8; 4] {
    let h = hash(px, py, 15) % 25;
    [210 + (h / 2) as u8, 190 + (h / 2) as u8, 140 + h as u8, 255]
}

// ---- Tiles 16-23 ----

fn tile_snow(px: u32, py: u32) -> [u8; 4] {
    let h = hash(px, py, 16) % 15;
    let v = 235 + (h / 2) as u8;
    [v, v, 255, 255]
}

fn tile_ice(px: u32, py: u32) -> [u8; 4] {
    let h = hash(px, py, 17) % 20;
    [180 + h as u8, 210 + (h / 2) as u8, 240, 200]
}

fn tile_lava(px: u32, py: u32) -> [u8; 4] {
    let h = hash(px, py, 18) % 100;
    if h < 30 {
        [255, 200, 0, 255]
    } else if h < 60 {
        [255, 120, 0, 255]
    } else {
        [200, 50, 0, 255]
    }
}

fn tile_cobblestone(px: u32, py: u32) -> [u8; 4] {
    let cell_x = px / 4;
    let cell_y = py / 4;
    let h = hash(cell_x, cell_y, 19) % 40;
    let edge = px % 4 == 0 || py % 4 == 0;
    if edge {
        [90, 90, 85, 255]
    } else {
        let v = 130 + h as u8;
        [v, v, (v as u16 * 90 / 100) as u8, 255]
    }
}

fn tile_gravel(px: u32, py: u32) -> [u8; 4] {
    let h = hash(px, py, 20) % 50;
    let v = 100 + h as u8;
    [v, v, (v as u16 * 95 / 100) as u8, 255]
}

fn tile_planks(px: u32, _py: u32) -> [u8; 4] {
    let plank = px / 4;
    let h = hash(plank, 0, 21) % 30;
    let edge = px % 4 == 0;
    if edge {
        [80, 50, 25, 255]
    } else {
        [140 + h as u8, 95 + (h / 2) as u8, 50 + (h / 3) as u8, 255]
    }
}

fn tile_marble(px: u32, py: u32) -> [u8; 4] {
    let h = hash(px, py, 22) % 100;
    if h < 10 {
        // vein
        [180, 180, 175, 255]
    } else {
        let v = 220 + (h % 15) as u8;
        [v, v, v, 255]
    }
}

fn tile_dark_brick(px: u32, py: u32) -> [u8; 4] {
    let mortar_h = py % 8 == 0;
    let offset = if (py / 8) % 2 == 0 { 0 } else { 4 };
    let mortar_v = (px + offset) % 8 == 0;
    if mortar_h || mortar_v {
        [60, 55, 50, 255]
    } else {
        let h = hash(px, py, 23) % 20;
        [80 + h as u8, 35 + (h / 2) as u8, 25, 255]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atlas_dimensions_correct() {
        let (data, w, h, cols, rows) = generate_builtin_tiles();
        assert_eq!(w, ATLAS_W);
        assert_eq!(h, ATLAS_H);
        assert_eq!(cols, TILE_COLS);
        assert_eq!(rows, TILE_ROWS);
        assert_eq!(data.len(), (ATLAS_W * ATLAS_H * 4) as usize);
    }

    #[test]
    fn tile_0_is_transparent() {
        let (data, _, _, _, _) = generate_builtin_tiles();
        // Tile 0 is at origin (0,0), check all pixels are transparent
        for py in 0..TILE_SIZE {
            for px in 0..TILE_SIZE {
                let offset = ((py * ATLAS_W + px) * 4) as usize;
                assert_eq!(data[offset + 3], 0, "tile 0 pixel ({px},{py}) should be transparent");
            }
        }
    }

    #[test]
    fn tile_1_has_opaque_pixels() {
        let (data, _, _, _, _) = generate_builtin_tiles();
        // Tile 1 (grass) is at column 1, row 0
        let origin_x = TILE_SIZE;
        let mut has_opaque = false;
        for py in 0..TILE_SIZE {
            for px in 0..TILE_SIZE {
                let offset = (((py) * ATLAS_W + origin_x + px) * 4) as usize;
                if data[offset + 3] > 0 {
                    has_opaque = true;
                }
            }
        }
        assert!(has_opaque, "tile 1 (grass) should have opaque pixels");
    }

    #[test]
    fn all_32_tiles_fit_in_atlas() {
        let (data, w, h, cols, rows) = generate_builtin_tiles();
        assert!(cols * rows >= 32);
        assert_eq!(data.len(), (w * h * 4) as usize);
    }

    #[test]
    fn solid_color_tiles_are_uniform() {
        let (data, _, _, _, _) = generate_builtin_tiles();
        // Tile 24 (red) is at col 0, row 3
        let origin_x = 0;
        let origin_y = 3 * TILE_SIZE;
        let first_offset = ((origin_y * ATLAS_W + origin_x) * 4) as usize;
        let expected_r = data[first_offset];
        let expected_g = data[first_offset + 1];
        let expected_b = data[first_offset + 2];
        for py in 0..TILE_SIZE {
            for px in 0..TILE_SIZE {
                let offset = (((origin_y + py) * ATLAS_W + origin_x + px) * 4) as usize;
                assert_eq!(data[offset], expected_r);
                assert_eq!(data[offset + 1], expected_g);
                assert_eq!(data[offset + 2], expected_b);
                assert_eq!(data[offset + 3], 255);
            }
        }
    }
}
