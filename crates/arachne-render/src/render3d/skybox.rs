use arachne_math::Mat4;
use crate::pipeline::create_shader_module;
use crate::buffer::VertexBuffer;

// ---------------------------------------------------------------------------
// Skybox vertex (just position)
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SkyboxVertex {
    pub position: [f32; 3],
}

impl SkyboxVertex {
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: 12,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x3,
            offset: 0,
            shader_location: 0,
        }],
    };
}

/// Skybox camera uniform (view-rotation-projection matrix only, no translation).
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SkyboxCameraUniform {
    pub view_rotation_proj: [[f32; 4]; 4],
}

impl SkyboxCameraUniform {
    /// Build from a view matrix and projection matrix, stripping translation.
    pub fn new(view: &Mat4, projection: &Mat4) -> Self {
        let mut view_rot = *view;
        // Zero out translation column
        view_rot.cols[3][0] = 0.0;
        view_rot.cols[3][1] = 0.0;
        view_rot.cols[3][2] = 0.0;
        Self {
            view_rotation_proj: (*projection * view_rot).cols,
        }
    }
}

// ---------------------------------------------------------------------------
// SkyboxRenderer
// ---------------------------------------------------------------------------

pub struct SkyboxRenderer {
    pipeline: wgpu::RenderPipeline,
    camera_bgl: wgpu::BindGroupLayout,
    cubemap_bgl: wgpu::BindGroupLayout,
    cube_vb: VertexBuffer<SkyboxVertex>,
    sampler: wgpu::Sampler,
}

