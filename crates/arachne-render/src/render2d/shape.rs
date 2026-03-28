use arachne_math::{Color, Vec2};
use crate::buffer::DynamicBuffer;

// ---------------------------------------------------------------------------
// Shape vertex
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ShapeVertex {
    pub position: [f32; 2],
    pub color: [f32; 4],
}

impl ShapeVertex {
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
                format: wgpu::VertexFormat::Float32x4,
            },
        ],
    };

    fn new(pos: Vec2, color: Color) -> Self {
        Self {
            position: [pos.x, pos.y],
            color: [color.r, color.g, color.b, color.a],
        }
    }
}

// ---------------------------------------------------------------------------
// ShapeRenderer
// ---------------------------------------------------------------------------

/// Immediate-mode shape renderer. Accumulates vertices/indices per frame.
pub struct ShapeRenderer {
    vertices: Vec<ShapeVertex>,
    indices: Vec<u32>,
    vertex_buffer: DynamicBuffer,
    index_buffer: DynamicBuffer,
}

impl ShapeRenderer {
    pub fn new(device: &wgpu::Device) -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
            vertex_buffer: DynamicBuffer::new(device, 4096, wgpu::BufferUsages::VERTEX),
            index_buffer: DynamicBuffer::new(device, 2048, wgpu::BufferUsages::INDEX),
        }
    }

    pub fn begin_frame(&mut self) {
        self.vertices.clear();
        self.indices.clear();
        self.vertex_buffer.clear();
        self.index_buffer.clear();
    }

    /// Draw a line segment with given width.
    pub fn line(&mut self, a: Vec2, b: Vec2, color: Color, width: f32) {
        let dir = b - a;
        let len = dir.length();
        if len < 1e-6 {
            return;
        }
        let normal = Vec2::new(-dir.y / len, dir.x / len) * (width * 0.5);

        let base = self.vertices.len() as u32;
        self.vertices.push(ShapeVertex::new(a + normal, color));
        self.vertices.push(ShapeVertex::new(a - normal, color));
        self.vertices.push(ShapeVertex::new(b - normal, color));
        self.vertices.push(ShapeVertex::new(b + normal, color));

        self.indices.extend_from_slice(&[
            base, base + 1, base + 2,
            base, base + 2, base + 3,
        ]);
    }

    /// Draw a filled rectangle.
    pub fn rect(&mut self, pos: Vec2, size: Vec2, color: Color) {
        let base = self.vertices.len() as u32;
        let tl = pos;
        let tr = Vec2::new(pos.x + size.x, pos.y);
        let br = Vec2::new(pos.x + size.x, pos.y + size.y);
        let bl = Vec2::new(pos.x, pos.y + size.y);

        self.vertices.push(ShapeVertex::new(tl, color));
        self.vertices.push(ShapeVertex::new(tr, color));
        self.vertices.push(ShapeVertex::new(br, color));
        self.vertices.push(ShapeVertex::new(bl, color));

        self.indices.extend_from_slice(&[
            base, base + 1, base + 2,
            base, base + 2, base + 3,
        ]);
    }

    /// Draw a rectangle outline.
    pub fn rect_outline(&mut self, pos: Vec2, size: Vec2, color: Color, line_width: f32) {
        let tl = pos;
        let tr = Vec2::new(pos.x + size.x, pos.y);
        let br = Vec2::new(pos.x + size.x, pos.y + size.y);
        let bl = Vec2::new(pos.x, pos.y + size.y);

        self.line(tl, tr, color, line_width);
        self.line(tr, br, color, line_width);
        self.line(br, bl, color, line_width);
        self.line(bl, tl, color, line_width);
    }

    /// Draw a filled circle as a triangle fan.
    pub fn circle(&mut self, center: Vec2, radius: f32, color: Color, segments: u32) {
        let segments = segments.max(3);
        let base = self.vertices.len() as u32;

        // Center vertex
        self.vertices.push(ShapeVertex::new(center, color));

        let angle_step = std::f32::consts::TAU / segments as f32;
        for i in 0..segments {
            let angle = i as f32 * angle_step;
            let x = center.x + radius * angle.cos();
            let y = center.y + radius * angle.sin();
            self.vertices.push(ShapeVertex::new(Vec2::new(x, y), color));
        }

        // Triangle fan indices
        for i in 0..segments {
            let next = (i + 1) % segments;
            self.indices.extend_from_slice(&[
                base,           // center
                base + 1 + i,   // current edge vertex
                base + 1 + next, // next edge vertex
            ]);
        }
    }

    /// Draw a filled polygon (triangle fan from first vertex).
    pub fn polygon(&mut self, vertices: &[Vec2], color: Color) {
        if vertices.len() < 3 {
            return;
        }

        let base = self.vertices.len() as u32;
        for &v in vertices {
            self.vertices.push(ShapeVertex::new(v, color));
        }

        // Triangle fan from first vertex
        for i in 1..(vertices.len() as u32 - 1) {
            self.indices.extend_from_slice(&[base, base + i, base + i + 1]);
        }
    }

    /// Upload vertices/indices to GPU and return counts for rendering.
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> ShapePrepared {
        if self.vertices.is_empty() {
            return ShapePrepared {
                vertex_count: 0,
                index_count: 0,
            };
        }

        self.vertex_buffer.clear();
        self.index_buffer.clear();
        self.vertex_buffer.write(device, queue, &self.vertices);
        self.index_buffer.write(device, queue, &self.indices);

        ShapePrepared {
            vertex_count: self.vertices.len() as u32,
            index_count: self.indices.len() as u32,
        }
    }

    /// Issue draw call into a render pass.
    pub fn render<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        prepared: &ShapePrepared,
        pipeline: &'a wgpu::RenderPipeline,
        camera_bind_group: &'a wgpu::BindGroup,
    ) {
        if prepared.index_count == 0 {
            return;
        }

        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, Some(camera_bind_group), &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.buffer().slice(..));
        pass.set_index_buffer(
            self.index_buffer.buffer().slice(..),
            wgpu::IndexFormat::Uint32,
        );
        pass.draw_indexed(0..prepared.index_count, 0, 0..1);
    }

    pub fn vertex_buffer(&self) -> &wgpu::Buffer {
        self.vertex_buffer.buffer()
    }

    pub fn index_buffer(&self) -> &wgpu::Buffer {
        self.index_buffer.buffer()
    }

    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    pub fn index_count(&self) -> usize {
        self.indices.len()
    }

    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }
}

