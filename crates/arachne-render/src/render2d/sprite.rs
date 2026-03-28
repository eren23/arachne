use arachne_math::{Color, Mat4, Vec2, Rect};
use crate::buffer::DynamicBuffer;
use crate::camera::CameraUniform;
use crate::pipeline::{self, PipelineCache, PipelineKey, PrimitiveTopologyKey};
use crate::texture::TextureHandle;
use crate::render2d::batch::{Batcher, BatchStats, DrawCommand, SortKey};

// ---------------------------------------------------------------------------
// Sprite data types
// ---------------------------------------------------------------------------

/// Anchor point for sprite positioning.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Anchor {
    Center,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    TopCenter,
    BottomCenter,
    CenterLeft,
    CenterRight,
}

impl Anchor {
    /// Returns offset from center in normalized [-0.5, 0.5] space.
    pub fn offset(self) -> Vec2 {
        match self {
            Self::Center => Vec2::ZERO,
            Self::TopLeft => Vec2::new(-0.5, 0.5),
            Self::TopRight => Vec2::new(0.5, 0.5),
            Self::BottomLeft => Vec2::new(-0.5, -0.5),
            Self::BottomRight => Vec2::new(0.5, -0.5),
            Self::TopCenter => Vec2::new(0.0, 0.5),
            Self::BottomCenter => Vec2::new(0.0, -0.5),
            Self::CenterLeft => Vec2::new(-0.5, 0.0),
            Self::CenterRight => Vec2::new(0.5, 0.0),
        }
    }
}

impl Default for Anchor {
    fn default() -> Self {
        Self::Center
    }
}

/// Sprite component: references a texture and rendering properties.
#[derive(Clone, Debug)]
pub struct Sprite {
    pub texture: TextureHandle,
    pub color: Color,
    pub flip_x: bool,
    pub flip_y: bool,
    pub anchor: Anchor,
    pub custom_size: Option<Vec2>,
}

impl Sprite {
    pub fn new(texture: TextureHandle) -> Self {
        Self {
            texture,
            color: Color::WHITE,
            flip_x: false,
            flip_y: false,
            anchor: Anchor::Center,
            custom_size: None,
        }
    }
}

// ---------------------------------------------------------------------------
// GPU vertex types
// ---------------------------------------------------------------------------

/// Per-vertex data for the unit quad.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SpriteVertex {
    pub position: [f32; 2],
    pub uv: [f32; 2],
}

impl SpriteVertex {
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Self>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x2,
            },
            wgpu::VertexAttribute {
                offset: 8,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32x2,
            },
        ],
    };
}

/// Unit quad vertices (two triangles).
pub const QUAD_VERTICES: [SpriteVertex; 4] = [
    SpriteVertex { position: [-0.5, -0.5], uv: [0.0, 1.0] }, // bottom-left
    SpriteVertex { position: [ 0.5, -0.5], uv: [1.0, 1.0] }, // bottom-right
    SpriteVertex { position: [ 0.5,  0.5], uv: [1.0, 0.0] }, // top-right
    SpriteVertex { position: [-0.5,  0.5], uv: [0.0, 0.0] }, // top-left
];

pub const QUAD_INDICES: [u16; 6] = [0, 1, 2, 2, 3, 0];

/// Per-instance data uploaded to the GPU.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SpriteInstance {
    pub model_0: [f32; 4],
    pub model_1: [f32; 4],
    pub model_2: [f32; 4],
    pub model_3: [f32; 4],
    pub uv_rect: [f32; 4],
    pub color: [f32; 4],
}

impl SpriteInstance {
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Self>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &[
            wgpu::VertexAttribute { offset: 0,  shader_location: 2, format: wgpu::VertexFormat::Float32x4 },
            wgpu::VertexAttribute { offset: 16, shader_location: 3, format: wgpu::VertexFormat::Float32x4 },
            wgpu::VertexAttribute { offset: 32, shader_location: 4, format: wgpu::VertexFormat::Float32x4 },
            wgpu::VertexAttribute { offset: 48, shader_location: 5, format: wgpu::VertexFormat::Float32x4 },
            wgpu::VertexAttribute { offset: 64, shader_location: 6, format: wgpu::VertexFormat::Float32x4 },
            wgpu::VertexAttribute { offset: 80, shader_location: 7, format: wgpu::VertexFormat::Float32x4 },
        ],
    };

    pub fn from_sprite(transform: &Mat4, uv_rect: Rect, color: Color, flip_x: bool, flip_y: bool) -> Self {
        let mut uv = [
            uv_rect.min.x,
            uv_rect.min.y,
            uv_rect.width(),
            uv_rect.height(),
        ];

        if flip_x {
            uv[0] += uv[2];
            uv[2] = -uv[2];
        }
        if flip_y {
            uv[1] += uv[3];
            uv[3] = -uv[3];
        }

        Self {
            model_0: transform.cols[0],
            model_1: transform.cols[1],
            model_2: transform.cols[2],
            model_3: transform.cols[3],
            uv_rect: uv,
            color: [color.r, color.g, color.b, color.a],
        }
    }
}

