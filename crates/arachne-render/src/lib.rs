pub mod context;
pub mod pipeline;
pub mod pipeline_2d;
pub mod pipeline_3d;
pub mod pipeline_shadow;
pub mod pipeline_shapes;
pub mod pipeline_text;
pub mod pipeline_tilemap;
pub mod buffer;
pub mod builtin_font;
pub mod builtin_tiles;
pub mod texture;
pub mod camera;
pub mod graph;
pub mod render2d;
pub mod render3d;

// Re-export key types
pub use context::{PresentMode, RenderContext};
pub use pipeline::{PipelineCache, PipelineKey};
pub use buffer::{BufferPool, DynamicBuffer, UniformBuffer, VertexBuffer, IndexBuffer};
pub use texture::{TextureHandle, TextureStorage, TextureAtlas};
pub use camera::{Camera2d, Camera3d, CameraUniform};
pub use graph::RenderGraph;
pub use pipeline_2d::{create_sprite_pipeline, create_camera_bind_group, create_texture_bind_group, RenderFrame};
pub use pipeline_3d::{create_mesh_pipeline, create_depth_texture, create_light_uniform_buffer, create_material_bind_group as create_mesh_material_bind_group, render_meshes, MeshDrawCall};
pub use pipeline_shadow::{create_shadow_pipeline, create_shadow_map, compute_light_space_matrix, render_shadow_pass, create_shadow_bind_group, DEFAULT_SHADOW_RESOLUTION};
pub use pipeline_shapes::create_shape_pipeline;
pub use pipeline_text::{create_text_pipeline, create_fallback_text_pipeline, render_text};
pub use pipeline_tilemap::create_tilemap_pipeline;
pub use builtin_font::generate_builtin_font;
pub use builtin_tiles::generate_builtin_tiles;
pub use render2d::{
    Anchor, Sprite, SpriteBatch, SpriteRenderer, SpriteInstance, SpriteVertex,
    ShapeRenderer, ShapeVertex, ShapePrepared,
    TextRenderer, TextVertex, TextParams, TextPrepared, BmFont,
    TilemapRenderer, TilemapLayer, TilemapPrepared, Tile,
    Batcher, BatchStats,
};

pub use render3d::{
    MeshVertex, MeshInstance, CameraUniform3d, MeshRenderer,
    PbrMaterial, MaterialHandle, MaterialUniform, MaterialStorage, Albedo,
    PointLight, DirectionalLight, SpotLight, GpuLight, LightUniform, LightState,
    SkyboxRenderer, SkyboxVertex,
    ShadowMap, ShadowUniform,
};

/// Embedded shader sources.
pub mod shaders {
    pub const SPRITE: &str = include_str!("shaders/sprite.wgsl");
    pub const SHAPE: &str = include_str!("shaders/shape.wgsl");
    pub const TEXT_SDF: &str = include_str!("shaders/text_sdf.wgsl");
    pub const MESH_PBR: &str = include_str!("shaders/mesh_pbr.wgsl");
    pub const SHADOW: &str = include_str!("shaders/shadow.wgsl");
    pub const SKYBOX: &str = include_str!("shaders/skybox.wgsl");
    pub const TILEMAP: &str = include_str!("shaders/tilemap.wgsl");
    pub const POSTPROCESS: &str = include_str!("shaders/postprocess.wgsl");
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use arachne_math::{Color, Mat4, Vec2, Vec3, Rect};

