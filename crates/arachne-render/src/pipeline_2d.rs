//! Sprite render pipeline and RenderFrame orchestration.
//!
//! Creates the wgpu render pipeline for sprite instanced rendering and provides
//! [`RenderFrame`] for acquiring surface textures and submitting draw calls.

use crate::context::RenderContext;
use crate::render2d::batch::MergedDrawCall;
use crate::render2d::shape::ShapePrepared;
use crate::render2d::sprite::{SpriteInstance, SpriteVertex};
use crate::render2d::text::TextPrepared;
use crate::shaders;

/// Create the sprite render pipeline.
///
/// Bind group layout 0: camera uniform buffer (mat4x4<f32>, 64 bytes, vertex+fragment).
/// Bind group layout 1: texture_2d<f32> + filtering sampler.
/// Alpha blend: SrcAlpha / OneMinusSrcAlpha. Primitive: TriangleList, CCW, cull back.
pub fn create_sprite_pipeline(
    device: &wgpu::Device,
    surface_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("sprite_shader"),
        source: wgpu::ShaderSource::Wgsl(shaders::SPRITE.into()),
    });

    let camera_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("sprite_camera_bgl"),
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

    let texture_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("sprite_texture_bgl"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
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

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("sprite_pipeline_layout"),
        bind_group_layouts: &[&camera_bgl, &texture_bgl],
        push_constant_ranges: &[],
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("sprite_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader_module,
            entry_point: Some("vs_main"),
            buffers: &[SpriteVertex::LAYOUT, SpriteInstance::LAYOUT],
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
            cull_mode: None, // No culling for 2D sprites (Y-flip in projection reverses winding)
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

/// Create a camera bind group for the sprite pipeline.
///
/// The buffer must be at least 64 bytes (one `mat4x4<f32>`).
/// Use `pipeline.get_bind_group_layout(0)` for the layout.
pub fn create_camera_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    buffer: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("camera_bind_group"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: buffer.as_entire_binding(),
        }],
    })
}

/// Create a texture bind group for the sprite pipeline.
///
/// Use `pipeline.get_bind_group_layout(1)` for the layout.
pub fn create_texture_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("texture_bind_group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    })
}

/// Orchestrates a single frame: acquires a surface texture, records render passes,
/// and submits the command buffer.
pub struct RenderFrame {
    pub encoder: wgpu::CommandEncoder,
    surface_texture: Option<wgpu::SurfaceTexture>,
    pub surface_view: wgpu::TextureView,
}

