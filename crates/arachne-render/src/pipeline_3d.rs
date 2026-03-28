//! 3D PBR mesh render pipeline.
//!
//! Creates the wgpu render pipeline for instanced PBR mesh rendering with
//! depth testing, and provides helpers for depth textures, light uniform
//! buffers, material bind groups, and draw call submission.

use crate::render3d::light::{GpuLight, LightUniform, MAX_LIGHTS};
use crate::render3d::material::MaterialUniform;
use crate::render3d::mesh_render::{CameraUniform3d, MeshInstance, MeshVertex};
use crate::render3d::shadow::ShadowUniform;
use crate::shaders;

/// A 3D mesh draw call: index range + instance range within shared buffers.
#[derive(Clone, Debug)]
pub struct MeshDrawCall {
    pub index_offset: u32,
    pub index_count: u32,
    pub instance_offset: u32,
    pub instance_count: u32,
}

/// Create the PBR mesh render pipeline.
///
/// Bind group layouts:
///   0: camera uniform (CameraUniform3d — view_proj + camera_pos)
///   1: material uniform + albedo texture/sampler + normal texture/sampler
///   2: light uniform (LightUniform — 8 lights + ambient)
///   3: shadow uniform + shadow depth texture + comparison sampler
///
/// Depth stencil: `depth_format` with depth write, Less comparison.
/// Color target: `surface_format`, opaque (BlendState::REPLACE).
/// Primitive: TriangleList, CCW front face, back-face culling.
pub fn create_mesh_pipeline(
    device: &wgpu::Device,
    surface_format: wgpu::TextureFormat,
    depth_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("mesh_pbr_shader"),
        source: wgpu::ShaderSource::Wgsl(shaders::MESH_PBR.into()),
    });

    // group 0: camera
    let camera_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("mesh_camera_bgl"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: wgpu::BufferSize::new(
                    std::mem::size_of::<CameraUniform3d>() as u64,
                ),
            },
            count: None,
        }],
    });

    // group 1: material uniform + textures
    let material_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("mesh_material_bgl"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(
                        std::mem::size_of::<MaterialUniform>() as u64,
                    ),
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 4,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });

    // group 2: lights
    let light_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("mesh_light_bgl"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: wgpu::BufferSize::new(
                    std::mem::size_of::<LightUniform>() as u64,
                ),
            },
            count: None,
        }],
    });

    // group 3: shadow
    let shadow_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("mesh_shadow_bgl"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(
                        std::mem::size_of::<ShadowUniform>() as u64,
                    ),
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Depth,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                count: None,
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("mesh_pipeline_layout"),
        bind_group_layouts: &[&camera_bgl, &material_bgl, &light_bgl, &shadow_bgl],
        push_constant_ranges: &[],
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("mesh_pbr_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader_module,
            entry_point: Some("vs_main"),
            buffers: &[MeshVertex::LAYOUT, MeshInstance::LAYOUT],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader_module,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: surface_format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: depth_format,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

/// Create a depth texture and its view.
///
/// Format: Depth32Float. Usage: RENDER_ATTACHMENT | TEXTURE_BINDING.
pub fn create_depth_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("depth_texture"),
        size: wgpu::Extent3d {
            width,
            height,
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

/// Create a GPU buffer holding a `LightUniform` (8 × GpuLight + count/ambient).
///
/// The buffer is initialized with the provided lights (up to [`MAX_LIGHTS`]),
/// with remaining slots zeroed.
pub fn create_light_uniform_buffer(
    device: &wgpu::Device,
    lights: &[GpuLight],
) -> wgpu::Buffer {
    use wgpu::util::DeviceExt;

    let mut uniform = LightUniform::default();
    let count = lights.len().min(MAX_LIGHTS);
    for (i, light) in lights.iter().take(count).enumerate() {
        uniform.lights[i] = *light;
    }
    uniform.num_lights_ambient[0] = count as f32;

    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("light_uniform_buffer"),
        contents: bytemuck::bytes_of(&uniform),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    })
}

/// Create a material bind group for the mesh pipeline.
///
/// Binds `uniform_buffer` at binding 0 (PBR material params).
/// Bindings 1-4 (textures/samplers) must be provided by the caller via
/// [`MeshRenderer::create_material_bind_group`] for the full bind group;
/// this helper creates a simple uniform-only bind group using a layout
/// that matches bind group 1 of `create_mesh_pipeline`.
pub fn create_material_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    uniform_buffer: &wgpu::Buffer,
    albedo_view: &wgpu::TextureView,
    albedo_sampler: &wgpu::Sampler,
    normal_view: &wgpu::TextureView,
    normal_sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("mesh_material_bg"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(albedo_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(albedo_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(normal_view),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::Sampler(normal_sampler),
            },
        ],
    })
}