    fn test_ctx() -> (wgpu::Device, wgpu::Queue) {
        pollster::block_on(async {
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::LowPower,
                    ..Default::default()
                })
                .await
                .expect("no GPU adapter found");
            adapter
                .request_device(&wgpu::DeviceDescriptor::default(), None)
                .await
                .expect("device creation failed")
        })
    }

    #[test]
    fn sprite_10k_benchmark() {
        let (device, queue) = test_ctx();
        let mut renderer = SpriteRenderer::new(&device);
        let transform = Mat4::IDENTITY;
        let uv = Rect::new(Vec2::ZERO, Vec2::ONE);
        let tex = TextureHandle(0);

        let start = std::time::Instant::now();
        renderer.begin_frame();
        for _i in 0..10_000 {
            let sprite = Sprite::new(tex);
            renderer.draw(&sprite, &transform, uv, 0.0);
        }
        let (_batches, stats) = renderer.prepare(&device, &queue);
        let elapsed = start.elapsed();

        eprintln!(
            "10,000 sprites: {:.2}ms, {} draw calls, reduction {:.1}%",
            elapsed.as_secs_f64() * 1000.0,
            stats.draw_calls,
            stats.reduction_ratio() * 100.0,
        );

        assert!(
            elapsed.as_millis() < 8,
            "10,000 sprites took {}ms, expected < 8ms",
            elapsed.as_millis()
        );
        assert_eq!(stats.draw_calls, 1);
    }

    #[test]
    fn batching_1000_sprites_4_textures() {
        let (device, queue) = test_ctx();
        let mut renderer = SpriteRenderer::new(&device);
        let transform = Mat4::IDENTITY;
        let uv = Rect::new(Vec2::ZERO, Vec2::ONE);

        renderer.begin_frame();
        for i in 0..1000u32 {
            let tex = TextureHandle(i % 4);
            let sprite = Sprite::new(tex);
            renderer.draw(&sprite, &transform, uv, 0.0);
        }
        let (batches, stats) = renderer.prepare(&device, &queue);

        assert!(
            batches.len() <= 10,
            "1000 sprites/4 textures -> {} draw calls, expected <= 10",
            batches.len()
        );

        let reduction = stats.reduction_ratio();
        assert!(
            reduction >= 0.9,
            "draw call reduction {:.1}%, expected >= 90%",
            reduction * 100.0
        );
    }

    #[test]
    fn pipeline_cache_hit_rate() {
        let (device, _queue) = test_ctx();
        let mut cache = PipelineCache::new();

        let shader_src = shaders::SHAPE;
        let shader_hash = pipeline::hash_shader_source(shader_src);

        let key = PipelineKey {
            shader_hash,
            vertex_layout_hash: 0,
            blend_enabled: false,
            depth_enabled: false,
            topology: pipeline::PrimitiveTopologyKey::TriangleList,
        };

        let camera_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("camera_bgl"),
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

        // Create pipeline once
        let _ = cache.get_or_create(&key, || {
            let module = pipeline::create_shader_module(&device, "shape", shader_src);
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("test"),
                bind_group_layouts: &[&camera_bgl],
                push_constant_ranges: &[],
            });
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("test"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs_main"),
                    buffers: &[ShapeVertex::LAYOUT],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba8UnormSrgb,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            })
        });

        // Hit the cache 99 more times
        for _ in 0..99 {
            let _ = cache.get_or_create(&key, || {
                panic!("should not be called on cache hit");
            });
        }

        let hit_rate = cache.stats().hit_rate();
        assert!(
            hit_rate >= 0.95,
            "cache hit rate {:.1}%, expected >= 95%",
            hit_rate * 100.0
        );
    }

    #[test]
    fn camera2d_matrices_reference() {
        let cam = Camera2d::new(800.0, 600.0);
        let vp = cam.view_projection();

        // Y is flipped for wgpu clip space (Y=-1 at top).
        let expected = Mat4::orthographic(-400.0, 400.0, 300.0, -300.0, -1.0, 1.0);

        for c in 0..4 {
            for r in 0..4 {
                assert!(
                    (vp.cols[c][r] - expected.cols[c][r]).abs() < 1e-5,
                    "mismatch at [{c}][{r}]: got {}, expected {}",
                    vp.cols[c][r],
                    expected.cols[c][r],
                );
            }
        }
    }

    #[test]
    fn camera3d_matrices_reference() {
        let cam = Camera3d {
            position: Vec3::new(0.0, 0.0, 5.0),
            target: Vec3::ZERO,
            up: Vec3::Y,
            fov: std::f32::consts::FRAC_PI_4,
            near: 0.1,
            far: 100.0,
            aspect: 16.0 / 9.0,
        };

        let view = cam.view_matrix();
        let proj = cam.projection_matrix();

        let expected_view = Mat4::look_at(
            Vec3::new(0.0, 0.0, 5.0),
            Vec3::ZERO,
            Vec3::Y,
        );
        for c in 0..4 {
            for r in 0..4 {
                assert!(
                    (view.cols[c][r] - expected_view.cols[c][r]).abs() < 1e-5,
                    "view mismatch at [{c}][{r}]"
                );
            }
        }

        let expected_proj = Mat4::perspective(
            std::f32::consts::FRAC_PI_4,
            16.0 / 9.0,
            0.1,
            100.0,
        );
        for c in 0..4 {
            for r in 0..4 {
                assert!(
                    (proj.cols[c][r] - expected_proj.cols[c][r]).abs() < 1e-5,
                    "proj mismatch at [{c}][{r}]"
                );
            }
        }
    }
}
