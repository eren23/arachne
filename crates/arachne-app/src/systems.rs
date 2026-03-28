//! Built-in systems wired by plugins and DefaultPlugins.
//!
//! Each system is a plain function with ECS system-parameter signatures so it
//! can be registered directly via `Schedule::add_system`.

use arachne_ecs::{Query, Res, ResMut};
use arachne_input::PlatformInput;
use arachne_math::{Quat, Transform, Vec3};

use crate::components::{GlobalTransform, PhysicsBody, PhysicsBodyState};
use crate::time::Time;

// ---------------------------------------------------------------------------
// Input
// ---------------------------------------------------------------------------

/// Calls `begin_frame()` on the InputSystem resource to transition key states.
pub fn input_update_system(mut input: ResMut<arachne_input::InputSystem>) {
    input.begin_frame();
}

// ---------------------------------------------------------------------------
// Asset
// ---------------------------------------------------------------------------

/// Wrapper around AssetServer that is Send + Sync.
/// AssetServer contains a `Receiver` which is `!Sync`, so we wrap it to allow
/// use as an ECS resource. Access is always single-threaded via `ResMut`.
pub struct AssetServerResource(pub arachne_asset::AssetServer);

// SAFETY: AssetServerResource is only accessed via &mut through ResMut in a
// single-threaded schedule. The Receiver inside is never shared across threads.
unsafe impl Sync for AssetServerResource {}
unsafe impl Send for AssetServerResource {}

/// Polls the asset server to process background-loaded assets.
pub fn asset_poll_system(mut server: ResMut<AssetServerResource>) {
    server.0.poll();
}

// ---------------------------------------------------------------------------
// Physics
// ---------------------------------------------------------------------------

/// Steps the physics world using the frame delta from the Time resource.
/// PhysicsWorld internally uses a fixed-timestep accumulator.
pub fn physics_step_system(time: Res<Time>, mut physics: ResMut<arachne_physics::PhysicsWorld>) {
    let dt = time.delta_seconds();
    if dt > 0.0 {
        physics.update(dt);
    }
}

