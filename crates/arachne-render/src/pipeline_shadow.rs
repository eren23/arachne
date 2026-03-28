//! Shadow map depth-only render pipeline.
//!
//! Provides standalone functions for creating the shadow depth pass pipeline,
//! shadow map textures, light-space matrix computation, and shadow pass recording.

use crate::pipeline_3d::MeshDrawCall;
use crate::render3d::light::DirectionalLight;
use crate::render3d::mesh_render::{MeshInstance, MeshVertex};
use crate::render3d::shadow::ShadowUniform;
use crate::shaders;
use arachne_math::{Mat4, Vec3};

/// Default shadow map resolution (1024x1024).
pub const DEFAULT_SHADOW_RESOLUTION: u32 = 1024;

/// Create the shadow depth-only render pipeline.
///
/// Bind group layout 0: light-space matrix uniform (`ShadowUniform`, 80 bytes).
/// No color targets (depth-only pass).
/// Depth stencil: Depth32Float, depth write enabled, Less comparison.
/// Primitive: TriangleList, front face CCW, back-face culling.
pub fn create_shadow_pipeline(device: &wgpu::Device) -> wgpu::RenderPipeline {
    let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("shadow_depth_shader"),
        source: wgpu::ShaderSource::Wgsl(shaders::SHADOW.into()),
    });

    let light_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("shadow_light_bgl"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: wgpu::BufferSize::new(
                    std::mem::size_of::<ShadowUniform>() as u64,
                ),
            },
            count: None,
        }],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("shadow_depth_pipeline_layout"),
        bind_group_layouts: &[&light_bgl],
        push_constant_ranges: &[],
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("shadow_depth_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader_module,
            entry_point: Some("vs_main"),
            buffers: &[MeshVertex::LAYOUT, MeshInstance::LAYOUT],
            compilation_options: Default::default(),
        },
        fragment: None,
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState {
                constant: 2,
                slope_scale: 2.0,
                clamp: 0.0,
            },
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

/// Create a shadow map texture and view.
///
/// Format: Depth32Float. Usage: RENDER_ATTACHMENT | TEXTURE_BINDING.
/// Default resolution: [`DEFAULT_SHADOW_RESOLUTION`] (1024x1024).
pub fn create_shadow_map(
    device: &wgpu::Device,
    resolution: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("shadow_map_texture"),
        size: wgpu::Extent3d {
            width: resolution,
            height: resolution,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

/// Compute a light-space view-projection matrix for a directional light.
///
/// `scene_bounds` is the half-extent of the scene bounding box centered at the origin.
/// The orthographic projection encompasses `[-scene_bounds, scene_bounds]` in all axes.
pub fn compute_light_space_matrix(light: &DirectionalLight, scene_bounds: f32) -> Mat4 {
    let dir = light.direction.normalize();

    // Place the light camera looking from far away toward the origin
    let light_pos = -dir * scene_bounds;

    // Choose an up vector that isn't parallel to the light direction
    let up = if dir.cross(Vec3::Y).length_squared() > 1e-4 {
        Vec3::Y
    } else {
        Vec3::Z
    };

    let light_view = Mat4::look_at(light_pos, Vec3::ZERO, up);
    let light_proj = Mat4::orthographic(
        -scene_bounds,
        scene_bounds,
        -scene_bounds,
        scene_bounds,
        0.0,
        scene_bounds * 2.0,
    );

    light_proj * light_view
}

/// Record a shadow depth pass into a command encoder.
///
/// Clears the shadow map to depth 1.0 and renders all draw calls using
/// the shadow pipeline with only depth output.
pub fn render_shadow_pass(
    encoder: &mut wgpu::CommandEncoder,
    pipeline: &wgpu::RenderPipeline,
    shadow_view: &wgpu::TextureView,
    light_bg: &wgpu::BindGroup,
    draw_calls: &[MeshDrawCall],
    vertex_buffer: &wgpu::Buffer,
    index_buffer: &wgpu::Buffer,
    instance_buffer: &wgpu::Buffer,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("shadow_depth_pass"),
        color_attachments: &[],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: shadow_view,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(1.0),
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        }),
        ..Default::default()
    });

    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, Some(light_bg), &[]);
    pass.set_vertex_buffer(0, vertex_buffer.slice(..));
    pass.set_vertex_buffer(1, instance_buffer.slice(..));
    pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);

    for dc in draw_calls {
        pass.draw_indexed(
            dc.index_offset..dc.index_offset + dc.index_count,
            0,
            dc.instance_offset..dc.instance_offset + dc.instance_count,
        );
    }
}

