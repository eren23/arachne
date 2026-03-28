//! Shared GPU resource initialization used by both WindowedRunner and WasmRunner.
//!
//! Extracts the common GPU setup (pipelines, textures, camera uniform, font atlas)
//! into a single function so windowed and WASM runners share the same code.

use arachne_ecs::World;
use arachne_render::{
    Camera2d, RenderContext, SpriteRenderer, TextRenderer, TilemapRenderer,
};
use wgpu::util::DeviceExt;

use crate::systems::{
    ScreenTextBuffer, SpriteRendererResource, TextRendererResource,
    TextureStorageResource, TilemapRendererResource,
};

// ---------------------------------------------------------------------------
// Resource wrappers for GPU types (must be Send + Sync for ECS)
// ---------------------------------------------------------------------------

/// ECS resource wrapping [`RenderContext`].
///
/// The inner `RenderContext` contains a `wgpu::Surface` which is `!Send` on some
/// backends. Access is always single-threaded via `ResMut` in the ECS schedule.
pub struct RenderContextResource(pub RenderContext);
// SAFETY: Only accessed via &mut through ResMut in a single-threaded schedule.
unsafe impl Send for RenderContextResource {}
unsafe impl Sync for RenderContextResource {}

/// ECS resource wrapping the sprite render pipeline.
pub struct SpritePipelineResource(pub wgpu::RenderPipeline);
unsafe impl Send for SpritePipelineResource {}
unsafe impl Sync for SpritePipelineResource {}

/// ECS resource wrapping the tilemap render pipeline.
pub struct TilemapPipelineResource(pub wgpu::RenderPipeline);
unsafe impl Send for TilemapPipelineResource {}
unsafe impl Sync for TilemapPipelineResource {}

/// GPU resources needed for rendering (camera uniform buffer and bind group).
pub struct GpuResources {
    pub camera_buffer: wgpu::Buffer,
    pub camera_bind_group: wgpu::BindGroup,
}
unsafe impl Send for GpuResources {}
unsafe impl Sync for GpuResources {}

// ---------------------------------------------------------------------------
// Shared GPU initialization
// ---------------------------------------------------------------------------

