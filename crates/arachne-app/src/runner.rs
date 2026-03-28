//! Runner trait and implementations.
//!
//! The runner owns the main loop. It calls `Schedule::run()` each frame,
//! updates the `Time` resource, and handles platform events.

use arachne_ecs::{Schedule, World};
use crate::time::Time;

// ---------------------------------------------------------------------------
// Runner trait
// ---------------------------------------------------------------------------

/// Trait for the main application loop driver.
///
/// A runner consumes the app's World + Schedule and enters a loop.
/// Different runners exist for native (winit), WASM, and headless (testing).
pub trait Runner: 'static {
    /// Enter the main loop. This typically does not return.
    fn run(&mut self, world: &mut World, schedule: &mut Schedule);
}

// ---------------------------------------------------------------------------
// HeadlessRunner – runs a fixed number of frames (useful for tests/CI)
// ---------------------------------------------------------------------------

/// A runner that executes a fixed number of frames then returns.
pub struct HeadlessRunner {
    /// Number of frames to execute.
    pub frame_count: u64,
    /// Simulated delta time per frame (seconds).
    pub simulated_delta: f32,
}

impl HeadlessRunner {
    /// Create a headless runner that runs `frame_count` frames at the given
    /// simulated delta time.
    pub fn new(frame_count: u64, simulated_delta: f32) -> Self {
        Self {
            frame_count,
            simulated_delta,
        }
    }
}

impl Default for HeadlessRunner {
    fn default() -> Self {
        Self {
            frame_count: 1,
            simulated_delta: 1.0 / 60.0,
        }
    }
}

impl Runner for HeadlessRunner {
    fn run(&mut self, world: &mut World, schedule: &mut Schedule) {
        for _ in 0..self.frame_count {
            // Update time resource.
            {
                let time = world.get_resource_mut::<Time>();
                time.update(self.simulated_delta);
            }

            // Run the schedule (startup + all runtime stages).
            schedule.run(world);
        }
    }
}

// ---------------------------------------------------------------------------
// NativeRunner – real-time loop with wall-clock delta (no winit dependency)
// ---------------------------------------------------------------------------

/// A native runner that uses wall-clock time. Runs until `should_quit` is set.
///
/// This is a simple polling loop without a windowing library. For actual
/// windowed applications, a winit-based runner would replace this.
pub struct NativeRunner {
    /// Maximum frames to run (0 = unlimited).
    pub max_frames: u64,
}

impl NativeRunner {
    pub fn new() -> Self {
        Self { max_frames: 0 }
    }

    pub fn with_max_frames(max_frames: u64) -> Self {
        Self { max_frames }
    }
}

