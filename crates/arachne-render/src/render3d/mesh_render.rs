use arachne_math::{Mat4, Vec3};
use crate::pipeline::create_shader_module;
use super::material::MaterialUniform;
use super::light::LightUniform;
use super::shadow::ShadowUniform;

// ---------------------------------------------------------------------------
// Vertex format: position(3) + normal(3) + texcoord(2) + tangent(4) = 48 bytes
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MeshVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub texcoord: [f32; 2],
    pub tangent: [f32; 4],
}

impl MeshVertex {
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: 48,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            // location 0: position
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: 0,
                shader_location: 0,
            },
            // location 1: normal
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: 12,
                shader_location: 1,
            },
            // location 2: texcoord
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 24,
                shader_location: 2,
            },
            // location 3: tangent
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 32,
                shader_location: 3,
            },
        ],
    };

    /// Generate a unit cube: 24 vertices, 36 indices (12 triangles).
    pub fn cube() -> (Vec<MeshVertex>, Vec<u32>) {
        let mut vertices = Vec::with_capacity(24);
        let mut indices = Vec::with_capacity(36);

        // Face data: (normal, tangent, 4 corner positions)
        let faces: [([f32; 3], [f32; 4], [[f32; 3]; 4]); 6] = [
            // +Z (front)
            ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0, 1.0], [
                [-0.5, -0.5,  0.5],
                [ 0.5, -0.5,  0.5],
                [ 0.5,  0.5,  0.5],
                [-0.5,  0.5,  0.5],
            ]),
            // -Z (back)
            ([0.0, 0.0, -1.0], [-1.0, 0.0, 0.0, 1.0], [
                [ 0.5, -0.5, -0.5],
                [-0.5, -0.5, -0.5],
                [-0.5,  0.5, -0.5],
                [ 0.5,  0.5, -0.5],
            ]),
            // +X (right)
            ([1.0, 0.0, 0.0], [0.0, 0.0, -1.0, 1.0], [
                [0.5, -0.5,  0.5],
                [0.5, -0.5, -0.5],
                [0.5,  0.5, -0.5],
                [0.5,  0.5,  0.5],
            ]),
            // -X (left)
            ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 1.0], [
                [-0.5, -0.5, -0.5],
                [-0.5, -0.5,  0.5],
                [-0.5,  0.5,  0.5],
                [-0.5,  0.5, -0.5],
            ]),
            // +Y (top)
            ([0.0, 1.0, 0.0], [1.0, 0.0, 0.0, 1.0], [
                [-0.5, 0.5,  0.5],
                [ 0.5, 0.5,  0.5],
                [ 0.5, 0.5, -0.5],
                [-0.5, 0.5, -0.5],
            ]),
            // -Y (bottom)
            ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0, 1.0], [
                [-0.5, -0.5, -0.5],
                [ 0.5, -0.5, -0.5],
                [ 0.5, -0.5,  0.5],
                [-0.5, -0.5,  0.5],
            ]),
        ];

        let uvs = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];

        for (normal, tangent, corners) in &faces {
            let base = vertices.len() as u32;
            for (i, pos) in corners.iter().enumerate() {
                vertices.push(MeshVertex {
                    position: *pos,
                    normal: *normal,
                    texcoord: uvs[i],
                    tangent: *tangent,
                });
            }
            indices.extend_from_slice(&[
                base, base + 1, base + 2,
                base, base + 2, base + 3,
            ]);
        }

        (vertices, indices)
    }
}

// ---------------------------------------------------------------------------
// Instance data: model matrix as 4 column vectors (64 bytes)
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MeshInstance {
    pub model: [[f32; 4]; 4],
}

impl MeshInstance {
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: 64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &[
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 0,
                shader_location: 4,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 16,
                shader_location: 5,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 32,
                shader_location: 6,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 48,
                shader_location: 7,
            },
        ],
    };

    pub fn from_mat4(m: &Mat4) -> Self {
        Self { model: m.cols }
    }

    pub fn from_translation(x: f32, y: f32, z: f32) -> Self {
        Self::from_mat4(&Mat4::from_translation(Vec3::new(x, y, z)))
    }

    pub fn identity() -> Self {
        Self { model: Mat4::IDENTITY.cols }
    }
}

