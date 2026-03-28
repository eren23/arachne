//! Text render pipeline for SDF-based font rendering.
//!
//! Creates a wgpu render pipeline for [`TextVertex`](crate::render2d::text::TextVertex)
//! geometry and provides a standalone `render_text` function to issue indexed draw calls.
//! A fallback pipeline is available for solid-color text when no font atlas is loaded.

use crate::render2d::text::TextVertex;
use crate::shaders;

/// Inline shader for fallback (no-atlas) text rendering.
/// Uses the same vertex layout as the SDF pipeline but outputs vertex color directly.
const FALLBACK_TEXT_SHADER: &str = r#"
struct CameraUniform {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(in.position, 0.0, 1.0);
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    if (in.color.a < 0.01) {
        discard;
    }
    return in.color;
}
"#;

/// Create the SDF text render pipeline.
///
/// Bind group layout 0: camera uniform (binding 0, mat4x4<f32>) + TextParams uniform
/// (binding 1, 32 bytes).
/// Bind group layout 1: font atlas texture (binding 0) + sampler (binding 1).
/// Alpha blend: SrcAlpha / OneMinusSrcAlpha.  Primitive: TriangleList, no depth.
pub fn create_text_pipeline(
    device: &wgpu::Device,
    surface_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("text_sdf_shader"),
        source: wgpu::ShaderSource::Wgsl(shaders::TEXT_SDF.into()),
    });

    // Group 0: camera uniform + text params
    let camera_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("text_camera_bgl"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });

    // Group 1: font atlas texture + sampler
    let font_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("text_font_bgl"),
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
        label: Some("text_pipeline_layout"),
        bind_group_layouts: &[&camera_bgl, &font_bgl],
        push_constant_ranges: &[],
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("text_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader_module,
            entry_point: Some("vs_main"),
            buffers: &[TextVertex::LAYOUT],
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
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

/// Create a fallback text pipeline that renders solid-color quads (no texture sampling).
///
/// Use this when no font atlas is available. Bind group layout 0: camera uniform only
/// (binding 0, mat4x4<f32>). No bind group 1. Same vertex layout as the SDF pipeline.
pub fn create_fallback_text_pipeline(
    device: &wgpu::Device,
    surface_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("text_fallback_shader"),
        source: wgpu::ShaderSource::Wgsl(FALLBACK_TEXT_SHADER.into()),
    });

    let camera_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("text_fallback_camera_bgl"),
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
        label: Some("text_fallback_pipeline_layout"),
        bind_group_layouts: &[&camera_bgl],
        push_constant_ranges: &[],
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("text_fallback_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader_module,
            entry_point: Some("vs_main"),
            buffers: &[TextVertex::LAYOUT],
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
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

/// Issue indexed draw calls for SDF text geometry.
///
/// Sets the pipeline, camera + font bind groups, vertex/index buffers, and draws.
/// Skips the draw if `index_count` is zero.
pub fn render_text<'a>(
    pass: &mut wgpu::RenderPass<'a>,
    pipeline: &'a wgpu::RenderPipeline,
    camera_bg: &'a wgpu::BindGroup,
    font_bg: &'a wgpu::BindGroup,
    vertex_buffer: &'a wgpu::Buffer,
    index_buffer: &'a wgpu::Buffer,
    index_count: u32,
) {
    if index_count == 0 {
        return;
    }

    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, Some(camera_bg), &[]);
    pass.set_bind_group(1, Some(font_bg), &[]);
    pass.set_vertex_buffer(0, vertex_buffer.slice(..));
    pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
    pass.draw_indexed(0..index_count, 0, 0..1);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render2d::text::{BmFont, TextRenderer};
    use arachne_math::Color;
    use arachne_math::Vec2;

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

    fn sample_fnt() -> &'static str {
        r#"info face="TestFont" size=32 bold=0 italic=0 charset="" unicode=1 stretchH=100 smooth=1 aa=1 padding=0,0,0,0 spacing=1,1 outline=0