pub struct ShapePrepared {
    pub vertex_count: u32,
    pub index_count: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn rect_vertices_correct() {
        let (device, _queue) = test_ctx();
        let mut shapes = ShapeRenderer::new(&device);
        shapes.begin_frame();

        shapes.rect(Vec2::new(10.0, 20.0), Vec2::new(100.0, 50.0), Color::RED);

        assert_eq!(shapes.vertex_count(), 4);
        assert_eq!(shapes.index_count(), 6);

        // Check vertex positions
        let v = &shapes.vertices;
        assert_eq!(v[0].position, [10.0, 20.0]);   // top-left
        assert_eq!(v[1].position, [110.0, 20.0]);  // top-right
        assert_eq!(v[2].position, [110.0, 70.0]);  // bottom-right
        assert_eq!(v[3].position, [10.0, 70.0]);   // bottom-left
    }

    #[test]
    fn circle_triangle_count() {
        let (device, _queue) = test_ctx();
        let mut shapes = ShapeRenderer::new(&device);
        shapes.begin_frame();

        let segments = 16u32;
        shapes.circle(Vec2::ZERO, 50.0, Color::BLUE, segments);

        // Circle as triangle fan: segments triangles, center + segments edge vertices
        assert_eq!(shapes.vertex_count(), (1 + segments) as usize);
        assert_eq!(shapes.triangle_count(), segments as usize);
        assert_eq!(shapes.index_count(), (segments * 3) as usize);
    }

    #[test]
    fn circle_minimum_segments() {
        let (device, _queue) = test_ctx();
        let mut shapes = ShapeRenderer::new(&device);
        shapes.begin_frame();

        // Request 1 segment, should be clamped to 3
        shapes.circle(Vec2::ZERO, 10.0, Color::GREEN, 1);
        assert_eq!(shapes.triangle_count(), 3);
    }

    #[test]
    fn line_creates_quad() {
        let (device, _queue) = test_ctx();
        let mut shapes = ShapeRenderer::new(&device);
        shapes.begin_frame();

        shapes.line(Vec2::ZERO, Vec2::new(100.0, 0.0), Color::WHITE, 2.0);
        assert_eq!(shapes.vertex_count(), 4);
        assert_eq!(shapes.index_count(), 6);
    }

    #[test]
    fn polygon_fan() {
        let (device, _queue) = test_ctx();
        let mut shapes = ShapeRenderer::new(&device);
        shapes.begin_frame();

        let verts = [
            Vec2::new(0.0, 0.0),
            Vec2::new(100.0, 0.0),
            Vec2::new(100.0, 100.0),
            Vec2::new(50.0, 120.0),
            Vec2::new(0.0, 100.0),
        ];
        shapes.polygon(&verts, Color::RED);

        assert_eq!(shapes.vertex_count(), 5);
        assert_eq!(shapes.triangle_count(), 3); // n-2 triangles
    }

    #[test]
    fn shape_prepare_upload() {
        let (device, queue) = test_ctx();
        let mut shapes = ShapeRenderer::new(&device);
        shapes.begin_frame();

        shapes.rect(Vec2::ZERO, Vec2::new(10.0, 10.0), Color::WHITE);
        let prepared = shapes.prepare(&device, &queue);
        assert_eq!(prepared.vertex_count, 4);
        assert_eq!(prepared.index_count, 6);
    }
}