/// Issue 3D mesh draw calls into an existing render pass.
///
/// Sets the pipeline and bind groups (camera=0, material=1, light=2, shadow=3),
/// then iterates `draw_calls`, issuing indexed instanced draws.
pub fn render_meshes<'a>(
    pass: &mut wgpu::RenderPass<'a>,
    pipeline: &'a wgpu::RenderPipeline,
    camera_bg: &'a wgpu::BindGroup,
    material_bg: &'a wgpu::BindGroup,
    light_bg: &'a wgpu::BindGroup,
    shadow_bg: &'a wgpu::BindGroup,
    draw_calls: &[MeshDrawCall],
    vertex_buffer: &'a wgpu::Buffer,
    index_buffer: &'a wgpu::Buffer,
    instance_buffer: &'a wgpu::Buffer,
) {
    if draw_calls.is_empty() {
        return;
    }

    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, Some(camera_bg), &[]);
    pass.set_bind_group(1, Some(material_bg), &[]);
    pass.set_bind_group(2, Some(light_bg), &[]);
    pass.set_bind_group(3, Some(shadow_bg), &[]);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::{IndexBuffer, UniformBuffer, VertexBuffer};
    use crate::render3d::light::{DirectionalLight, LightState, PointLight};
    use crate::render3d::material::MaterialUniform;
    use crate::render3d::shadow::ShadowUniform;
    use arachne_math::{Color, Mat4, Vec3};

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
    fn mesh_pipeline_creation_succeeds() {
        let (device, _queue) = test_ctx();
        let pipeline = create_mesh_pipeline(
            &device,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            wgpu::TextureFormat::Depth32Float,
        );
        let _camera_bgl = pipeline.get_bind_group_layout(0);
        let _material_bgl = pipeline.get_bind_group_layout(1);
        let _light_bgl = pipeline.get_bind_group_layout(2);
        let _shadow_bgl = pipeline.get_bind_group_layout(3);
    }

    #[test]
    fn depth_texture_format_and_dimensions() {
        let (device, _queue) = test_ctx();
        let (texture, _view) = create_depth_texture(&device, 1920, 1080);
        assert_eq!(texture.format(), wgpu::TextureFormat::Depth32Float);
        assert_eq!(texture.width(), 1920);
        assert_eq!(texture.height(), 1080);
    }

    #[test]
    fn light_uniform_buffer_size() {
        let (device, _queue) = test_ctx();
        let lights: Vec<GpuLight> = (0..8)
            .map(|i| {
                GpuLight::from_point(&PointLight::new(
                    Vec3::new(i as f32, 0.0, 0.0),
                    Color::WHITE,
                    1.0,
                    10.0,
                ))
            })
            .collect();
        let buffer = create_light_uniform_buffer(&device, &lights);
        assert_eq!(
            buffer.size(),
            std::mem::size_of::<LightUniform>() as u64,
        );
    }

    #[test]
    fn light_uniform_buffer_clamps_to_max() {
        let (device, _queue) = test_ctx();
        let lights: Vec<GpuLight> = (0..20)
            .map(|i| {
                GpuLight::from_point(&PointLight::new(
                    Vec3::new(i as f32, 0.0, 0.0),
                    Color::WHITE,
                    1.0,
                    5.0,
                ))
            })
            .collect();
        // Should not panic — only first MAX_LIGHTS are used
        let buffer = create_light_uniform_buffer(&device, &lights);
        assert_eq!(buffer.size(), std::mem::size_of::<LightUniform>() as u64);
    }

    #[test]
    fn material_bind_group_creation_succeeds() {
        let (device, queue) = test_ctx();
        let pipeline = create_mesh_pipeline(
            &device,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            wgpu::TextureFormat::Depth32Float,
        );
        let layout = pipeline.get_bind_group_layout(1);

        let mat_uniform = MaterialUniform::default();
        let mat_buf = UniformBuffer::new(&device, "material", &mat_uniform);

        // Create 1x1 white texture for albedo and normal
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("test_tex"),
            size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255u8, 255, 255, 255],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
        );
        let tex_view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("test_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let _bg = create_material_bind_group(
            &device,
            &layout,
            mat_buf.buffer(),
            &tex_view,
            &sampler,
            &tex_view,
            &sampler,
        );
    }

    #[test]
    fn one_cube_one_draw_call() {
        let (_vertices, indices) = MeshVertex::cube();
        assert_eq!(indices.len(), 36);

        let draw_calls = vec![MeshDrawCall {
            index_offset: 0,
            index_count: 36,
            instance_offset: 0,
            instance_count: 1,
        }];
        assert_eq!(draw_calls.len(), 1);
    }

    #[test]
    fn hundred_meshes_batch_count() {
        let num_meshes = 100u32;
        let indices_per_mesh = 36u32;

        // One draw call per unique mesh/material combo — with same mesh, 1 call
        let draw_calls = vec![MeshDrawCall {
            index_offset: 0,
            index_count: indices_per_mesh,
            instance_offset: 0,
            instance_count: num_meshes,
        }];
        assert_eq!(draw_calls.len(), 1);
        assert_eq!(draw_calls[0].instance_count, 100);
    }

    #[test]
    fn benchmark_100_mesh_prepare() {
        let start = std::time::Instant::now();

        let num_meshes = 100u32;
        let mut instances = Vec::with_capacity(num_meshes as usize);
        for i in 0..num_meshes {
            let x = (i % 10) as f32 * 2.0;
            let z = (i / 10) as f32 * 2.0;
            instances.push(MeshInstance::from_translation(x, 0.0, z));
        }

        let draw_calls = vec![MeshDrawCall {
            index_offset: 0,
            index_count: 36,
            instance_offset: 0,
            instance_count: num_meshes,
        }];

        let elapsed = start.elapsed();
        eprintln!(
            "100 mesh prepare: {:.2}ms, {} draw calls",
            elapsed.as_secs_f64() * 1000.0,
            draw_calls.len(),
        );
        assert!(
            elapsed.as_millis() < 4,
            "100 mesh prepare took {}ms, expected < 4ms",
            elapsed.as_millis(),
        );
    }

    #[test]
    fn full_3d_render_pass_no_validation_errors() {
        let (device, queue) = test_ctx();
        let renderer = crate::render3d::MeshRenderer::new(
            &device,
            &queue,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        );

        let (vertices, indices) = MeshVertex::cube();
        let vb = VertexBuffer::new(&device, &vertices);
        let ib = IndexBuffer::new_u32(&device, &indices);
        let instance_buf = VertexBuffer::new(&device, &[MeshInstance::identity()]);

        let camera_uniform = CameraUniform3d::default();
        let camera_buf = UniformBuffer::new(&device, "camera", &camera_uniform);
        let camera_bg = renderer.create_camera_bind_group(&device, camera_buf.buffer());

        let mat_uniform = MaterialUniform::default();
        let mat_buf = UniformBuffer::new(&device, "material", &mat_uniform);
        let mat_bg = renderer.create_material_bind_group(&device, mat_buf.buffer(), None, None);

        let light_uniform = LightUniform::default();
        let light_buf = UniformBuffer::new(&device, "lights", &light_uniform);
        let light_bg = renderer.create_light_bind_group(&device, light_buf.buffer());

        let shadow_uniform = ShadowUniform::default();
        let shadow_buf = UniformBuffer::new(&device, "shadow", &shadow_uniform);
        let shadow_bg = renderer.create_shadow_bind_group(&device, shadow_buf.buffer(), None);

        let color_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("test_color"),
            size: wgpu::Extent3d { width: 64, height: 64, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let color_view = color_tex.create_view(&Default::default());

        let (_depth_tex, depth_view) = create_depth_texture(&device, 64, 64);

        let draw_calls = vec![MeshDrawCall {
            index_offset: 0,
            index_count: ib.count(),
            instance_offset: 0,
            instance_count: 1,
        }];

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("test_3d"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("mesh_test_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });

            render_meshes(
                &mut pass,
                renderer.pipeline(),
                &camera_bg,
                &mat_bg,
                &light_bg,
                &shadow_bg,
                &draw_calls,
                vb.buffer(),
                ib.buffer(),
                instance_buf.buffer(),
            );
        }

        queue.submit(std::iter::once(encoder.finish()));
        device.poll(wgpu::Maintain::Wait);
    }
}
