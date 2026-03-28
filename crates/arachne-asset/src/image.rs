/// RGBA8 image and PNG decoding.

pub struct Image {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>, // RGBA8, row-major, 4 bytes per pixel
}

impl Image {
    pub fn new(width: u32, height: u32, data: Vec<u8>) -> Self {
        assert_eq!(
            data.len(),
            (width as usize) * (height as usize) * 4,
            "Image data length mismatch: expected {}x{}x4={}, got {}",
            width,
            height,
            (width as usize) * (height as usize) * 4,
            data.len()
        );
        Image {
            width,
            height,
            data,
        }
    }

    /// Create a solid-color image for testing.
    pub fn solid(width: u32, height: u32, rgba: [u8; 4]) -> Self {
        let pixel_count = (width as usize) * (height as usize);
        let mut data = Vec::with_capacity(pixel_count * 4);
        for _ in 0..pixel_count {
            data.extend_from_slice(&rgba);
        }
        Image {
            width,
            height,
            data,
        }
    }

    /// Decode a PNG from raw bytes into an RGBA8 image.
    pub fn decode_png(bytes: &[u8]) -> Result<Image, String> {
        let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        let mut reader = decoder.read_info().map_err(|e| format!("PNG header: {}", e))?;
        let mut buf = vec![0u8; reader.output_buffer_size()];
        let info = reader
            .next_frame(&mut buf)
            .map_err(|e| format!("PNG decode: {}", e))?;

        let width = info.width;
        let height = info.height;
        let src = &buf[..info.buffer_size()];

        let data = match info.color_type {
            png::ColorType::Rgba => src.to_vec(),
            png::ColorType::Rgb => {
                let mut rgba = Vec::with_capacity((width as usize) * (height as usize) * 4);
                for chunk in src.chunks_exact(3) {
                    rgba.extend_from_slice(chunk);
                    rgba.push(255);
                }
                rgba
            }
            png::ColorType::Grayscale => {
                let mut rgba = Vec::with_capacity((width as usize) * (height as usize) * 4);
                for &g in src.iter() {
                    rgba.push(g);
                    rgba.push(g);
                    rgba.push(g);
                    rgba.push(255);
                }
                rgba
            }
            png::ColorType::GrayscaleAlpha => {
                let mut rgba = Vec::with_capacity((width as usize) * (height as usize) * 4);
                for chunk in src.chunks_exact(2) {
                    rgba.push(chunk[0]);
                    rgba.push(chunk[0]);
                    rgba.push(chunk[0]);
                    rgba.push(chunk[1]);
                }
                rgba
            }
            png::ColorType::Indexed => {
                return Err("indexed PNG not supported; convert to RGBA first".into());
            }
        };

        Ok(Image {
            width,
            height,
            data,
        })
    }

    /// Encode this image as PNG bytes.
    pub fn encode_png(&self) -> Result<Vec<u8>, String> {
        let mut out = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut out, self.width, self.height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder
                .write_header()
                .map_err(|e| format!("PNG encode header: {}", e))?;
            writer
                .write_image_data(&self.data)
                .map_err(|e| format!("PNG encode data: {}", e))?;
        }
        Ok(out)
    }

    /// Get the RGBA pixel at (x, y).
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let offset = ((y as usize) * (self.width as usize) + (x as usize)) * 4;
        [
            self.data[offset],
            self.data[offset + 1],
            self.data[offset + 2],
            self.data[offset + 3],
        ]
    }

    /// Total bytes of pixel data.
    pub fn size_bytes(&self) -> usize {
        self.data.len()
    }
}

/// UV rectangle describing where a sprite was placed in an atlas.
#[derive(Clone, Debug)]
pub struct UvRect {
    /// Pixel position in the atlas.
    pub x: u32,
    pub y: u32,
    /// Original sprite dimensions.
    pub width: u32,
    pub height: u32,
    /// Normalized UV coordinates [0..1].
    pub u_min: f32,
    pub v_min: f32,
    pub u_max: f32,
    pub v_max: f32,
}