// ---------------------------------------------------------------------------
// Extended camera uniform (view_proj + camera_pos)
// ---------------------------------------------------------------------------

/// Matches `CameraUniforms` in mesh_pbr.wgsl (80 bytes).
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform3d {
    pub view_proj: [[f32; 4]; 4],
    pub camera_pos: [f32; 4],
}

impl Default for CameraUniform3d {
    fn default() -> Self {
        Self {
            view_proj: Mat4::IDENTITY.cols,
            camera_pos: [0.0, 0.0, 5.0, 0.0],
        }
    }
}

impl CameraUniform3d {
    pub fn new(view_proj: &Mat4, position: Vec3) -> Self {
        Self {
            view_proj: view_proj.cols,
            camera_pos: [position.x, position.y, position.z, 0.0],
        }
    }
}

// ---------------------------------------------------------------------------
// MeshRenderer: owns PBR pipeline, bind group layouts, and default resources
// ---------------------------------------------------------------------------

pub struct MeshRenderer {
    pbr_pipeline: wgpu::RenderPipeline,
    shadow_pipeline: wgpu::RenderPipeline,
    camera_bgl: wgpu::BindGroupLayout,
    material_bgl: wgpu::BindGroupLayout,
    light_bgl: wgpu::BindGroupLayout,
    shadow_bgl: wgpu::BindGroupLayout,
    shadow_only_bgl: wgpu::BindGroupLayout,
    default_sampler: wgpu::Sampler,
    comparison_sampler: wgpu::Sampler,
    _default_white: wgpu::Texture,
    default_white_view: wgpu::TextureView,
    _default_normal: wgpu::Texture,
    default_normal_view: wgpu::TextureView,
    _default_shadow: wgpu::Texture,
    default_shadow_view: wgpu::TextureView,
}