/// Syncs physics body positions/rotations back into ECS Transform components.
pub fn physics_sync_system(
    physics: Res<arachne_physics::PhysicsWorld>,
    mut query: Query<(&PhysicsBody, &mut Transform)>,
) {
    for (body_comp, transform) in query.iter_mut() {
        if let PhysicsBodyState::Active(handle) = body_comp.state {
            if let Some(body) = physics.bodies.get(handle.0 as usize) {
                transform.position.x = body.position.x;
                transform.position.y = body.position.y;
                // 2D rotation around Z axis.
                transform.rotation = Quat::from_axis_angle(Vec3::Z, body.rotation);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Audio
// ---------------------------------------------------------------------------

/// Wrapper around AudioMixer that is an ECS resource.
pub struct AudioMixerResource(pub arachne_audio::AudioMixer);
unsafe impl Sync for AudioMixerResource {}
unsafe impl Send for AudioMixerResource {}

/// Placeholder audio update: mix a silent buffer to advance fades/envelopes.
pub fn audio_update_system(mut mixer: ResMut<AudioMixerResource>) {
    let mut buf = [0.0f32; 512];
    mixer.0.mix(&mut buf);
}

// ---------------------------------------------------------------------------
// Transform propagation
// ---------------------------------------------------------------------------

/// Propagates parent transforms to children by computing GlobalTransform.
///
/// For entities that have a `Transform` but no `GlobalTransform`, this system
/// simply copies Transform into GlobalTransform. A full hierarchy system
/// would walk the parent chain, but since the ECS doesn't have a built-in
/// Parent component, we do a flat copy for now.
pub fn transform_propagation_system(
    mut query: Query<(&Transform, &mut GlobalTransform)>,
) {
    for (local, global) in query.iter_mut() {
        global.0 = *local;
    }
}

// ---------------------------------------------------------------------------
// Camera
// ---------------------------------------------------------------------------

/// Updates the Camera2d resource from a Camera + Transform entity.
/// This is a minimal wiring — a real engine would extract the camera entity.
pub fn camera_update_system(
    query: Query<(&crate::components::Camera, &Transform)>,
    mut cam2d: ResMut<arachne_render::Camera2d>,
) {
    for (cam, transform) in query.iter() {
        cam2d.position.x = transform.position.x;
        cam2d.position.y = transform.position.y;
        // Extract Z-rotation angle from quaternion for 2D camera rotation.
        let (_, _, z_angle) = quat_to_euler(transform.rotation);
        cam2d.rotation = z_angle;
        cam2d.zoom = cam.zoom;
    }
}

/// Minimal quaternion → Euler extraction (Z-axis rotation only).
fn quat_to_euler(q: Quat) -> (f32, f32, f32) {
    let sinr_cosp = 2.0 * (q.w * q.x + q.y * q.z);
    let cosr_cosp = 1.0 - 2.0 * (q.x * q.x + q.y * q.y);
    let roll = sinr_cosp.atan2(cosr_cosp);

    let sinp = 2.0 * (q.w * q.y - q.z * q.x);
    let pitch = if sinp.abs() >= 1.0 {
        std::f32::consts::FRAC_PI_2.copysign(sinp)
    } else {
        sinp.asin()
    };

    let siny_cosp = 2.0 * (q.w * q.z + q.x * q.y);
    let cosy_cosp = 1.0 - 2.0 * (q.y * q.y + q.z * q.z);
    let yaw = siny_cosp.atan2(cosy_cosp);

    (roll, pitch, yaw)
}

// ---------------------------------------------------------------------------
// Texture storage resource
// ---------------------------------------------------------------------------

/// Wrapper around [`TextureStorage`](arachne_render::TextureStorage) that is
/// Send + Sync for use as an ECS resource.
pub struct TextureStorageResource(pub arachne_render::TextureStorage);
unsafe impl Sync for TextureStorageResource {}
unsafe impl Send for TextureStorageResource {}

impl TextureStorageResource {
    /// Load a PNG file from disk and register it as a GPU texture.
    /// Returns the TextureHandle on success.
    pub fn load_png(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        path: &str,
    ) -> Result<arachne_render::TextureHandle, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("read {}: {}", path, e))?;
        let img = arachne_asset::Image::decode_png(&bytes)?;
        Ok(self.0.create_texture(device, queue, img.width, img.height, &img.data))
    }

    /// Load a PNG from raw bytes and register it as a GPU texture.
    /// Useful for WASM where textures are fetched as byte arrays.
    pub fn load_png_bytes(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bytes: &[u8],
    ) -> Result<arachne_render::TextureHandle, String> {
        let img = arachne_asset::Image::decode_png(bytes)?;
        Ok(self.0.create_texture(device, queue, img.width, img.height, &img.data))
    }

    /// Create a procedural texture from RGBA8 data.
    pub fn create_from_rgba(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        data: &[u8],
    ) -> arachne_render::TextureHandle {
        self.0.create_texture(device, queue, width, height, data)
    }

    /// Generate a checkerboard pattern texture (useful for testing).
    pub fn create_checkerboard(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        size: u32,
        cell_size: u32,
        color_a: [u8; 4],
        color_b: [u8; 4],
    ) -> arachne_render::TextureHandle {
        let mut data = Vec::with_capacity((size * size * 4) as usize);
        for y in 0..size {
            for x in 0..size {
                let cell = ((x / cell_size) + (y / cell_size)) % 2;
                let c = if cell == 0 { color_a } else { color_b };
                data.extend_from_slice(&c);
            }
        }
        self.0.create_texture(device, queue, size, size, &data)
    }

    /// Generate a circle texture with the given color (antialiased).
    pub fn create_circle(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        size: u32,
        color: [u8; 4],
    ) -> arachne_render::TextureHandle {
        let mut data = Vec::with_capacity((size * size * 4) as usize);
        let center = size as f32 / 2.0;
        let radius = center - 1.0;
        for y in 0..size {
            for x in 0..size {
                let dx = x as f32 - center + 0.5;
                let dy = y as f32 - center + 0.5;
                let dist = (dx * dx + dy * dy).sqrt();
                let alpha = ((radius - dist + 0.5).clamp(0.0, 1.0) * color[3] as f32) as u8;
                data.extend_from_slice(&[color[0], color[1], color[2], alpha]);
            }
        }
        self.0.create_texture(device, queue, size, size, &data)
    }
}

// ---------------------------------------------------------------------------
// Text rendering integration
// ---------------------------------------------------------------------------

/// A single text draw request: position, text, size, color.
#[derive(Clone, Debug)]
pub struct TextDrawRequest {
    pub text: String,
    pub position: arachne_math::Vec2,
    pub font_size: f32,
    pub color: arachne_math::Color,
    pub max_width: Option<f32>,
}

/// Buffer of text draw requests accumulated each frame.
///
/// Systems push text here; the windowed runner drains and renders them.
#[derive(Default)]
pub struct ScreenTextBuffer {
    pub requests: Vec<TextDrawRequest>,
}

impl ScreenTextBuffer {
    pub fn draw(
        &mut self,
        text: impl Into<String>,
        position: arachne_math::Vec2,
        font_size: f32,
        color: arachne_math::Color,
    ) {
        self.requests.push(TextDrawRequest {
            text: text.into(),
            position,
            font_size,
            color,
            max_width: None,
        });
    }

    pub fn draw_wrapped(
        &mut self,
        text: impl Into<String>,
        position: arachne_math::Vec2,
        font_size: f32,
        color: arachne_math::Color,
        max_width: f32,
    ) {
        self.requests.push(TextDrawRequest {
            text: text.into(),
            position,
            font_size,
            color,
            max_width: Some(max_width),
        });
    }

    pub fn clear(&mut self) {
        self.requests.clear();
    }
}

/// Wrapper around [`TextRenderer`](arachne_render::TextRenderer) that is
/// Send + Sync for use as an ECS resource. Includes the built-in font,
/// the GPU text pipeline, and bind groups needed for rendering.
pub struct TextRendererResource {
    pub renderer: arachne_render::TextRenderer,
    pub font: arachne_render::BmFont,
    pub pipeline: wgpu::RenderPipeline,
    pub camera_bind_group: wgpu::BindGroup,
    pub font_bind_group: wgpu::BindGroup,
    pub text_params_buffer: wgpu::Buffer,
    pub last_prepared: arachne_render::TextPrepared,
}

// SAFETY: TextRendererResource is only accessed via &mut through ResMut in a
// single-threaded schedule. The wgpu handles are thread-safe.
unsafe impl Sync for TextRendererResource {}
unsafe impl Send for TextRendererResource {}

/// System that processes [`ScreenTextBuffer`] requests into GPU vertex data
/// via [`TextRendererResource`].
pub fn text_render_system(
    mut text_buf: ResMut<ScreenTextBuffer>,
    mut text_res: Option<ResMut<TextRendererResource>>,
) {
    if let Some(ref mut res) = text_res {
        res.renderer.begin_frame();
        // Clone font to avoid simultaneous borrow of res.renderer and res.font.
        let font = res.font.clone();
        for req in &text_buf.requests {
            res.renderer.draw_text(
                &font,
                &req.text,
                req.position,
                req.font_size,
                req.color,
                req.max_width,
            );
        }
        // We need device + queue for prepare, but we do that in the runner
        // just before rendering (same pattern as sprites). Mark as needing
        // prepare by leaving the vertices in the renderer.
    }
    text_buf.clear();
}

// ---------------------------------------------------------------------------
// Tilemap rendering integration
// ---------------------------------------------------------------------------

/// Wrapper around [`TilemapRenderer`](arachne_render::TilemapRenderer) that is
/// Send + Sync for use as an ECS resource. Stores the renderer, layers, atlas
/// texture handle, and last prepared data for the windowed runner to draw.
pub struct TilemapRendererResource {
    pub renderer: arachne_render::TilemapRenderer,
    pub layers: Vec<arachne_render::TilemapLayer>,
    pub atlas_texture: arachne_render::TextureHandle,
    pub last_prepared: arachne_render::render2d::tilemap::TilemapPrepared,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

// SAFETY: TilemapRendererResource is only accessed via &mut through ResMut in a
// single-threaded schedule. The wgpu handles are thread-safe.
unsafe impl Sync for TilemapRendererResource {}
unsafe impl Send for TilemapRendererResource {}

/// System that builds tilemap geometry and uploads it to the GPU each frame.
pub fn tilemap_render_system(
    mut tilemap_res: Option<ResMut<TilemapRendererResource>>,
) {
    if let Some(ref mut res) = tilemap_res {
        let TilemapRendererResource {
            ref mut renderer,
            ref layers,
            ref device,
            ref queue,
            ref mut last_prepared,
            ..
        } = **res;

        renderer.begin_frame();
        for layer in layers {
            renderer.build_layer(layer);
        }
        *last_prepared = renderer.prepare(device, queue);
    }
}

// ---------------------------------------------------------------------------
// Sprite rendering integration
// ---------------------------------------------------------------------------

/// Wrapper around [`SpriteRenderer`](arachne_render::SpriteRenderer) that is
/// Send + Sync for use as an ECS resource. Includes the GPU device and queue
/// needed by [`SpriteRenderer::prepare`], and stores the last prepared batches
/// so the windowed runner can draw them.
pub struct SpriteRendererResource {
    pub renderer: arachne_render::SpriteRenderer,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub last_batches: Vec<arachne_render::SpriteBatch>,
}

// SAFETY: SpriteRendererResource is only accessed via &mut through ResMut in a
// single-threaded schedule. The wgpu handles are thread-safe.
unsafe impl Sync for SpriteRendererResource {}
unsafe impl Send for SpriteRendererResource {}

/// Renders sprites via [`SpriteRenderer`](arachne_render::SpriteRenderer) when
/// a [`SpriteRendererResource`] is available, otherwise falls back to counting
/// drawable entities for headless/test use.
pub fn sprite_render_system(
    query: Query<(&arachne_render::Sprite, &Transform)>,
    mut draw_count: ResMut<crate::components::DrawCallCount>,
    mut renderer_res: Option<ResMut<SpriteRendererResource>>,
) {
    if let Some(ref mut res) = renderer_res {
        let SpriteRendererResource {
            ref mut renderer,
            ref device,
            ref queue,
            ..
        } = **res;

        renderer.begin_frame();

        let mut sprite_count = 0u32;
        for (sprite, transform) in query.iter() {
            // Apply custom_size as scale on top of the transform.
            // Without this, sprites are 1x1 world-unit quads (invisible on
            // an 800x600 viewport).
            let size = sprite.custom_size.unwrap_or(arachne_math::Vec2::new(32.0, 32.0));
            let mut scaled = *transform;
            scaled.scale.x *= size.x;
            scaled.scale.y *= size.y;
            let model_matrix = scaled.local_to_world();
            let uv_rect = arachne_math::Rect::new(
                arachne_math::Vec2::ZERO,
                arachne_math::Vec2::ONE,
            );
            let depth = transform.position.z;
            renderer.draw(sprite, &model_matrix, uv_rect, depth);
            sprite_count += 1;
        }

        let (batches, stats) = renderer.prepare(device, queue);
        let _ = sprite_count;
        draw_count.0 = stats.draw_calls;
        res.last_batches = batches;
    } else {
        let mut count = 0u32;
        for (_sprite, _transform) in query.iter() {
            count += 1;
        }
        draw_count.0 = count;
    }
}