/// Pack multiple images into a single atlas using a greedy shelf algorithm.
///
/// Images are sorted by height (tallest first) and placed left-to-right on shelves.
/// When a shelf runs out of horizontal space, a new shelf is started.
///
/// Returns the packed atlas image and a `UvRect` per input image (same order as input).
pub fn pack_atlas(
    images: &[Image],
    max_width: u32,
    max_height: u32,
) -> Result<(Image, Vec<UvRect>), String> {
    if images.is_empty() {
        let atlas = Image::new(max_width, max_height, vec![0u8; (max_width as usize) * (max_height as usize) * 4]);
        return Ok((atlas, Vec::new()));
    }

    // Sort indices by height descending for better shelf packing.
    let mut indices: Vec<usize> = (0..images.len()).collect();
    indices.sort_by(|&a, &b| images[b].height.cmp(&images[a].height));

    let mut rects: Vec<UvRect> = (0..images.len())
        .map(|_| UvRect {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            u_min: 0.0,
            v_min: 0.0,
            u_max: 0.0,
            v_max: 0.0,
        })
        .collect();

    let mut shelf_y: u32 = 0;
    let mut shelf_height: u32 = 0;
    let mut cursor_x: u32 = 0;

    for &idx in &indices {
        let img = &images[idx];
        if img.width > max_width || img.height > max_height {
            return Err(format!(
                "image {} ({}x{}) exceeds atlas dimensions ({}x{})",
                idx, img.width, img.height, max_width, max_height
            ));
        }

        // Check if image fits on current shelf.
        if cursor_x + img.width > max_width {
            // Start new shelf.
            shelf_y += shelf_height;
            shelf_height = 0;
            cursor_x = 0;
        }

        if shelf_y + img.height > max_height {
            return Err(format!(
                "atlas overflow: cannot fit image {} ({}x{}) at y={}",
                idx, img.width, img.height, shelf_y
            ));
        }

        rects[idx] = UvRect {
            x: cursor_x,
            y: shelf_y,
            width: img.width,
            height: img.height,
            u_min: cursor_x as f32 / max_width as f32,
            v_min: shelf_y as f32 / max_height as f32,
            u_max: (cursor_x + img.width) as f32 / max_width as f32,
            v_max: (shelf_y + img.height) as f32 / max_height as f32,
        };

        cursor_x += img.width;
        shelf_height = shelf_height.max(img.height);
    }

    // Blit images into atlas.
    let mut atlas_data = vec![0u8; (max_width as usize) * (max_height as usize) * 4];

    for (idx, img) in images.iter().enumerate() {
        let rect = &rects[idx];
        for row in 0..img.height {
            let src_start = (row as usize) * (img.width as usize) * 4;
            let src_end = src_start + (img.width as usize) * 4;
            let dst_start =
                ((rect.y + row) as usize * max_width as usize + rect.x as usize) * 4;
            let dst_end = dst_start + (img.width as usize) * 4;
            atlas_data[dst_start..dst_end].copy_from_slice(&img.data[src_start..src_end]);
        }
    }

    let atlas = Image {
        width: max_width,
        height: max_height,
        data: atlas_data,
    };

    Ok((atlas, rects))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_png(width: u32, height: u32, r: u8, g: u8, b: u8) -> Vec<u8> {
        let img = Image::solid(width, height, [r, g, b, 255]);
        img.encode_png().unwrap()
    }

    #[test]
    fn decode_png_rgba() {
        let png_data = make_test_png(4, 4, 255, 0, 128);
        let img = Image::decode_png(&png_data).unwrap();
        assert_eq!(img.width, 4);
        assert_eq!(img.height, 4);
        assert_eq!(img.pixel(0, 0), [255, 0, 128, 255]);
        assert_eq!(img.pixel(3, 3), [255, 0, 128, 255]);
    }

    #[test]
    fn decode_png_dimensions_and_pixels() {
        let width = 16;
        let height = 8;
        // Gradient image
        let mut data = Vec::with_capacity((width * height * 4) as usize);
        for y in 0..height {
            for x in 0..width {
                data.push((x * 16) as u8); // R
                data.push((y * 32) as u8); // G
                data.push(128);            // B
                data.push(255);            // A
            }
        }
        let img = Image::new(width, height, data);
        let png_bytes = img.encode_png().unwrap();
        let decoded = Image::decode_png(&png_bytes).unwrap();

        assert_eq!(decoded.width, width);
        assert_eq!(decoded.height, height);
        // Spot check pixels
        assert_eq!(decoded.pixel(0, 0), [0, 0, 128, 255]);
        assert_eq!(decoded.pixel(15, 0), [240, 0, 128, 255]);
        assert_eq!(decoded.pixel(0, 7), [0, 224, 128, 255]);
    }

    #[test]
    fn atlas_pack_10_images_no_overlap() {
        let images: Vec<Image> = (0..10)
            .map(|i| {
                let w = 30 + (i as u32 % 5) * 10; // 30..70
                let h = 20 + (i as u32 % 4) * 10;  // 20..50
                let c = (i as u8 * 25) + 10;
                Image::solid(w, h, [c, 255 - c, c / 2, 255])
            })
            .collect();

        let (atlas, rects) = pack_atlas(&images, 512, 512).unwrap();
        assert_eq!(rects.len(), 10);
        assert_eq!(atlas.width, 512);
        assert_eq!(atlas.height, 512);

        // Verify all UVs are valid (within [0,1]).
        for r in &rects {
            assert!(r.u_min >= 0.0 && r.u_min <= 1.0, "u_min out of range: {}", r.u_min);
            assert!(r.v_min >= 0.0 && r.v_min <= 1.0, "v_min out of range: {}", r.v_min);
            assert!(r.u_max >= 0.0 && r.u_max <= 1.0, "u_max out of range: {}", r.u_max);
            assert!(r.v_max >= 0.0 && r.v_max <= 1.0, "v_max out of range: {}", r.v_max);
            assert!(r.u_min < r.u_max);
            assert!(r.v_min < r.v_max);
        }

        // Verify no overlap: for each pair of rects, check they don't intersect.
        for i in 0..rects.len() {
            for j in (i + 1)..rects.len() {
                let a = &rects[i];
                let b = &rects[j];
                let no_overlap = a.x + a.width <= b.x
                    || b.x + b.width <= a.x
                    || a.y + a.height <= b.y
                    || b.y + b.height <= a.y;
                assert!(
                    no_overlap,
                    "overlap between rect {} ({},{} {}x{}) and rect {} ({},{} {}x{})",
                    i, a.x, a.y, a.width, a.height, j, b.x, b.y, b.width, b.height
                );
            }
        }

        // Verify pixel data was blitted correctly: check first pixel of each sprite.
        for (idx, img) in images.iter().enumerate() {
            let r = &rects[idx];
            let atlas_pixel = atlas.pixel(r.x, r.y);
            let src_pixel = img.pixel(0, 0);
            assert_eq!(
                atlas_pixel, src_pixel,
                "image {} pixel mismatch at atlas ({},{})",
                idx, r.x, r.y
            );
        }
    }

    #[test]
    fn atlas_pack_100_sprites_low_waste() {
        // 100 sprites of 204x204 in a 2048x2048 atlas.
        let images: Vec<Image> = (0..100)
            .map(|i| {
                let c = (i as u8).wrapping_mul(7);
                Image::solid(204, 204, [c, c, c, 255])
            })
            .collect();

        let (atlas, rects) = pack_atlas(&images, 2048, 2048).unwrap();
        assert_eq!(rects.len(), 100);

        let total_pixels = (atlas.width as u64) * (atlas.height as u64);
        let used_pixels: u64 = rects.iter().map(|r| (r.width as u64) * (r.height as u64)).sum();
        let waste = 1.0 - (used_pixels as f64 / total_pixels as f64);

        eprintln!(
            "Atlas waste: {:.2}% (used {} / {} pixels)",
            waste * 100.0,
            used_pixels,
            total_pixels
        );
        assert!(
            waste < 0.05,
            "atlas waste {:.2}% exceeds 5% threshold",
            waste * 100.0
        );
    }

    #[test]
    fn bench_png_decode_throughput() {
        // Create a 256x256 test PNG.
        let img = Image::solid(256, 256, [128, 64, 32, 255]);
        let png_bytes = img.encode_png().unwrap();

        let iterations = 100;
        let start = std::time::Instant::now();
        for _ in 0..iterations {
            let decoded = Image::decode_png(&png_bytes).unwrap();
            std::hint::black_box(&decoded);
        }
        let elapsed = start.elapsed();

        let per_sec = iterations as f64 / elapsed.as_secs_f64();
        eprintln!(
            "PNG decode: {:.0} images/sec ({} iterations in {:.3}ms)",
            per_sec,
            iterations,
            elapsed.as_secs_f64() * 1000.0
        );
        assert!(
            per_sec >= 100.0,
            "PNG decode throughput {:.0} images/sec is below 100/sec threshold",
            per_sec
        );
    }
}