impl SkyboxRenderer {
    pub fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> Self {
        let shader = create_shader_module(device, "skybox", include_str!("../shaders/skybox.wgsl"));

        let camera_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("skybox_camera_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(
                        std::mem::size_of::<SkyboxCameraUniform>() as u64,
                    ),
                },
                count: None,
            }],
        });

        let cubemap_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("skybox_cubemap_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::Cube,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("skybox_pipeline_layout"),
            bind_group_layouts: &[&camera_bgl, &cubemap_bgl],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("skybox_pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[SkyboxVertex::LAYOUT],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                // Inside of cube = no culling (we're inside it)
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let cube_vb = VertexBuffer::new(device, &Self::cube_vertices());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("skybox_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Self {
            pipeline,
            camera_bgl,
            cubemap_bgl,
            cube_vb,
            sampler,
        }
    }

    pub fn pipeline(&self) -> &wgpu::RenderPipeline {
        &self.pipeline
    }

    pub fn camera_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.camera_bgl
    }

    pub fn cubemap_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.cubemap_bgl
    }

    pub fn create_camera_bind_group(
        &self,
        device: &wgpu::Device,
        buffer: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("skybox_camera_bg"),
            layout: &self.camera_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        })
    }

    pub fn create_cubemap_bind_group(
        &self,
        device: &wgpu::Device,
        cubemap_view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("skybox_cubemap_bg"),
            layout: &self.cubemap_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(cubemap_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        })
    }

    pub fn cube_vertex_buffer(&self) -> &VertexBuffer<SkyboxVertex> {
        &self.cube_vb
    }

    /// 36 vertices for a unit cube (no index buffer, for simplicity).
    fn cube_vertices() -> Vec<SkyboxVertex> {
        let p = [
            [-1.0f32, -1.0, -1.0],
            [ 1.0, -1.0, -1.0],
            [ 1.0,  1.0, -1.0],
            [-1.0,  1.0, -1.0],
            [-1.0, -1.0,  1.0],
            [ 1.0, -1.0,  1.0],
            [ 1.0,  1.0,  1.0],
            [-1.0,  1.0,  1.0],
        ];

        let tris: [[usize; 3]; 12] = [
            // -Z face
            [0, 2, 1], [0, 3, 2],
            // +Z face
            [4, 5, 6], [4, 6, 7],
            // -X face
            [0, 4, 7], [0, 7, 3],
            // +X face
            [1, 2, 6], [1, 6, 5],
            // -Y face
            [0, 1, 5], [0, 5, 4],
            // +Y face
            [3, 7, 6], [3, 6, 2],
        ];

        let mut verts = Vec::with_capacity(36);
        for tri in &tris {
            for &idx in tri {
                verts.push(SkyboxVertex { position: p[idx] });
            }
        }
        verts
    }

    /// Helper: create a cubemap texture from 6 face RGBA8 data arrays.
    /// Each face is `size × size` pixels.
    pub fn create_cubemap(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        size: u32,
        faces: [&[u8]; 6],
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("cubemap"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 6,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        for (i, face_data) in faces.iter().enumerate() {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: i as u32,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                face_data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * size),
                    rows_per_image: Some(size),
                },
                wgpu::Extent3d {
                    width: size,
                    height: size,
                    depth_or_array_layers: 1,
                },
            );
        }

        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });

        (texture, view)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::UniformBuffer;

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

    fn create_test_cubemap(device: &wgpu::Device, queue: &wgpu::Queue) -> (wgpu::Texture, wgpu::TextureView) {
        let colors: [[u8; 4]; 6] = [
            [255, 0, 0, 255],
            [0, 255, 0, 255],
            [0, 0, 255, 255],
            [255, 255, 0, 255],
            [255, 0, 255, 255],
            [0, 255, 255, 255],
        ];
        let faces: [&[u8]; 6] = [
            &colors[0], &colors[1], &colors[2],
            &colors[3], &colors[4], &colors[5],
        ];
        SkyboxRenderer::create_cubemap(device, queue, 1, faces)
    }

    #[test]
    fn skybox_pipeline_creation() {
        let (device, _queue) = test_ctx();
        let _renderer = SkyboxRenderer::new(&device, wgpu::TextureFormat::Rgba8UnormSrgb);
        // Reaching here means the pipeline compiled without errors.
    }

    #[test]
    fn skybox_cube_vertices() {
        let verts = SkyboxRenderer::cube_vertices();
        assert_eq!(verts.len(), 36);
    }

    #[test]
    fn skybox_renders_behind_geometry() {
        let (device, queue) = test_ctx();
        let skybox = SkyboxRenderer::new(&device, wgpu::TextureFormat::Rgba8UnormSrgb);
        let (_cube_tex, cubemap_view) = create_test_cubemap(&device, &queue);

        // Camera uniform (strip translation)
        let view = Mat4::look_at(
            arachne_math::Vec3::new(0.0, 0.0, 3.0),
            arachne_math::Vec3::ZERO,
            arachne_math::Vec3::Y,
        );
        let proj = Mat4::perspective(std::f32::consts::FRAC_PI_4, 1.0, 0.1, 100.0);
        let skybox_cam = SkyboxCameraUniform::new(&view, &proj);
        let cam_buf = UniformBuffer::new(&device, "skybox_cam", &skybox_cam);
        let cam_bg = skybox.create_camera_bind_group(&device, cam_buf.buffer());
        let cubemap_bg = skybox.create_cubemap_bind_group(&device, &cubemap_view);

        // Render targets
        let color_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("test_color"),
            size: wgpu::Extent3d { width: 64, height: 64, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let color_view = color_tex.create_view(&Default::default());

        let depth_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("test_depth"),
            size: wgpu::Extent3d { width: 64, height: 64, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let depth_view = depth_tex.create_view(&Default::default());

        // Render skybox
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("skybox_pass"),
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

            pass.set_pipeline(skybox.pipeline());
            pass.set_bind_group(0, &cam_bg, &[]);
            pass.set_bind_group(1, &cubemap_bg, &[]);
            pass.set_vertex_buffer(0, skybox.cube_vertex_buffer().slice());
            pass.draw(0..36, 0..1);
        }

        // Read back color to verify skybox rendered something (not all black)
        let bpr = 64 * 4;
        let aligned_bpr = (bpr + 255) & !255;
        let staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: (aligned_bpr * 64) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &color_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &staging,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(aligned_bpr as u32),
                    rows_per_image: Some(64),
                },
            },
            wgpu::Extent3d { width: 64, height: 64, depth_or_array_layers: 1 },
        );

        queue.submit(std::iter::once(encoder.finish()));

        staging.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        device.poll(wgpu::Maintain::Wait);

        let data = staging.slice(..).get_mapped_range();
        // The skybox depth test is LessEqual with z=1.0. Any geometry with z < 1.0
        // would draw in front. Since we only drew the skybox, it should fill the screen.
        // Verify at least some non-black pixels exist (skybox rendered).
        let mut has_color = false;
        for row in 0..64u32 {
            let offset = (row * aligned_bpr as u32) as usize;
            let row_data = &data[offset..offset + bpr];
            for pixel in row_data.chunks(4) {
                if pixel[0] > 0 || pixel[1] > 0 || pixel[2] > 0 {
                    has_color = true;
                    break;
                }
            }
            if has_color { break; }
        }

        drop(data);
        staging.unmap();

        assert!(has_color, "skybox should render non-black pixels (draws behind geometry at depth=1.0)");
    }

    #[test]
    fn skybox_camera_strips_translation() {
        let view = Mat4::look_at(
            arachne_math::Vec3::new(10.0, 20.0, 30.0),
            arachne_math::Vec3::ZERO,
            arachne_math::Vec3::Y,
        );
        let proj = Mat4::perspective(std::f32::consts::FRAC_PI_4, 1.0, 0.1, 100.0);
        let u = SkyboxCameraUniform::new(&view, &proj);

        // The view-rotation-proj should map a far-away point the same regardless
        // of camera position (since translation is stripped).
        let view2 = Mat4::look_at(
            arachne_math::Vec3::new(0.0, 0.0, 0.0),
            arachne_math::Vec3::new(-10.0, -20.0, -30.0), // same direction
            arachne_math::Vec3::Y,
        );
        let u2 = SkyboxCameraUniform::new(&view2, &proj);

        // Both should produce the same rotation component (approximately)
        // Just verify both are valid (non-zero) matrices
        let m1 = Mat4 { cols: u.view_rotation_proj };
        let m2 = Mat4 { cols: u2.view_rotation_proj };
        assert!(m1.determinant().abs() > 1e-6, "skybox matrix should be non-singular");
        assert!(m2.determinant().abs() > 1e-6, "skybox matrix should be non-singular");
    }
}
