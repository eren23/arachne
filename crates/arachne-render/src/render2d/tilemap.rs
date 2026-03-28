use arachne_math::Vec2;
use crate::buffer::DynamicBuffer;
use crate::render2d::sprite::SpriteVertex;

// ---------------------------------------------------------------------------
// Tile data
// ---------------------------------------------------------------------------

/// A single tile in a tilemap layer.
#[derive(Clone, Copy, Debug)]
pub struct Tile {
    pub index: u16,
    pub flip_x: bool,
    pub flip_y: bool,
}

impl Tile {
    pub fn new(index: u16) -> Self {
        Self {
            index,
            flip_x: false,
            flip_y: false,
        }
    }
}

// ---------------------------------------------------------------------------
// TilemapLayer
// ---------------------------------------------------------------------------

/// A layer of tiles with a fixed grid.
pub struct TilemapLayer {
    pub tiles: Vec<Vec<Option<Tile>>>,
    pub tile_size: Vec2,
    pub width: u32,
    pub height: u32,
    pub atlas_columns: u32,
    pub atlas_rows: u32,
}

impl TilemapLayer {
    pub fn new(width: u32, height: u32, tile_size: Vec2, atlas_columns: u32, atlas_rows: u32) -> Self {
        let tiles = vec![vec![None; width as usize]; height as usize];
        Self {
            tiles,
            tile_size,
            width,
            height,
            atlas_columns,
            atlas_rows,
        }
    }

    pub fn set_tile(&mut self, x: u32, y: u32, tile: Option<Tile>) {
        if x < self.width && y < self.height {
            self.tiles[y as usize][x as usize] = tile;
        }
    }

    pub fn get_tile(&self, x: u32, y: u32) -> Option<&Tile> {
        if x < self.width && y < self.height {
            self.tiles[y as usize][x as usize].as_ref()
        } else {
            None
        }
    }

    /// Compute UV rect for a tile index in the atlas.
    pub fn tile_uv(&self, index: u16) -> [f32; 4] {
        let col = (index as u32) % self.atlas_columns;
        let row = (index as u32) / self.atlas_columns;
        let u = col as f32 / self.atlas_columns as f32;
        let v = row as f32 / self.atlas_rows as f32;
        let uw = 1.0 / self.atlas_columns as f32;
        let vh = 1.0 / self.atlas_rows as f32;
        [u, v, uw, vh]
    }
}

// ---------------------------------------------------------------------------
// TilemapRenderer
// ---------------------------------------------------------------------------

/// Renders tilemap layers efficiently using chunked vertex buffers.
pub struct TilemapRenderer {
    vertices: Vec<SpriteVertex>,
    indices: Vec<u32>,
    vertex_buffer: DynamicBuffer,
    index_buffer: DynamicBuffer,
}

impl TilemapRenderer {
    pub fn new(device: &wgpu::Device) -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
            vertex_buffer: DynamicBuffer::new(device, 16384, wgpu::BufferUsages::VERTEX),
            index_buffer: DynamicBuffer::new(device, 8192, wgpu::BufferUsages::INDEX),
        }
    }

    pub fn begin_frame(&mut self) {
        self.vertices.clear();
        self.indices.clear();
        self.vertex_buffer.clear();
        self.index_buffer.clear();
    }

    /// Build geometry for a tilemap layer.
    pub fn build_layer(&mut self, layer: &TilemapLayer) {
        for ty in 0..layer.height {
            for tx in 0..layer.width {
                if let Some(tile) = layer.get_tile(tx, ty) {
                    let base = self.vertices.len() as u32;

                    let x = tx as f32 * layer.tile_size.x;
                    let y = ty as f32 * layer.tile_size.y;
                    let w = layer.tile_size.x;
                    let h = layer.tile_size.y;

                    let [u, v, uw, vh] = layer.tile_uv(tile.index);

                    let (u0, u1) = if tile.flip_x { (u + uw, u) } else { (u, u + uw) };
                    let (v0, v1) = if tile.flip_y { (v + vh, v) } else { (v, v + vh) };

                    // Bottom-left, bottom-right, top-right, top-left
                    self.vertices.push(SpriteVertex {
                        position: [x, y + h],
                        uv: [u0, v1],
                    });
                    self.vertices.push(SpriteVertex {
                        position: [x + w, y + h],
                        uv: [u1, v1],
                    });
                    self.vertices.push(SpriteVertex {
                        position: [x + w, y],
                        uv: [u1, v0],
                    });
                    self.vertices.push(SpriteVertex {
                        position: [x, y],
                        uv: [u0, v0],
                    });

                    self.indices.extend_from_slice(&[
                        base, base + 1, base + 2,
                        base, base + 2, base + 3,
                    ]);
                }
            }
        }
    }

    /// Upload geometry to GPU.
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> TilemapPrepared {
        if self.vertices.is_empty() {
            return TilemapPrepared {
                vertex_count: 0,
                index_count: 0,
            };
        }

        self.vertex_buffer.clear();
        self.index_buffer.clear();
        self.vertex_buffer.write(device, queue, &self.vertices);
        self.index_buffer.write(device, queue, &self.indices);

        TilemapPrepared {
            vertex_count: self.vertices.len() as u32,
            index_count: self.indices.len() as u32,
        }
    }

    /// Render the tilemap using the sprite pipeline (same vertex format).
    pub fn render<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        prepared: &TilemapPrepared,
        pipeline: &'a wgpu::RenderPipeline,
        camera_bind_group: &'a wgpu::BindGroup,
        atlas_bind_group: &'a wgpu::BindGroup,
    ) {
        if prepared.index_count == 0 {
            return;
        }

        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, Some(camera_bind_group), &[]);
        pass.set_bind_group(1, Some(atlas_bind_group), &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.buffer().slice(..));
        pass.set_index_buffer(
            self.index_buffer.buffer().slice(..),
            wgpu::IndexFormat::Uint32,
        );
        pass.draw_indexed(0..prepared.index_count, 0, 0..1);
    }

    pub fn tile_quad_count(&self) -> usize {
        self.vertices.len() / 4
    }
}

