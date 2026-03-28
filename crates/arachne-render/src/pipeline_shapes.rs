//! Shape render pipeline for lines, rectangles, circles, and polygons.
//!
//! Creates a wgpu render pipeline for per-vertex colored geometry and provides
//! a standalone `render_shapes` function to issue indexed draw calls from
//! [`ShapeRenderer`](crate::render2d::shape::ShapeRenderer) prepared data.

use crate::render2d::shape::ShapeVertex;
use crate::shaders;

/// Create the shape render pipeline.
///
/// Bind group layout 0: camera uniform buffer (mat4x4<f32>, 64 bytes, vertex+fragment).
/// No texture bind group — colour comes from per-vertex attributes.
/// Alpha blend: SrcAlpha / OneMinusSrcAlpha.  Primitive: TriangleList, no culling.
pub fn create_shape_pipeline(
    device: &wgpu::Device,
    surface_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("shape_shader"),
        source: wgpu::ShaderSource::Wgsl(shaders::SHAPE.into()),
    });

    let camera_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("shape_camera_bgl"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("shape_pipeline_layout"),
        bind_group_layouts: &[&camera_bgl],
        push_constant_ranges: &[],
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("shape_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader_module,
            entry_point: Some("vs_main"),
            buffers: &[ShapeVertex::LAYOUT],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader_module,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: surface_format,
                blend: Some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::SrcAlpha,
                        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                        operation: wgpu::BlendOperation::Add,
                    },
                }),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None, // shapes may be wound either way
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

/// Issue indexed draw calls for shape geometry.
///
/// `draw_calls` come from [`Batcher::sort_and_merge`](crate::render2d::batch::Batcher).
/// For the simple single-batch path produced by [`ShapeRenderer::render`], pass a
/// single-element slice.
pub fn render_shapes<'a>(
    pass: &mut wgpu::RenderPass<'a>,
    pipeline: &'a wgpu::RenderPipeline,
    camera_bg: &'a wgpu::BindGroup,
    vertex_buffer: &'a wgpu::Buffer,
    index_buffer: &'a wgpu::Buffer,
    index_count: u32,
) {
    if index_count == 0 {
        return;
    }

    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, Some(camera_bg), &[]);
    pass.set_vertex_buffer(0, vertex_buffer.slice(..));
    pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
    pass.draw_indexed(0..index_count, 0, 0..1);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render2d::shape::ShapeRenderer;
    use arachne_math::{Color, Vec2};

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
    fn shape_pipeline_creation_succeeds() {
        let (device, _queue) = test_ctx();
        let pipeline = create_shape_pipeline(&device, wgpu::TextureFormat::Rgba8UnormSrgb);
        // Camera bind group layout is at index 0
        let _camera_bgl = pipeline.get_bind_group_layout(0);
    }

    #[test]
    fn shape_pipeline_camera_bind_group() {
        let (device, _queue) = test_ctx();
        let pipeline = create_shape_pipeline(&device, wgpu::TextureFormat::Rgba8UnormSrgb);
        let layout = pipeline.get_bind_group_layout(0);
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera_uniform"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // Should succeed — layout is compatible with a 64-byte uniform buffer
        let _bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shape_camera_bg"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });
    }

    #[test]
    fn prepare_100_mixed_shapes_nonzero_draw() {
        let (device, queue) = test_ctx();
        let mut shapes = ShapeRenderer::new(&device);
        shapes.begin_frame();

        // Mix of rects, circles, and lines
        for i in 0..34 {
            shapes.rect(
                Vec2::new(i as f32 * 10.0, 0.0),
                Vec2::new(8.0, 8.0),
                Color::RED,
            );
        }
        for i in 0..33 {
            shapes.circle(Vec2::new(i as f32 * 10.0, 50.0), 5.0, Color::BLUE, 12);
        }
        for i in 0..33 {
            shapes.line(
                Vec2::new(i as f32 * 10.0, 100.0),
                Vec2::new(i as f32 * 10.0 + 8.0, 100.0),
                Color::GREEN,
                2.0,
            );
        }

        let prepared = shapes.prepare(&device, &queue);
        assert!(
            prepared.index_count > 0,
            "100 mixed shapes should produce non-zero index count"
        );
        assert!(
            prepared.vertex_count > 0,
            "100 mixed shapes should produce non-zero vertex count"
        );
    }

    #[test]
    fn zero_shapes_graceful_noop() {
        let (device, queue) = test_ctx();
        let mut shapes = ShapeRenderer::new(&device);
        shapes.begin_frame();

        let prepared = shapes.prepare(&device, &queue);
        assert_eq!(prepared.vertex_count, 0);
        assert_eq!(prepared.index_count, 0);
    }

    #[test]
    fn prepare_1000_shapes_benchmark() {
        let (device, queue) = test_ctx();
        let mut shapes = ShapeRenderer::new(&device);

        let start = std::time::Instant::now();
        shapes.begin_frame();
        for i in 0..400 {
            shapes.rect(
                Vec2::new(i as f32 * 2.0, 0.0),
                Vec2::new(1.0, 1.0),
                Color::RED,
            );
        }
        for i in 0..300 {
            shapes.circle(Vec2::new(i as f32 * 3.0, 50.0), 4.0, Color::BLUE, 12);
        }
        for i in 0..300 {
            shapes.line(
                Vec2::new(i as f32 * 3.0, 100.0),
                Vec2::new(i as f32 * 3.0 + 2.0, 108.0),
                Color::GREEN,
                1.0,
            );
        }
        let _prepared = shapes.prepare(&device, &queue);
        let elapsed = start.elapsed();

        eprintln!(
            "1,000 shapes prepare: {:.2}ms",
            elapsed.as_secs_f64() * 1000.0,
        );
        assert!(
            elapsed.as_millis() < 1,
            "prepare 1000 shapes took {}ms, expected < 1ms",
            elapsed.as_millis()
        );
    }
}