impl Default for NativeRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl Runner for NativeRunner {
    fn run(&mut self, world: &mut World, schedule: &mut Schedule) {
        let mut last_instant = std::time::Instant::now();
        let mut frames_run: u64 = 0;

        loop {
            let now = std::time::Instant::now();
            let raw_delta = now.duration_since(last_instant).as_secs_f32();
            last_instant = now;

            // Update time resource.
            {
                let time = world.get_resource_mut::<Time>();
                time.update(raw_delta);
            }

            schedule.run(world);
            frames_run += 1;

            if self.max_frames > 0 && frames_run >= self.max_frames {
                break;
            }

            // Check quit flag.
            if world.has_resource::<AppExit>() {
                break;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// AppExit – resource marker to signal the runner to stop
// ---------------------------------------------------------------------------

/// Insert this resource to signal the runner to exit the main loop.
pub struct AppExit;

// ---------------------------------------------------------------------------
// WindowedRunner – winit event loop integration (requires "windowed" feature)
// ---------------------------------------------------------------------------

#[cfg(feature = "windowed")]
mod windowed {
    use super::*;
    use arachne_input::{InputSystem, PlatformInput};
    use arachne_render::{RenderContext, RenderFrame, SpriteRenderer, TextRenderer, TilemapRenderer};
    use arachne_window::{ArachneWindow, WindowConfig};
    use wgpu::util::DeviceExt;
    use crate::systems::{
        ScreenTextBuffer, SpriteRendererResource, TextRendererResource,
        TextureStorageResource, TilemapRendererResource,
    };
    use std::time::Instant;
    use winit::application::ApplicationHandler;
    use winit::event::WindowEvent;
    use winit::event_loop::ActiveEventLoop;
    use winit::window::WindowId;

    /// Wrapper around [`RenderContext`] that is Send + Sync for use as an ECS resource.
    ///
    /// The inner `RenderContext` contains a `wgpu::Surface` which is `!Send` on some
    /// backends. Access is always single-threaded via `ResMut` in the ECS schedule.
    pub struct RenderContextResource(pub RenderContext);
    // SAFETY: RenderContextResource is only accessed via &mut through ResMut in a
    // single-threaded schedule. The wgpu handles are thread-safe in practice.
    unsafe impl Send for RenderContextResource {}
    unsafe impl Sync for RenderContextResource {}

    /// A runner that drives the application through a real winit event loop.
    ///
    /// Creates a platform window, initialises GPU rendering, and bridges input
    /// events from winit into the ECS [`InputSystem`].
    pub struct WindowedRunner {
        /// Window configuration (title, size, vsync, etc.).
        pub config: WindowConfig,
        /// Target frames per second for frame-rate capping (default 60).
        pub target_fps: u32,
    }

    impl WindowedRunner {
        /// Create a new windowed runner with the given window configuration.
        pub fn new(config: WindowConfig) -> Self {
            Self {
                config,
                target_fps: 60,
            }
        }

        /// Set the target frames per second (builder pattern).
        pub fn with_target_fps(mut self, fps: u32) -> Self {
            self.target_fps = fps;
            self
        }
    }

    impl Runner for WindowedRunner {
        fn run(&mut self, world: &mut World, schedule: &mut Schedule) {
            let event_loop = arachne_window::create_event_loop();
            let frame_budget = std::time::Duration::from_secs_f64(1.0 / self.target_fps as f64);

            let mut app_state = AppState {
                world,
                schedule,
                config: self.config.clone(),
                frame_budget,
                window: None,
                last_instant: Instant::now(),
            };

            let _ = event_loop.run_app(&mut app_state);

            // Clean up GPU resources that reference the window before it drops.
            // The window lives in AppState and will be dropped after this function
            // returns, so we must remove any Surface-backed resources first.
            app_state.world.remove_resource::<TilemapRendererResource>();
            app_state.world.remove_resource::<TextRendererResource>();
            app_state.world.remove_resource::<TextureStorageResource>();
            app_state.world.remove_resource::<RenderContextResource>();
            app_state.world.remove_resource::<SpriteRendererResource>();
        }
    }

    /// Internal application state that implements winit's [`ApplicationHandler`].
    struct AppState<'a> {
        world: &'a mut World,
        schedule: &'a mut Schedule,
        config: WindowConfig,
        frame_budget: std::time::Duration,
        window: Option<ArachneWindow>,
        last_instant: Instant,
    }

    impl AppState<'_> {
        fn render_frame(&mut self) {
            // Upload camera matrix to GPU.
            if self.world.has_resource::<GpuResources>()
                && self.world.has_resource::<arachne_render::Camera2d>()
                && self.world.has_resource::<RenderContextResource>()
            {
                let cam = self.world.get_resource::<arachne_render::Camera2d>();
                let vp = cam.view_projection();
                let uniform = arachne_render::CameraUniform::from_mat4(&vp);
                let gpu = self.world.get_resource::<GpuResources>();
                let ctx = self.world.get_resource::<RenderContextResource>();
                ctx.0.queue().write_buffer(
                    &gpu.camera_buffer,
                    0,
                    bytemuck::bytes_of(&uniform),
                );
            }

            // Acquire surface texture, render sprites, present.
            if !self.world.has_resource::<RenderContextResource>() {
                return;
            }

            let ctx = self.world.get_resource_mut::<RenderContextResource>();
            let mut frame = match RenderFrame::begin(&mut ctx.0) {
                Some(f) => f,
                None => return,
            };
            let queue = ctx.0.queue().clone();

            // Track whether we've already cleared the surface this frame.
            let mut surface_cleared = false;

            // Render tilemaps BEFORE sprites (background layer).
            if self.world.has_resource::<TilemapRendererResource>()
                && self.world.has_resource::<TilemapPipelineResource>()
                && self.world.has_resource::<GpuResources>()
                && self.world.has_resource::<TextureStorageResource>()
            {
                let tilemap_res = self.world.get_resource::<TilemapRendererResource>();
                let tilemap_pipeline = self.world.get_resource::<TilemapPipelineResource>();
                let gpu = self.world.get_resource::<GpuResources>();
                let tex_store = self.world.get_resource::<TextureStorageResource>();

                if tilemap_res.last_prepared.index_count > 0 {
                    let atlas_bg = tex_store.0.get_bind_group(tilemap_res.atlas_texture);
                    {
                        let mut pass = frame.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("tilemap_pass"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &frame.surface_view,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color {
                                        r: 0.1, g: 0.1, b: 0.15, a: 1.0,
                                    }),
                                    store: wgpu::StoreOp::Store,
                                },
                            })],
                            depth_stencil_attachment: None,
                            ..Default::default()
                        });

                        tilemap_res.renderer.render(
                            &mut pass,
                            &tilemap_res.last_prepared,
                            &tilemap_pipeline.0,
                            &gpu.camera_bind_group,
                            atlas_bg,
                        );
                    }
                    surface_cleared = true;
                }
            }

            // Render sprites on top of tilemaps.
            if self.world.has_resource::<SpriteRendererResource>()
                && self.world.has_resource::<SpritePipelineResource>()
                && self.world.has_resource::<GpuResources>()
                && self.world.has_resource::<TextureStorageResource>()
            {
                let srr = self.world.get_resource::<SpriteRendererResource>();
                let pipeline_res = self.world.get_resource::<SpritePipelineResource>();
                let gpu = self.world.get_resource::<GpuResources>();
                let tex_store = self.world.get_resource::<TextureStorageResource>();

                // Use LoadOp::Load if tilemap already cleared, otherwise clear here.
                let load_op = if surface_cleared {
                    wgpu::LoadOp::Load
                } else {
                    wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.1, g: 0.1, b: 0.15, a: 1.0,
                    })
                };

                {
                    let mut pass = frame.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("sprite_pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &frame.surface_view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: load_op,
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        ..Default::default()
                    });

                    if !srr.last_batches.is_empty() {
                        pass.set_pipeline(&pipeline_res.0);
                        pass.set_bind_group(0, Some(&gpu.camera_bind_group), &[]);
                        pass.set_vertex_buffer(0, srr.renderer.quad_vertex_buffer().slice(..));
                        pass.set_vertex_buffer(1, srr.renderer.instance_buffer_slice());
                        pass.set_index_buffer(srr.renderer.quad_index_buffer().slice(..), wgpu::IndexFormat::Uint16);

                        for batch in &srr.last_batches {
                            // Look up the correct bind group for this texture handle.
                            let tex_handle = batch.texture;
                            let bg = if (tex_handle.0 as usize) < tex_store.0.count() {
                                tex_store.0.get_bind_group(tex_handle)
                            } else {
                                // Fallback to handle 0 (white texture) for unknown handles.
                                tex_store.0.get_bind_group(arachne_render::TextureHandle(0))
                            };
                            pass.set_bind_group(1, Some(bg), &[]);
                            pass.draw_indexed(
                                0..6,
                                0,
                                batch.instance_offset..batch.instance_offset + batch.instance_count,
                            );
                        }
                    }
                }
            }

            // Render text on top of sprites using the text SDF pipeline.
            if self.world.has_resource::<TextRendererResource>()
                && self.world.has_resource::<RenderContextResource>()
            {
                let ctx = self.world.get_resource::<RenderContextResource>();
                let device = ctx.0.device().clone();

                let text_res = self.world.get_resource_mut::<TextRendererResource>();
                let prepared = text_res.renderer.prepare(&device, &queue);
                text_res.last_prepared = arachne_render::TextPrepared {
                    vertex_count: prepared.vertex_count,
                    index_count: prepared.index_count,
                };

                if prepared.index_count > 0 {
                    frame.render_text(
                        &text_res.pipeline,
                        &text_res.camera_bind_group,
                        &text_res.font_bind_group,
                        &text_res.renderer,
                        &prepared,
                    );
                }
            }

            frame.present(&queue);
        }
    }

    impl ApplicationHandler for AppState<'_> {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            // Create the platform window.
            let window = ArachneWindow::new(event_loop, &self.config);
            let (w, h) = window.inner_size();

            // Create GPU render context bound to the window.
            // SAFETY: The window is stored in self.window and will outlive the
            // RenderContext (which is removed from the world before the window drops).
            let context = pollster::block_on(
                RenderContext::new_with_window(&window, w, h),
            )
            .expect("failed to create render context");

            // Create sprite pipeline and renderer.
            let format = context.surface_format();
            let sprite_pipeline = arachne_render::create_sprite_pipeline(context.device(), format);
            let sprite_renderer = SpriteRenderer::new(context.device());

            let srr = SpriteRendererResource {
                renderer: sprite_renderer,
                device: context.device().clone(),
                queue: context.queue().clone(),
                last_batches: Vec::new(),
            };

            // Create TextureStorage with a 1x1 white fallback at handle 0.
            let mut tex_storage = arachne_render::TextureStorage::new(context.device());
            let _fallback_handle = tex_storage.create_texture(
                context.device(), context.queue(), 1, 1, &[255u8, 255, 255, 255],
            );
            // handle 0 = white fallback, used by sprites without loaded textures.
            self.world.insert_resource(TextureStorageResource(tex_storage));

            // Create camera uniform buffer and bind group.
            let camera_buf = context.device().create_buffer(&wgpu::BufferDescriptor {
                label: Some("camera_uniform"),
                size: 64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let cam_bgl = sprite_pipeline.get_bind_group_layout(0);
            let camera_bg = arachne_render::create_camera_bind_group(
                context.device(), &cam_bgl, &camera_buf,
            );

            self.world.insert_resource(GpuResources {
                camera_buffer: camera_buf,
                camera_bind_group: camera_bg,
            });

            // Insert resources into the ECS world.
            self.world.insert_resource(RenderContextResource(context));
            self.world.insert_resource(srr);

            if !self.world.has_resource::<InputSystem>() {
                let input = InputSystem::new()
                    .with_window_size(w as f32, h as f32)
                    .with_dpi_scale(window.scale_factor() as f32);
                self.world.insert_resource(input);
            }

            // Store the pipeline as a resource for the render step.
            self.world.insert_resource(SpritePipelineResource(sprite_pipeline));

            // Create tilemap pipeline, renderer, and built-in tile atlas.
            // Split into two scoped borrows to avoid overlapping immutable
            // (RenderContextResource) and mutable (TextureStorageResource) borrows.
            let (tilemap_pipeline, tilemap_renderer, atlas_rgba, atlas_w, atlas_h, dev_clone, q_clone) = {
                let ctx = self.world.get_resource::<RenderContextResource>();
                let device = ctx.0.device();
                let queue_ref = ctx.0.queue();
                let format = ctx.0.surface_format();

                let pipeline = arachne_render::create_tilemap_pipeline(device, format);
                let renderer = TilemapRenderer::new(device);

                let (rgba, w, h, _tile_cols, _tile_rows) =
                    arachne_render::generate_builtin_tiles();

                (pipeline, renderer, rgba, w, h, device.clone(), queue_ref.clone())
            };
            {
                let tex_store = self.world.get_resource_mut::<TextureStorageResource>();
                let atlas_handle = tex_store.create_from_rgba(
                    &dev_clone, &q_clone, atlas_w, atlas_h, &atlas_rgba,
                );

                let tilemap_res = TilemapRendererResource {
                    renderer: tilemap_renderer,
                    layers: Vec::new(),
                    atlas_texture: atlas_handle,
                    last_prepared: arachne_render::TilemapPrepared {
                        vertex_count: 0,
                        index_count: 0,
                    },
                    device: dev_clone,
                    queue: q_clone,
                };

                self.world.insert_resource(tilemap_res);
                self.world.insert_resource(TilemapPipelineResource(tilemap_pipeline));
            }

            // Create text rendering pipeline and built-in font.
            {
                let ctx = self.world.get_resource::<RenderContextResource>();
                let device = ctx.0.device();
                let queue_ref = ctx.0.queue();
                let format = ctx.0.surface_format();

                let text_pipeline = arachne_render::create_text_pipeline(device, format);

                // Generate the built-in bitmap font atlas.
                let (atlas_rgba, font) = arachne_render::generate_builtin_font();
                let atlas_w = arachne_render::builtin_font::ATLAS_W;
                let atlas_h = arachne_render::builtin_font::ATLAS_H;

                // Upload font atlas as an R8Unorm texture (the SDF shader samples .r).
                // Convert RGBA to single-channel R8: take the alpha channel.
                let mut r8_data = vec![0u8; (atlas_w * atlas_h) as usize];
                for i in 0..(atlas_w * atlas_h) as usize {
                    r8_data[i] = atlas_rgba[i * 4 + 3]; // alpha -> R
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
                    usage: wgpu::TextureUsages::TEXTURE_BINDING
                        | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });

                queue_ref.write_texture(
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

                let atlas_view = atlas_texture.create_view(
                    &wgpu::TextureViewDescriptor::default(),
                );
                let atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                    label: Some("font_sampler"),
                    mag_filter: wgpu::FilterMode::Nearest,
                    min_filter: wgpu::FilterMode::Nearest,
                    ..Default::default()
                });

                // Create bind groups matching the text SDF pipeline layout.
                // Group 0: camera uniform (binding 0) + text params (binding 1)
                let text_params = arachne_render::TextParams {
                    edge_softness: 0.05,
                    outline_width: 0.0,
                    ..Default::default()
                };
                let text_params_buffer = device.create_buffer_init(
                    &wgpu::util::BufferInitDescriptor {
                        label: Some("text_params"),
                        contents: bytemuck::bytes_of(&text_params),
                        usage: wgpu::BufferUsages::UNIFORM
                            | wgpu::BufferUsages::COPY_DST,
                    },
                );

                let gpu = self.world.get_resource::<GpuResources>();
                let text_camera_bgl = text_pipeline.get_bind_group_layout(0);
                let text_camera_bg = device.create_bind_group(
                    &wgpu::BindGroupDescriptor {
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
                    },
                );

                let text_font_bgl = text_pipeline.get_bind_group_layout(1);
                let text_font_bg = device.create_bind_group(
                    &wgpu::BindGroupDescriptor {
                        label: Some("text_font_bg"),
                        layout: &text_font_bgl,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: wgpu::BindingResource::TextureView(
                                    &atlas_view,
                                ),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::Sampler(
                                    &atlas_sampler,
                                ),
                            },
                        ],
                    },
                );

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

                self.world.insert_resource(text_res);
            }

            // Insert the ScreenTextBuffer if not already present.
            if !self.world.has_resource::<ScreenTextBuffer>() {
                self.world.insert_resource(ScreenTextBuffer::default());
            }

            // Update Camera2d viewport to match the actual window size.
            if self.world.has_resource::<arachne_render::Camera2d>() {
                let cam = self.world.get_resource_mut::<arachne_render::Camera2d>();
                cam.viewport_size = arachne_math::Vec2::new(w as f32, h as f32);
            }

            self.last_instant = Instant::now();
            window.request_redraw();
            self.window = Some(window);
        }

        fn window_event(
            &mut self,
            event_loop: &ActiveEventLoop,
            _window_id: WindowId,
            event: WindowEvent,
        ) {
            match event {
                WindowEvent::CloseRequested => {
                    self.world.insert_resource(AppExit);
                    event_loop.exit();
                }

                WindowEvent::Resized(size) => {
                    if size.width > 0 && size.height > 0 {
                        if self.world.has_resource::<RenderContextResource>() {
                            let ctx = self.world.get_resource_mut::<RenderContextResource>();
                            ctx.0.resize(size.width, size.height);
                        }
                        if self.world.has_resource::<InputSystem>() {
                            let input = self.world.get_resource_mut::<InputSystem>();
                            input.window_size = arachne_math::Vec2::new(
                                size.width as f32,
                                size.height as f32,
                            );
                        }
                    }
                }

                WindowEvent::RedrawRequested => {
                    // Frame timing.
                    let now = Instant::now();
                    let raw_delta = now.duration_since(self.last_instant).as_secs_f32();
                    self.last_instant = now;

                    // Update Time resource.
                    {
                        let time = self.world.get_resource_mut::<Time>();
                        time.update(raw_delta);
                    }

                    // Transition input states for the new frame.
                    if self.world.has_resource::<InputSystem>() {
                        let input = self.world.get_resource_mut::<InputSystem>();
                        input.begin_frame();
                    }

                    // Run all ECS systems (game logic, physics, sprite preparation, etc.).
                    self.schedule.run(self.world);

                    // GPU rendering: acquire frame, draw sprites, present.
                    self.render_frame();

                    // Frame-rate cap: sleep if we finished early.
                    let frame_time = now.elapsed();
                    if frame_time < self.frame_budget {
                        std::thread::sleep(self.frame_budget - frame_time);
                    }

                    // Request next frame.
                    if let Some(ref window) = self.window {
                        window.request_redraw();
                    }

                    // Check quit flag set by a system.
                    if self.world.has_resource::<AppExit>() {
                        event_loop.exit();
                    }
                }

                // Delegate input events to the winit bridge.
                ref input_event => {
                    if self.world.has_resource::<InputSystem>() {
                        let input = self.world.get_resource_mut::<InputSystem>();
                        arachne_input::winit_bridge::process_window_event(input, input_event);
                    }
                }
            }
        }
    }

    /// ECS resource wrapping the sprite render pipeline.
    pub struct SpritePipelineResource(pub wgpu::RenderPipeline);
    unsafe impl Send for SpritePipelineResource {}
    unsafe impl Sync for SpritePipelineResource {}

    /// ECS resource wrapping the tilemap render pipeline.
    pub struct TilemapPipelineResource(pub wgpu::RenderPipeline);
    unsafe impl Send for TilemapPipelineResource {}
    unsafe impl Sync for TilemapPipelineResource {}

    /// GPU resources needed for rendering (camera uniform).
    pub struct GpuResources {
        pub camera_buffer: wgpu::Buffer,
        pub camera_bind_group: wgpu::BindGroup,
    }
    unsafe impl Send for GpuResources {}
    unsafe impl Sync for GpuResources {}

    // -----------------------------------------------------------------------
    // Tests
    // -----------------------------------------------------------------------

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn windowed_runner_default_target_fps() {
            let runner = WindowedRunner::new(WindowConfig::default());
            assert_eq!(runner.target_fps, 60);
        }

        #[test]
        fn windowed_runner_with_target_fps() {
            let runner = WindowedRunner::new(WindowConfig::default()).with_target_fps(120);
            assert_eq!(runner.target_fps, 120);
        }

        #[test]
        fn frame_budget_60fps() {
            let budget = std::time::Duration::from_secs_f64(1.0 / 60.0);
            let expected_ms = 1000.0 / 60.0;
            let actual_ms = budget.as_secs_f64() * 1000.0;
            assert!(
                (actual_ms - expected_ms).abs() < 0.01,
                "60fps budget: {:.4}ms expected {:.4}ms",
                actual_ms,
                expected_ms,
            );
        }
    }
}

#[cfg(feature = "windowed")]
pub use windowed::{RenderContextResource, SpritePipelineResource, TilemapPipelineResource, WindowedRunner};