pub struct TilemapPrepared {
    pub vertex_count: u32,
    pub index_count: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tilemap_layer_set_get() {
        let mut layer = TilemapLayer::new(10, 10, Vec2::new(16.0, 16.0), 8, 8);
        assert!(layer.get_tile(0, 0).is_none());

        layer.set_tile(3, 5, Some(Tile::new(42)));
        let tile = layer.get_tile(3, 5).unwrap();
        assert_eq!(tile.index, 42);
    }

    #[test]
    fn tile_uv_calculation() {
        let layer = TilemapLayer::new(1, 1, Vec2::new(16.0, 16.0), 4, 4);

        // Tile index 0 -> (0,0) in atlas
        let [u, v, uw, vh] = layer.tile_uv(0);
        assert_eq!(u, 0.0);
        assert_eq!(v, 0.0);
        assert_eq!(uw, 0.25);
        assert_eq!(vh, 0.25);

        // Tile index 5 -> (1, 1) in 4x4 atlas
        let [u, v, _, _] = layer.tile_uv(5);
        assert!((u - 0.25).abs() < 0.001);
        assert!((v - 0.25).abs() < 0.001);
    }

    #[test]
    fn tilemap_renderer_build() {
        let (device, _queue) = test_ctx();
        let mut renderer = TilemapRenderer::new(&device);
        renderer.begin_frame();

        let mut layer = TilemapLayer::new(4, 4, Vec2::new(16.0, 16.0), 8, 8);
        layer.set_tile(0, 0, Some(Tile::new(0)));
        layer.set_tile(1, 0, Some(Tile::new(1)));
        layer.set_tile(0, 1, Some(Tile::new(8)));

        renderer.build_layer(&layer);

        // 3 tiles * 4 vertices = 12 vertices
        assert_eq!(renderer.tile_quad_count(), 3);
        assert_eq!(renderer.vertices.len(), 12);
        assert_eq!(renderer.indices.len(), 18); // 3 * 6
    }

    #[test]
    fn tilemap_empty_layer() {
        let (device, _queue) = test_ctx();
        let mut renderer = TilemapRenderer::new(&device);
        renderer.begin_frame();

        let layer = TilemapLayer::new(4, 4, Vec2::new(16.0, 16.0), 8, 8);
        renderer.build_layer(&layer);

        assert_eq!(renderer.tile_quad_count(), 0);
    }

    #[test]
    fn tile_flip() {
        let (device, _queue) = test_ctx();
        let mut renderer = TilemapRenderer::new(&device);
        renderer.begin_frame();

        let mut layer = TilemapLayer::new(2, 1, Vec2::new(16.0, 16.0), 4, 4);
        layer.set_tile(0, 0, Some(Tile { index: 0, flip_x: true, flip_y: false }));
        layer.set_tile(1, 0, Some(Tile { index: 0, flip_x: false, flip_y: false }));

        renderer.build_layer(&layer);

        // Flipped tile should have swapped UVs
        let normal_u0 = renderer.vertices[4].uv[0]; // tile 1, first vertex
        let flipped_u0 = renderer.vertices[0].uv[0]; // tile 0 (flipped), first vertex
        assert_ne!(normal_u0, flipped_u0, "flipped tile should have different UV");
    }

    fn test_ctx() -> (wgpu::Device, wgpu::Queue) {
        pollster::block_on(async {
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
        })
    }
}