// ---------------------------------------------------------------------------
// SpriteRenderer
// ---------------------------------------------------------------------------

/// Collects sprite draw calls, sorts by Z and texture, batches into instanced draws.
pub struct SpriteRenderer {
    instances: Vec<(TextureHandle, f32, SpriteInstance)>,
    instance_buffer: DynamicBuffer,
    quad_vb: wgpu::Buffer,
    quad_ib: wgpu::Buffer,
    pipeline_key: PipelineKey,
    shader_source: &'static str,
}

impl SpriteRenderer {
    pub fn new(device: &wgpu::Device) -> Self {
        use wgpu::util::DeviceExt;

        let quad_vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("sprite_quad_vb"),
            contents: bytemuck::cast_slice(&QUAD_VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let quad_ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("sprite_quad_ib"),
            contents: bytemuck::cast_slice(&QUAD_INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });

        let shader_source = include_str!("../shaders/sprite.wgsl");
        let shader_hash = pipeline::hash_shader_source(shader_source);

        let instance_buffer = DynamicBuffer::new(
            device,
            4096,
            wgpu::BufferUsages::VERTEX,
        );

        Self {
            instances: Vec::new(),
            instance_buffer,
            quad_vb,
            quad_ib,
            pipeline_key: PipelineKey {
                shader_hash,
                vertex_layout_hash: 0, // computed once
                blend_enabled: true,
                depth_enabled: false,
                topology: PrimitiveTopologyKey::TriangleList,
            },
            shader_source,
        }
    }

    /// Begin a new frame, clearing previous draw data.
    pub fn begin_frame(&mut self) {
        self.instances.clear();
        self.instance_buffer.clear();
    }

    /// Queue a sprite for rendering.
    pub fn draw(
        &mut self,
        sprite: &Sprite,
        transform: &Mat4,
        uv_rect: Rect,
        depth: f32,
    ) {
        let instance = SpriteInstance::from_sprite(
            transform,
            uv_rect,
            sprite.color,
            sprite.flip_x,
            sprite.flip_y,
        );
        self.instances.push((sprite.texture, depth, instance));
    }

    /// Sort, batch, and produce draw commands. Returns batch stats.
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> (Vec<SpriteBatch>, BatchStats) {
        if self.instances.is_empty() {
            return (Vec::new(), BatchStats::default());
        }

        // Sort by texture then depth
        self.instances.sort_by(|a, b| {
            a.0 .0
                .cmp(&b.0 .0)
                .then_with(|| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        });

        // Upload all instances to GPU buffer
        let instance_data: Vec<SpriteInstance> =
            self.instances.iter().map(|(_, _, inst)| *inst).collect();
        self.instance_buffer.clear();
        self.instance_buffer.write(device, queue, &instance_data);

        // Build batches: group by texture
        let mut batches = Vec::new();
        let mut batch_start = 0usize;
        let mut current_tex = self.instances[0].0;
        let total = self.instances.len();

        for i in 1..=total {
            let new_tex = if i < total {
                self.instances[i].0 != current_tex
            } else {
                true
            };

            if new_tex {
                let count = i - batch_start;
                batches.push(SpriteBatch {
                    texture: current_tex,
                    instance_offset: batch_start as u32,
                    instance_count: count as u32,
                });
                if i < total {
                    batch_start = i;
                    current_tex = self.instances[i].0;
                }
            }
        }

        let stats = BatchStats {
            total_commands: total as u32,
            merged_commands: (total as u32).saturating_sub(batches.len() as u32),
            draw_calls: batches.len() as u32,
        };

        (batches, stats)
    }

    /// Render the prepared sprite batches into a render pass.
    pub fn render<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        batches: &[SpriteBatch],
        pipeline: &'a wgpu::RenderPipeline,
        camera_bind_group: &'a wgpu::BindGroup,
        texture_bind_groups: &'a dyn Fn(TextureHandle) -> &'a wgpu::BindGroup,
    ) {
        if batches.is_empty() {
            return;
        }

        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, Some(camera_bind_group), &[]);
        pass.set_vertex_buffer(0, self.quad_vb.slice(..));
        pass.set_vertex_buffer(1, self.instance_buffer.buffer().slice(..));
        pass.set_index_buffer(self.quad_ib.slice(..), wgpu::IndexFormat::Uint16);