common lineHeight=40 base=32 scaleW=256 scaleH=256 pages=1 packed=0 alphaChnl=0 redChnl=4 greenChnl=4 blueChnl=4
page id=0 file="test.png"
chars count=5
char id=72  x=0   y=0   width=20  height=30  xoffset=1  yoffset=2  xadvance=22  page=0  chnl=15
char id=101 x=21  y=0   width=16  height=22  xoffset=1  yoffset=10 xadvance=18  page=0  chnl=15
char id=108 x=38  y=0   width=8   height=30  xoffset=2  yoffset=2  xadvance=10  page=0  chnl=15
char id=111 x=47  y=0   width=16  height=22  xoffset=1  yoffset=10 xadvance=18  page=0  chnl=15
char id=32  x=0   y=0   width=0   height=0   xoffset=0  yoffset=0  xadvance=10  page=0  chnl=15
kerning first=72 second=101 amount=-1
"#
    }

    #[test]
    fn text_pipeline_creation_succeeds() {
        let (device, _queue) = test_ctx();
        let pipeline = create_text_pipeline(&device, wgpu::TextureFormat::Rgba8UnormSrgb);
        // Verify both bind group layouts are accessible
        let _camera_bgl = pipeline.get_bind_group_layout(0);
        let _font_bgl = pipeline.get_bind_group_layout(1);
    }

    #[test]
    fn fallback_pipeline_creation_succeeds() {
        let (device, _queue) = test_ctx();
        let pipeline =
            create_fallback_text_pipeline(&device, wgpu::TextureFormat::Rgba8UnormSrgb);
        // Only camera bind group layout
        let _camera_bgl = pipeline.get_bind_group_layout(0);
    }

    #[test]
    fn text_renderer_hello_world_nonzero_vertices() {
        let (device, queue) = test_ctx();
        let font = BmFont::from_fnt(sample_fnt());
        let mut renderer = TextRenderer::new(&device);
        renderer.begin_frame();
        renderer.draw_text(
            &font,
            "Hello",
            Vec2::ZERO,
            40.0,
            Color::WHITE,
            None,
        );
        let prepared = renderer.prepare(&device, &queue);
        assert!(
            prepared.vertex_count > 0,
            "Hello should produce non-zero vertices, got {}",
            prepared.vertex_count,
        );
        assert!(
            prepared.index_count > 0,
            "Hello should produce non-zero indices, got {}",
            prepared.index_count,
        );
    }

    #[test]
    fn text_renderer_empty_string_zero_vertices() {
        let (device, queue) = test_ctx();
        let font = BmFont::from_fnt(sample_fnt());
        let mut renderer = TextRenderer::new(&device);
        renderer.begin_frame();
        renderer.draw_text(&font, "", Vec2::ZERO, 40.0, Color::WHITE, None);
        let prepared = renderer.prepare(&device, &queue);
        assert_eq!(prepared.vertex_count, 0);
        assert_eq!(prepared.index_count, 0);
    }

    #[test]
    fn text_pipeline_camera_and_params_bind_group() {
        let (device, _queue) = test_ctx();
        let pipeline = create_text_pipeline(&device, wgpu::TextureFormat::Rgba8UnormSrgb);
        let layout = pipeline.get_bind_group_layout(0);

        let camera_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera_uniform"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("text_params"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Should succeed — layout expects camera (binding 0) + text_params (binding 1)
        let _bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("text_camera_bg"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: params_buf.as_entire_binding(),
                },
            ],
        });
    }

    #[test]
    fn text_pipeline_font_bind_group() {
        let (device, _queue) = test_ctx();
        let pipeline = create_text_pipeline(&device, wgpu::TextureFormat::Rgba8UnormSrgb);
        let layout = pipeline.get_bind_group_layout(1);

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("font_atlas"),
            size: wgpu::Extent3d {
                width: 256,
                height: 256,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("font_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let _bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("text_font_bg"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
    }
}