impl MeshRenderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
    ) -> Self {
        // --- Default textures ------------------------------------------------
        let (default_white, default_white_view) =
            Self::create_1x1_texture(device, queue, [255, 255, 255, 255], "default_white");
        let (default_normal, default_normal_view) =
            Self::create_1x1_texture(device, queue, [128, 128, 255, 255], "default_normal");
        let (default_shadow, default_shadow_view) =
            Self::create_1x1_depth(device, "default_shadow");

        // Clear the default shadow texture to 1.0
        {
            let view = default_shadow.create_view(&wgpu::TextureViewDescriptor::default());
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("clear_default_shadow"),
            });
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear_default_shadow"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            drop(_pass);
            queue.submit(std::iter::once(encoder.finish()));
        }

        let default_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("default_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let comparison_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("comparison_sampler"),
            compare: Some(wgpu::CompareFunction::LessEqual),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // --- Bind group layouts -----------------------------------------------
        let camera_bgl = Self::create_camera_bgl(device);
        let material_bgl = Self::create_material_bgl(device);
        let light_bgl = Self::create_light_bgl(device);
        let shadow_bgl = Self::create_shadow_bgl(device);
        let shadow_only_bgl = Self::create_shadow_only_bgl(device);

        // --- PBR pipeline -----------------------------------------------------
        let pbr_shader = create_shader_module(
            device,
            "mesh_pbr",
            include_str!("../shaders/mesh_pbr.wgsl"),
        );

        let pbr_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pbr_pipeline_layout"),
            bind_group_layouts: &[&camera_bgl, &material_bgl, &light_bgl, &shadow_bgl],
            push_constant_ranges: &[],
        });

        let pbr_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("pbr_pipeline"),
            layout: Some(&pbr_layout),
            vertex: wgpu::VertexState {
                module: &pbr_shader,
                entry_point: Some("vs_main"),
                buffers: &[MeshVertex::LAYOUT, MeshInstance::LAYOUT],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &pbr_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
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
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // --- Shadow pipeline --------------------------------------------------
        let shadow_shader = create_shader_module(
            device,
            "shadow",
            include_str!("../shaders/shadow.wgsl"),
        );

        let shadow_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("shadow_pipeline_layout"),
            bind_group_layouts: &[&shadow_only_bgl],
            push_constant_ranges: &[],
        });

        let shadow_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shadow_pipeline"),
            layout: Some(&shadow_layout),
            vertex: wgpu::VertexState {
                module: &shadow_shader,
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
        });

        Self {
            pbr_pipeline,
            shadow_pipeline,
            camera_bgl,
            material_bgl,
            light_bgl,
            shadow_bgl,
            shadow_only_bgl,
            default_sampler,
            comparison_sampler,
            _default_white: default_white,
            default_white_view,
            _default_normal: default_normal,
            default_normal_view,
            _default_shadow: default_shadow,
            default_shadow_view,
        }
    }

    // --- Accessors -----------------------------------------------------------

    pub fn pipeline(&self) -> &wgpu::RenderPipeline {
        &self.pbr_pipeline
    }

    pub fn shadow_pipeline(&self) -> &wgpu::RenderPipeline {
        &self.shadow_pipeline
    }

    pub fn camera_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.camera_bgl
    }

    pub fn material_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.material_bgl
    }

    pub fn light_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.light_bgl
    }

    pub fn shadow_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.shadow_bgl
    }

    pub fn shadow_only_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.shadow_only_bgl
    }

    // --- Bind group factories ------------------------------------------------

    pub fn create_camera_bind_group(
        &self,
        device: &wgpu::Device,
        buffer: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera_bg"),
            layout: &self.camera_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        })
    }

    pub fn create_material_bind_group(
        &self,
        device: &wgpu::Device,
        uniform_buffer: &wgpu::Buffer,
        albedo_view: Option<&wgpu::TextureView>,
        normal_view: Option<&wgpu::TextureView>,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("material_bg"),
            layout: &self.material_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        albedo_view.unwrap_or(&self.default_white_view),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.default_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(
                        normal_view.unwrap_or(&self.default_normal_view),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&self.default_sampler),
                },
            ],
        })
    }

    pub fn create_light_bind_group(
        &self,
        device: &wgpu::Device,
        buffer: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("light_bg"),
            layout: &self.light_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        })
    }

    pub fn create_shadow_bind_group(
        &self,
        device: &wgpu::Device,
        uniform_buffer: &wgpu::Buffer,
        shadow_depth_view: Option<&wgpu::TextureView>,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shadow_bg"),
            layout: &self.shadow_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        shadow_depth_view.unwrap_or(&self.default_shadow_view),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.comparison_sampler),
                },
            ],
        })
    }

    pub fn create_shadow_only_bind_group(
        &self,
        device: &wgpu::Device,
        buffer: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shadow_only_bg"),
            layout: &self.shadow_only_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        })
    }

    // --- Bind group layout helpers -------------------------------------------

    fn create_camera_bgl(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("camera_bgl"),
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
        })
    }

    fn create_material_bgl(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("material_bgl"),
            entries: &[
                // binding 0: material uniform
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
                // binding 1: albedo texture
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
                // binding 2: albedo sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // binding 3: normal map texture
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
                // binding 4: normal sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        })
    }

    fn create_light_bgl(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("light_bgl"),
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
        })
    }

    fn create_shadow_bgl(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shadow_bgl"),
            entries: &[
                // binding 0: shadow uniform
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
                // binding 1: shadow depth texture
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
                // binding 2: comparison sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
            ],
        })
    }

    fn create_shadow_only_bgl(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shadow_only_bgl"),
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
        })
    }

    // --- Texture helpers -----------------------------------------------------

    fn create_1x1_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color: [u8; 4],
        label: &str,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &color,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    fn create_1x1_depth(
        device: &wgpu::Device,
        label: &str,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::{UniformBuffer, VertexBuffer, IndexBuffer};

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

    fn create_test_color_target(
        device: &wgpu::Device,
        w: u32,
        h: u32,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("test_color"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = tex.create_view(&Default::default());
        (tex, view)
    }

    fn create_test_depth_target(
        device: &wgpu::Device,
        w: u32,
        h: u32,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("test_depth"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = tex.create_view(&Default::default());
        (tex, view)
    }

    #[test]
    fn mesh_vertex_size() {
        assert_eq!(std::mem::size_of::<MeshVertex>(), 48);
    }

    #[test]
    fn mesh_instance_size() {
        assert_eq!(std::mem::size_of::<MeshInstance>(), 64);
    }

    #[test]
    fn camera_uniform_3d_size() {
        assert_eq!(std::mem::size_of::<CameraUniform3d>(), 80);
    }

    #[test]
    fn cube_mesh_data() {
        let (vertices, indices) = MeshVertex::cube();
        assert_eq!(vertices.len(), 24);
        assert_eq!(indices.len(), 36);
        // 12 triangles
        assert_eq!(indices.len() / 3, 12);
        // All indices in range
        for &idx in &indices {
            assert!(idx < vertices.len() as u32, "index {idx} out of range");
        }
    }

    #[test]
    fn mesh_cube_render_no_validation_errors() {
        let (device, queue) = test_ctx();
        let renderer = MeshRenderer::new(&device, &queue, wgpu::TextureFormat::Rgba8UnormSrgb);

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

        let (_color_tex, color_view) = create_test_color_target(&device, 64, 64);
        let (_depth_tex, depth_view) = create_test_depth_target(&device, 64, 64);

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("test_encoder"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("pbr_test"),
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

            pass.set_pipeline(renderer.pipeline());
            pass.set_bind_group(0, &camera_bg, &[]);
            pass.set_bind_group(1, &mat_bg, &[]);
            pass.set_bind_group(2, &light_bg, &[]);
            pass.set_bind_group(3, &shadow_bg, &[]);
            pass.set_vertex_buffer(0, vb.slice());
            pass.set_vertex_buffer(1, instance_buf.slice());
            pass.set_index_buffer(ib.slice(), ib.format());
            pass.draw_indexed(0..ib.count(), 0, 0..1);
        }

        queue.submit(std::iter::once(encoder.finish()));
        device.poll(wgpu::Maintain::Wait);
        // Reaching here without panic = zero validation errors.
    }

    #[test]
    fn benchmark_1000_mesh_instances() {
        let (device, queue) = test_ctx();
        let renderer = MeshRenderer::new(&device, &queue, wgpu::TextureFormat::Rgba8UnormSrgb);

        let (vertices, indices) = MeshVertex::cube();
        let vb = VertexBuffer::new(&device, &vertices);
        let ib = IndexBuffer::new_u32(&device, &indices);

        let instances: Vec<MeshInstance> = (0..1000)
            .map(|i| {
                let x = (i % 32) as f32 * 2.0 - 32.0;
                let y = ((i / 32) % 32) as f32 * 2.0 - 32.0;
                let z = (i / 1024) as f32 * 2.0;
                MeshInstance::from_translation(x, y, z)
            })
            .collect();
        let instance_buf = VertexBuffer::new(&device, &instances);

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

        let (_color_tex, color_view) = create_test_color_target(&device, 256, 256);
        let (_depth_tex, depth_view) = create_test_depth_target(&device, 256, 256);

        let start = std::time::Instant::now();

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("bench_encoder"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("bench_pass"),
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

            pass.set_pipeline(renderer.pipeline());
            pass.set_bind_group(0, &camera_bg, &[]);
            pass.set_bind_group(1, &mat_bg, &[]);
            pass.set_bind_group(2, &light_bg, &[]);
            pass.set_bind_group(3, &shadow_bg, &[]);
            pass.set_vertex_buffer(0, vb.slice());
            pass.set_vertex_buffer(1, instance_buf.slice());
            pass.set_index_buffer(ib.slice(), ib.format());
            pass.draw_indexed(0..ib.count(), 0, 0..1000);
        }

        queue.submit(std::iter::once(encoder.finish()));
        device.poll(wgpu::Maintain::Wait);
        let elapsed = start.elapsed();

        eprintln!(
            "1,000 mesh instances: {:.2}ms",
            elapsed.as_secs_f64() * 1000.0,
        );
        assert!(
            elapsed.as_millis() < 8,
            "1,000 instances took {}ms, expected < 8ms",
            elapsed.as_millis(),
        );
    }
}