        for batch in batches {
            let bg = texture_bind_groups(batch.texture);
            pass.set_bind_group(1, Some(bg), &[]);
            pass.draw_indexed(
                0..6,
                0,
                batch.instance_offset..batch.instance_offset + batch.instance_count,
            );
        }
    }

    /// Returns the quad vertex buffer for direct render pass usage.
    pub fn quad_vertex_buffer(&self) -> &wgpu::Buffer { &self.quad_vb }

    /// Returns the quad index buffer for direct render pass usage.
    pub fn quad_index_buffer(&self) -> &wgpu::Buffer { &self.quad_ib }

    /// Returns a slice of the instance buffer for direct render pass usage.
    pub fn instance_buffer_slice(&self) -> wgpu::BufferSlice<'_> {
        self.instance_buffer.buffer().slice(..)
    }

    pub fn shader_source(&self) -> &'static str {
        self.shader_source
    }

    pub fn pipeline_key(&self) -> &PipelineKey {
        &self.pipeline_key
    }

    pub fn instance_count(&self) -> usize {
        self.instances.len()
    }
}

/// A batch of sprites sharing the same texture.
#[derive(Clone, Debug)]
pub struct SpriteBatch {
    pub texture: TextureHandle,
    pub instance_offset: u32,
    pub instance_count: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use arachne_math::Vec3;

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

    #[test]
    fn anchor_offsets() {
        assert_eq!(Anchor::Center.offset(), Vec2::ZERO);
        assert_eq!(Anchor::TopLeft.offset(), Vec2::new(-0.5, 0.5));
        assert_eq!(Anchor::BottomRight.offset(), Vec2::new(0.5, -0.5));
    }

    #[test]
    fn sprite_instance_from_sprite() {
        let transform = Mat4::IDENTITY;
        let uv = Rect::new(Vec2::ZERO, Vec2::ONE);
        let inst = SpriteInstance::from_sprite(&transform, uv, Color::WHITE, false, false);
        assert_eq!(inst.color, [1.0, 1.0, 1.0, 1.0]);
        assert_eq!(inst.uv_rect, [0.0, 0.0, 1.0, 1.0]);
    }

    #[test]
    fn sprite_instance_flip() {
        let transform = Mat4::IDENTITY;
        let uv = Rect::new(Vec2::ZERO, Vec2::ONE);
        let inst = SpriteInstance::from_sprite(&transform, uv, Color::WHITE, true, false);
        // flip_x: uv_x starts at 1.0, width is -1.0
        assert_eq!(inst.uv_rect[0], 1.0);
        assert_eq!(inst.uv_rect[2], -1.0);
    }

    #[test]
    fn sprite_renderer_same_texture_one_draw_call() {
        let (device, queue) = test_ctx();
        let mut renderer = SpriteRenderer::new(&device);
        let tex = TextureHandle(0);

        renderer.begin_frame();
        let transform = Mat4::IDENTITY;
        let uv = Rect::new(Vec2::ZERO, Vec2::ONE);

        for _ in 0..100 {
            let sprite = Sprite::new(tex);
            renderer.draw(&sprite, &transform, uv, 0.0);
        }

        let (batches, stats) = renderer.prepare(&device, &queue);
        assert_eq!(batches.len(), 1, "100 sprites/1 texture -> 1 draw call");
        assert_eq!(batches[0].instance_count, 100);
        assert_eq!(stats.draw_calls, 1);
    }

    #[test]
    fn sprite_renderer_four_textures_four_draw_calls() {
        let (device, queue) = test_ctx();
        let mut renderer = SpriteRenderer::new(&device);

        renderer.begin_frame();
        let transform = Mat4::IDENTITY;
        let uv = Rect::new(Vec2::ZERO, Vec2::ONE);

        for i in 0..100u32 {
            let tex = TextureHandle(i % 4);
            let sprite = Sprite::new(tex);
            renderer.draw(&sprite, &transform, uv, 0.0);
        }

        let (batches, stats) = renderer.prepare(&device, &queue);
        assert_eq!(batches.len(), 4, "100 sprites/4 textures -> 4 draw calls");
        assert_eq!(stats.draw_calls, 4);
    }

    #[test]
    fn sprite_depth_sorting() {
        let (device, queue) = test_ctx();
        let mut renderer = SpriteRenderer::new(&device);
        let tex = TextureHandle(0);

        renderer.begin_frame();
        let uv = Rect::new(Vec2::ZERO, Vec2::ONE);

        // Add sprites at different depths, intentionally out of order
        for depth in [5.0f32, 1.0, 3.0, 0.0, 2.0, 4.0] {
            let sprite = Sprite::new(tex);
            let t = Mat4::from_translation(Vec3::new(0.0, 0.0, depth));
            renderer.draw(&sprite, &t, uv, depth);
        }

        let (batches, _) = renderer.prepare(&device, &queue);
        // All same texture -> 1 batch
        assert_eq!(batches.len(), 1);
        // Instances should be sorted by depth within the batch
    }
}