impl RenderFrame {
    /// Begin a new frame by acquiring the surface texture and creating a command encoder.
    ///
    /// Returns `None` if the context is headless (no surface available).
    pub fn begin(context: &mut RenderContext) -> Option<Self> {
        // Acquire the surface texture into context's pending_frame
        let _ = context.current_texture()?;
        // Take ownership so RenderFrame controls the lifetime
        let surface_texture = context.take_pending_frame()?;
        let surface_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let encoder = context
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render_frame"),
            });
        Some(Self {
            encoder,
            surface_texture: Some(surface_texture),
            surface_view,
        })
    }

    /// Record sprite draw calls into a render pass.
    ///
    /// Creates a render pass that clears to dark gray (0.1, 0.1, 0.1), sets the
    /// sprite pipeline, and iterates draw calls. Each draw call's `texture` field
    /// indexes into `texture_bgs` via `texture.0 as usize`.
    pub fn render_sprites(
        &mut self,
        pipeline: &wgpu::RenderPipeline,
        draw_calls: &[MergedDrawCall],
        camera_bg: &wgpu::BindGroup,
        texture_bgs: &[&wgpu::BindGroup],
        vertex_buffer: &wgpu::Buffer,
        index_buffer: &wgpu::Buffer,
        instance_buffer: &wgpu::Buffer,
    ) {
        let mut pass = self.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("sprite_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.surface_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.1,
                        g: 0.1,
                        b: 0.1,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            ..Default::default()
        });

        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, Some(camera_bg), &[]);
        pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        pass.set_vertex_buffer(1, instance_buffer.slice(..));
        pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);

        for dc in draw_calls {
            let tex_idx = dc.texture.0 as usize;
            if tex_idx < texture_bgs.len() {
                pass.set_bind_group(1, Some(texture_bgs[tex_idx]), &[]);
            }
            pass.draw_indexed(
                dc.index_offset..dc.index_offset + dc.index_count,
                dc.base_vertex,
                dc.instance_offset..dc.instance_offset + dc.instance_count,
            );
        }
    }

    /// Record shape draw calls into a render pass.
    ///
    /// Uses `LoadOp::Load` so shapes composite onto whatever was already rendered
    /// (call this BEFORE `render_sprites` so shapes sit behind sprites in z-order).
    pub fn render_shapes(
        &mut self,
        pipeline: &wgpu::RenderPipeline,
        camera_bg: &wgpu::BindGroup,
        shape_renderer: &crate::render2d::shape::ShapeRenderer,
        prepared: &ShapePrepared,
    ) {
        if prepared.index_count == 0 {
            return;
        }

        let mut pass = self.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("shape_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.surface_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            ..Default::default()
        });

        crate::pipeline_shapes::render_shapes(
            &mut pass,
            pipeline,
            camera_bg,
            shape_renderer.vertex_buffer(),
            shape_renderer.index_buffer(),
            prepared.index_count,
        );
    }

    /// Record text draw calls into a render pass.
    ///
    /// Uses `LoadOp::Load` so text composites on top of everything already rendered.
    /// Call this AFTER `render_sprites` and `render_shapes` so text appears on top.
    pub fn render_text(
        &mut self,
        pipeline: &wgpu::RenderPipeline,
        camera_bg: &wgpu::BindGroup,
        font_bg: &wgpu::BindGroup,
        text_renderer: &crate::render2d::text::TextRenderer,
        prepared: &TextPrepared,
    ) {
        if prepared.index_count == 0 {
            return;
        }

        let mut pass = self.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("text_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.surface_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            ..Default::default()
        });

        crate::pipeline_text::render_text(
            &mut pass,
            pipeline,
            camera_bg,
            font_bg,
            text_renderer.vertex_buffer(),
            text_renderer.index_buffer(),
            prepared.index_count,
        );
    }

    /// Record a shadow depth pass before the main 3D pass.
    ///
    /// This is a depth-only pass that renders into the shadow map texture.
    /// Call this BEFORE `render_meshes` so shadow data is available for the
    /// main 3D pass. Render order: shadow pass -> 3D mesh pass -> 2D shapes
    /// -> 2D sprites -> text.
    pub fn render_shadow_pass(
        &mut self,
        pipeline: &wgpu::RenderPipeline,
        shadow_view: &wgpu::TextureView,
        light_bg: &wgpu::BindGroup,
        draw_calls: &[crate::pipeline_3d::MeshDrawCall],
        vertex_buffer: &wgpu::Buffer,
        index_buffer: &wgpu::Buffer,
        instance_buffer: &wgpu::Buffer,
    ) {
        crate::pipeline_shadow::render_shadow_pass(
            &mut self.encoder,
            pipeline,
            shadow_view,
            light_bg,
            draw_calls,
            vertex_buffer,
            index_buffer,
            instance_buffer,
        );
    }

    /// Record 3D mesh draw calls into a render pass with depth testing.
    ///
    /// 3D content renders first (before 2D) with depth write/test enabled.
    /// Uses `LoadOp::Clear` on both color and depth so this should be called
    /// before any 2D render methods.
    pub fn render_meshes(
        &mut self,
        pipeline: &wgpu::RenderPipeline,
        camera_bg: &wgpu::BindGroup,
        material_bg: &wgpu::BindGroup,
        light_bg: &wgpu::BindGroup,
        shadow_bg: &wgpu::BindGroup,
        draw_calls: &[crate::pipeline_3d::MeshDrawCall],
        vertex_buffer: &wgpu::Buffer,
        index_buffer: &wgpu::Buffer,
        instance_buffer: &wgpu::Buffer,
        depth_view: &wgpu::TextureView,
    ) {
        if draw_calls.is_empty() {
            return;
        }

        let mut pass = self.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mesh_3d_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.surface_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.1,
                        g: 0.1,
                        b: 0.1,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });

        crate::pipeline_3d::render_meshes(
            &mut pass,
            pipeline,
            camera_bg,
            material_bg,
            light_bg,
            shadow_bg,
            draw_calls,
            vertex_buffer,
            index_buffer,
            instance_buffer,
        );
    }

    /// Submit the command encoder to the queue and present the surface texture.
    pub fn present(self, queue: &wgpu::Queue) {
        queue.submit(std::iter::once(self.encoder.finish()));
        if let Some(tex) = self.surface_texture {
            tex.present();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render2d::sprite::{Sprite, SpriteRenderer};
    use crate::texture::TextureHandle;
    use arachne_math::{Mat4, Rect, Vec2};

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
    fn pipeline_creation_succeeds() {
        let (device, _queue) = test_ctx();
        let pipeline = create_sprite_pipeline(&device, wgpu::TextureFormat::Rgba8UnormSrgb);
        // Verify both bind group layouts are accessible
        let _camera_bgl = pipeline.get_bind_group_layout(0);
        let _texture_bgl = pipeline.get_bind_group_layout(1);
    }

    #[test]
    fn camera_bind_group_with_zero_buffer() {
        let (device, _queue) = test_ctx();
        let pipeline = create_sprite_pipeline(&device, wgpu::TextureFormat::Rgba8UnormSrgb);
        let layout = pipeline.get_bind_group_layout(0);
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera_uniform"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let _bg = create_camera_bind_group(&device, &layout, &buffer);
    }

    #[test]
    fn render_frame_begin_headless_returns_none() {
        let mut ctx = pollster::block_on(RenderContext::new_headless()).unwrap();
        assert!(RenderFrame::begin(&mut ctx).is_none());
    }

    #[test]
    fn one_sprite_one_texture_one_draw_call() {
        let (device, queue) = test_ctx();
        let mut renderer = SpriteRenderer::new(&device);
        renderer.begin_frame();
        let sprite = Sprite::new(TextureHandle(0));
        let uv = Rect::new(Vec2::ZERO, Vec2::ONE);
        renderer.draw(&sprite, &Mat4::IDENTITY, uv, 0.0);
        let (batches, stats) = renderer.prepare(&device, &queue);
        assert_eq!(batches.len(), 1);
        assert_eq!(stats.draw_calls, 1);
    }

    #[test]
    fn thousand_sprites_four_textures_four_draw_calls() {
        let (device, queue) = test_ctx();
        let mut renderer = SpriteRenderer::new(&device);
        let uv = Rect::new(Vec2::ZERO, Vec2::ONE);
        renderer.begin_frame();
        for i in 0..1000u32 {
            let sprite = Sprite::new(TextureHandle(i % 4));
            renderer.draw(&sprite, &Mat4::IDENTITY, uv, 0.0);
        }
        let (batches, stats) = renderer.prepare(&device, &queue);
        assert_eq!(batches.len(), 4);
        assert_eq!(stats.draw_calls, 4);
    }

    #[test]
    fn zero_sprites_zero_draw_calls() {
        let (device, queue) = test_ctx();
        let mut renderer = SpriteRenderer::new(&device);
        renderer.begin_frame();
        let (batches, stats) = renderer.prepare(&device, &queue);
        assert_eq!(batches.len(), 0);
        assert_eq!(stats.draw_calls, 0);
    }

    #[test]
    fn prepare_1000_sprites_benchmark() {
        let (device, queue) = test_ctx();
        let mut renderer = SpriteRenderer::new(&device);
        let uv = Rect::new(Vec2::ZERO, Vec2::ONE);

        let start = std::time::Instant::now();
        renderer.begin_frame();
        for _ in 0..1000 {
            let sprite = Sprite::new(TextureHandle(0));
            renderer.draw(&sprite, &Mat4::IDENTITY, uv, 0.0);
        }
        let (_batches, _stats) = renderer.prepare(&device, &queue);
        let elapsed = start.elapsed();

        eprintln!(
            "1,000 sprites prepare: {:.2}ms",
            elapsed.as_secs_f64() * 1000.0,
        );
        assert!(
            elapsed.as_millis() < 2,
            "prepare 1000 sprites took {}ms, expected < 2ms",
            elapsed.as_millis()
        );
    }
}