/// Initialize all GPU rendering resources and insert them into the ECS world.
///
/// This creates sprite, tilemap, and text pipelines, a 1x1 white fallback
/// texture, the camera uniform buffer, and the built-in bitmap font atlas.
///
/// Both the windowed (winit) runner and WASM (canvas) runner call this after
/// creating a `RenderContext` from their respective surfaces.
pub fn init_gpu_resources(world: &mut World, context: &RenderContext) {
    let device = context.device();
    let queue = context.queue();
    let format = context.surface_format();

    // --- Sprite pipeline + renderer ---
    let sprite_pipeline = arachne_render::create_sprite_pipeline(device, format);
    let sprite_renderer = SpriteRenderer::new(device);

    let srr = SpriteRendererResource {
        renderer: sprite_renderer,
        device: device.clone(),
        queue: queue.clone(),
        last_batches: Vec::new(),
    };

    // --- TextureStorage with 1x1 white fallback at handle 0 ---
    let mut tex_storage = arachne_render::TextureStorage::new(device);
    let _fallback_handle = tex_storage.create_texture(
        device, queue, 1, 1, &[255u8, 255, 255, 255],
    );
    let mut tex_res = TextureStorageResource(tex_storage);

    // Load pixel art assets if available (native only; WASM uses fetch).
    #[cfg(not(target_arch = "wasm32"))]
    {
        let asset_paths = [
            "assets/sprites/player.png",   // handle 1
            "assets/tiles/tileset.png",    // handle 2
            "assets/physics/circle.png",   // handle 3
            "assets/physics/box.png",      // handle 4
        ];
        for path in &asset_paths {
            match tex_res.load_png(device, queue, path) {
                Ok(_h) => {}
                Err(_e) => {
                    // Asset not found — create a colored fallback instead.
                    tex_res.create_from_rgba(device, queue, 1, 1, &[200, 100, 200, 255]);
                }
            }
        }
    }

    // On WASM, create placeholder handles so handle indices match.
    // Actual textures will be loaded asynchronously via fetch.
    #[cfg(target_arch = "wasm32")]
    {
        for _ in 0..4 {
            tex_res.create_from_rgba(device, queue, 1, 1, &[200, 100, 200, 255]);
        }
    }

    world.insert_resource(tex_res);

    // --- Camera uniform buffer + bind group ---
    let camera_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("camera_uniform"),
        size: 64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let cam_bgl = sprite_pipeline.get_bind_group_layout(0);
    let camera_bg = arachne_render::create_camera_bind_group(device, &cam_bgl, &camera_buf);

    world.insert_resource(GpuResources {
        camera_buffer: camera_buf,
        camera_bind_group: camera_bg,
    });

    world.insert_resource(srr);
    world.insert_resource(SpritePipelineResource(sprite_pipeline));

    // --- Tilemap pipeline + renderer + built-in tile atlas ---
    {
        let tilemap_pipeline = arachne_render::create_tilemap_pipeline(device, format);
        let tilemap_renderer = TilemapRenderer::new(device);

        let (atlas_rgba, atlas_w, atlas_h, _tile_cols, _tile_rows) =
            arachne_render::generate_builtin_tiles();

        let tex_store = world.get_resource_mut::<TextureStorageResource>();
        let atlas_handle = tex_store.create_from_rgba(device, queue, atlas_w, atlas_h, &atlas_rgba);

        let tilemap_res = TilemapRendererResource {
            renderer: tilemap_renderer,
            layers: Vec::new(),
            atlas_texture: atlas_handle,
            last_prepared: arachne_render::TilemapPrepared {
                vertex_count: 0,
                index_count: 0,
            },
            device: device.clone(),
            queue: queue.clone(),
        };

        world.insert_resource(tilemap_res);
        world.insert_resource(TilemapPipelineResource(tilemap_pipeline));
    }

    // --- Text pipeline + built-in bitmap font atlas ---
    {
        let text_pipeline = arachne_render::create_text_pipeline(device, format);

        let (atlas_rgba, font) = arachne_render::generate_builtin_font();
        let atlas_w = arachne_render::builtin_font::ATLAS_W;
        let atlas_h = arachne_render::builtin_font::ATLAS_H;

        // Convert RGBA to single-channel R8: take the alpha channel.
        let mut r8_data = vec![0u8; (atlas_w * atlas_h) as usize];
        for i in 0..(atlas_w * atlas_h) as usize {
            r8_data[i] = atlas_rgba[i * 4 + 3];
        }

        let atlas_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("builtin_font_atlas"),
            size: wgpu::Extent3d {
                width: atlas_w,
                height: atlas_h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &atlas_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &r8_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(atlas_w),
                rows_per_image: Some(atlas_h),
            },
            wgpu::Extent3d {
                width: atlas_w,
                height: atlas_h,
                depth_or_array_layers: 1,
            },
        );

        let atlas_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("font_sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Bind groups matching the text SDF pipeline layout.
        let text_params = arachne_render::TextParams {
            edge_softness: 0.05,
            outline_width: 0.0,
            ..Default::default()
        };
        let text_params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("text_params"),
            contents: bytemuck::bytes_of(&text_params),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let gpu = world.get_resource::<GpuResources>();
        let text_camera_bgl = text_pipeline.get_bind_group_layout(0);
        let text_camera_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("text_camera_bg"),
            layout: &text_camera_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: gpu.camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: text_params_buffer.as_entire_binding(),
                },
            ],
        });

        let text_font_bgl = text_pipeline.get_bind_group_layout(1);
        let text_font_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("text_font_bg"),
            layout: &text_font_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&atlas_sampler),
                },
            ],
        });

        let text_renderer = TextRenderer::new(device);

        let text_res = TextRendererResource {
            renderer: text_renderer,
            font,
            pipeline: text_pipeline,
            camera_bind_group: text_camera_bg,
            font_bind_group: text_font_bg,
            text_params_buffer,
            last_prepared: arachne_render::TextPrepared {
                vertex_count: 0,
                index_count: 0,
            },
        };

        world.insert_resource(text_res);
    }

    // --- ScreenTextBuffer ---
    if !world.has_resource::<ScreenTextBuffer>() {
        world.insert_resource(ScreenTextBuffer::default());
    }

    // --- Update Camera2d viewport to match surface ---
    if world.has_resource::<Camera2d>() {
        let (w, h) = context.surface_size();
        let cam = world.get_resource_mut::<Camera2d>();
        cam.viewport_size = arachne_math::Vec2::new(w as f32, h as f32);
    }
}