/// Create a bind group for the shadow pipeline's light-space uniform.
///
/// Use `pipeline.get_bind_group_layout(0)` for the layout.
pub fn create_shadow_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    buffer: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("shadow_light_bg"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: buffer.as_entire_binding(),
        }],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::{IndexBuffer, UniformBuffer, VertexBuffer};
    use crate::render3d::mesh_render::{MeshInstance, MeshVertex};
    use crate::render3d::shadow::ShadowUniform;
    use arachne_math::{Color, Vec3};

    fn test_ctx() -> (wgpu::Device, wgpu::Queue) {
        pollster::block_on(async {
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::LowPower,
                    ..Default::default()
                })
                .await
                .expect("no GPU adapter");
            adapter
                .request_device(&wgpu::DeviceDescriptor::default(), None)
                .await
                .expect("device creation failed")
        })
    }

    #[test]
    fn shadow_pipeline_creation_succeeds() {
        let (device, _queue) = test_ctx();
        let pipeline = create_shadow_pipeline(&device);
        let _light_bgl = pipeline.get_bind_group_layout(0);
    }

    #[test]
    fn shadow_map_default_resolution() {
        let (device, _queue) = test_ctx();
        let (texture, _view) = create_shadow_map(&device, DEFAULT_SHADOW_RESOLUTION);
        assert_eq!(texture.width(), 1024);
        assert_eq!(texture.height(), 1024);
        assert_eq!(texture.format(), wgpu::TextureFormat::Depth32Float);
    }

    #[test]
    fn shadow_map_custom_resolution() {
        let (device, _queue) = test_ctx();
        let (texture, _view) = create_shadow_map(&device, 512);
        assert_eq!(texture.width(), 512);
        assert_eq!(texture.height(), 512);
        assert_eq!(texture.format(), wgpu::TextureFormat::Depth32Float);
    }

    #[test]
    fn light_space_matrix_directional_down() {
        let light = DirectionalLight::new(
            Vec3::new(0.0, -1.0, 0.0),
            Color::WHITE,
            1.0,
        );
        let bounds = 10.0;
        let vp = compute_light_space_matrix(&light, bounds);

        // The origin should project near the center of clip space
        let clip = vp.mul_vec4(arachne_math::Vec4::new(0.0, 0.0, 0.0, 1.0));
        let ndc_x = clip.x / clip.w;
        let ndc_y = clip.y / clip.w;
        assert!(
            ndc_x.abs() < 0.5,
            "origin x should be near center, got {ndc_x}"
        );
        assert!(
            ndc_y.abs() < 0.5,
            "origin y should be near center, got {ndc_y}"
        );

        // A point at the edge of bounds should still be within [-1, 1] NDC
        let edge = vp.mul_vec4(arachne_math::Vec4::new(bounds * 0.9, 0.0, 0.0, 1.0));
        let edge_x = edge.x / edge.w;
        assert!(
            edge_x.abs() <= 1.0,
            "edge point should be within NDC, got {edge_x}"
        );
    }

    #[test]
    fn shadow_bind_group_creation() {
        let (device, _queue) = test_ctx();
        let pipeline = create_shadow_pipeline(&device);
        let layout = pipeline.get_bind_group_layout(0);

        let uniform = ShadowUniform::default();
        let buffer = UniformBuffer::new(&device, "shadow_test", &uniform);
        let _bg = create_shadow_bind_group(&device, &layout, buffer.buffer());
    }

    #[test]
    fn shadow_pass_no_validation_errors() {
        let (device, queue) = test_ctx();
        let pipeline = create_shadow_pipeline(&device);
        let layout = pipeline.get_bind_group_layout(0);

        let (vertices, indices) = MeshVertex::cube();
        let vb = VertexBuffer::new(&device, &vertices);
        let ib = IndexBuffer::new_u32(&device, &indices);
        let instance_buf = VertexBuffer::new(&device, &[MeshInstance::identity()]);

        let (_shadow_tex, shadow_view) = create_shadow_map(&device, DEFAULT_SHADOW_RESOLUTION);

        let light = DirectionalLight::new(Vec3::new(0.0, -1.0, -1.0), Color::WHITE, 1.0);
        let light_vp = compute_light_space_matrix(&light, 10.0);
        let shadow_uniform = ShadowUniform::new(&light_vp, DEFAULT_SHADOW_RESOLUTION as f32);
        let shadow_buf = UniformBuffer::new(&device, "shadow", &shadow_uniform);
        let light_bg = create_shadow_bind_group(&device, &layout, shadow_buf.buffer());

        let draw_calls = vec![MeshDrawCall {
            index_offset: 0,
            index_count: ib.count(),
            instance_offset: 0,
            instance_count: 1,
        }];

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("shadow_test_encoder"),
        });

        render_shadow_pass(
            &mut encoder,
            &pipeline,
            &shadow_view,
            &light_bg,
            &draw_calls,
            vb.buffer(),
            ib.buffer(),
            instance_buf.buffer(),
        );

        queue.submit(std::iter::once(encoder.finish()));
        device.poll(wgpu::Maintain::Wait);
    }

    #[test]
    fn benchmark_shadow_pass_100_meshes() {
        let (device, queue) = test_ctx();
        let pipeline = create_shadow_pipeline(&device);
        let layout = pipeline.get_bind_group_layout(0);

        let (vertices, indices) = MeshVertex::cube();
        let vb = VertexBuffer::new(&device, &vertices);
        let ib = IndexBuffer::new_u32(&device, &indices);

        let instances: Vec<MeshInstance> = (0..100)
            .map(|i| {
                let x = (i % 10) as f32 * 2.0;
                let z = (i / 10) as f32 * 2.0;
                MeshInstance::from_translation(x, 0.0, z)
            })
            .collect();
        let instance_buf = VertexBuffer::new(&device, &instances);

        let (_shadow_tex, shadow_view) = create_shadow_map(&device, DEFAULT_SHADOW_RESOLUTION);

        let light = DirectionalLight::new(Vec3::new(0.0, -1.0, -1.0), Color::WHITE, 1.0);
        let light_vp = compute_light_space_matrix(&light, 20.0);
        let shadow_uniform = ShadowUniform::new(&light_vp, DEFAULT_SHADOW_RESOLUTION as f32);
        let shadow_buf = UniformBuffer::new(&device, "shadow", &shadow_uniform);
        let light_bg = create_shadow_bind_group(&device, &layout, shadow_buf.buffer());

        let draw_calls = vec![MeshDrawCall {
            index_offset: 0,
            index_count: ib.count(),
            instance_offset: 0,
            instance_count: 100,
        }];

        // Warm up: submit once so pipeline compilation is not measured
        {
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("shadow_warmup"),
            });
            render_shadow_pass(
                &mut enc,
                &pipeline,
                &shadow_view,
                &light_bg,
                &draw_calls,
                vb.buffer(),
                ib.buffer(),
                instance_buf.buffer(),
            );
            queue.submit(std::iter::once(enc.finish()));
            device.poll(wgpu::Maintain::Wait);
        }

        let start = std::time::Instant::now();

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("shadow_bench"),
        });

        render_shadow_pass(
            &mut encoder,
            &pipeline,
            &shadow_view,
            &light_bg,
            &draw_calls,
            vb.buffer(),
            ib.buffer(),
            instance_buf.buffer(),
        );

        queue.submit(std::iter::once(encoder.finish()));
        device.poll(wgpu::Maintain::Wait);

        let elapsed = start.elapsed();
        eprintln!(
            "Shadow pass 100 meshes: {:.2}ms",
            elapsed.as_secs_f64() * 1000.0,
        );
        assert!(
            elapsed.as_millis() < 8,
            "shadow pass 100 meshes took {}ms, expected < 8ms",
            elapsed.as_millis(),
        );
    }
}
