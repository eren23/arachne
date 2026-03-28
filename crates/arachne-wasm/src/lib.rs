//! arachne-wasm: WASM bindings and JS API layer for the Arachne engine.
//!
//! This crate provides the bridge between web browsers and the Arachne runtime.
//! It exposes a clean JS API via `wasm-bindgen` for embedding interactive content
//! in web pages with a single `<script>` tag + `<canvas>` element.
//!
//! When compiled on non-WASM targets, all web API calls are replaced with
//! stub/no-op implementations so the crate compiles and tests run on native.

pub mod canvas;
pub mod events;
pub mod audio_backend;
pub mod fetch;
pub mod api;
pub mod bindings;
pub mod canvas_runtime;

// Re-export the high-level JS API types.
pub use api::{ArachneApp, ArachneAppOptions, AppState};
pub use canvas::{CanvasConfig, CanvasHandle, DpiInfo};
pub use canvas_runtime::WasmRunner;
pub use events::{DomEvent, DomEventKind, EventTranslator};
pub use audio_backend::{WebAudioBackend, WebAudioConfig, WebAudioState};
pub use fetch::{FetchRequest, FetchResponse, FetchError, FetchProgress, AssetFetcher};
pub use bindings::{JsValueWrapper, TypeConverter};

// ---------------------------------------------------------------------------
// WASM entry point — boots the full engine with GPU rendering
// ---------------------------------------------------------------------------

/// Start the Arachne engine on a `<canvas>` element with the full GPU pipeline.
///
/// This is the primary entry point for WASM. It creates the App with default
/// plugins, sets up the WasmRunner targeting the canvas, and enters the
/// requestAnimationFrame loop.
///
/// Call from JS: `await init(); run("arachne-canvas");`
#[cfg(all(target_arch = "wasm32", feature = "wasm"))]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn run(canvas_id: &str) {
    use arachne_app::{
        App, Camera, Commands, DefaultPlugins, PhysicsBody, PhysicsBodyState,
        Physics2dPlugin, Plugin, Query, Res, ResMut, Runner, ScreenTextBuffer,
        Time, Transform, Vec2, Vec3, Color,
    };
    use arachne_input::{InputSystem, KeyCode, MouseButton};
    use arachne_math::Rng;
    use arachne_physics::{Collider, PhysicsWorld, RigidBodyData};
    use arachne_render::{Camera2d, Sprite, TextureHandle};

    /// Marks a dynamic physics body.
    #[derive(Clone, Copy)]
    struct DynBody;

    /// Playground state for spawn counter and drag tracking.
    struct PgState {
        rng: Rng,
        count: u32,
        drag: Option<Vec2>,
    }
    unsafe impl Send for PgState {}
    unsafe impl Sync for PgState {}

    fn hsv(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
        let c = v * s;
        let h2 = h / 60.0;
        let x = c * (1.0 - ((h2 % 2.0) - 1.0).abs());
        let (r, g, b) = match h2 as u32 {
            0 => (c, x, 0.0), 1 => (x, c, 0.0), 2 => (0.0, c, x),
            3 => (0.0, x, c), 4 => (x, 0.0, c), _ => (c, 0.0, x),
        };
        let m = v - c;
        (r + m, g + m, b + m)
    }

    fn setup(mut commands: Commands) {
        commands.spawn((Camera::new(), Transform::IDENTITY));
        commands.insert_resource(PgState {
            rng: Rng::seed(1234), count: 0, drag: None,
        });
    }

    fn setup_walls(mut physics: ResMut<PhysicsWorld>, mut commands: Commands) {
        // Screen Y: positive = down. Floor at bottom, ceiling at top.
        let walls = [
            (Vec2::new(0.0, 280.0), Vec2::new(400.0, 20.0)),   // floor (bottom)
            (Vec2::new(0.0, -280.0), Vec2::new(400.0, 20.0)),  // ceiling (top)
            (Vec2::new(-390.0, 0.0), Vec2::new(20.0, 300.0)),  // left wall
            (Vec2::new(390.0, 0.0), Vec2::new(20.0, 300.0)),   // right wall
        ];
        for (pos, half) in &walls {
            let body = RigidBodyData::new_static(*pos);
            let h = physics.add_body(body);
            physics.set_collider(h, Collider::aabb(*half));
            let mut spr = Sprite::new(TextureHandle(0));
            spr.color = Color::rgb(0.25, 0.25, 0.3);
            spr.custom_size = Some(Vec2::new(half.x * 2.0, half.y * 2.0));
            commands.spawn((spr, Transform::from_position(Vec3::new(pos.x, pos.y, 0.0))));
        }
    }

    fn spawn_initial(mut physics: ResMut<PhysicsWorld>, mut commands: Commands) {
        let mut rng = Rng::seed(9999);
        for i in 0..20u32 {
            let x = rng.next_range_f32(-300.0, 300.0);
            let y = rng.next_range_f32(-200.0, 0.0);
            let r = rng.next_range_f32(10.0, 25.0);
            let body = RigidBodyData::new_dynamic(Vec2::new(x, y), 1.0, 1.0);
            let h = physics.add_body(body);
            let is_circle = i % 2 == 0;
            if is_circle {
                physics.set_collider(h, Collider::circle(r));
            } else {
                physics.set_collider(h, Collider::aabb(Vec2::new(r, r)));
            }
            let mut pb = PhysicsBody::dynamic(1.0, 1.0);
            pb.state = PhysicsBodyState::Active(h);
            let hue = (i as f32 / 20.0) * 360.0;
            let (cr, cg, cb) = hsv(hue, 0.7, 1.0);
            let mut spr = Sprite::new(TextureHandle(0));
            spr.color = Color::rgb(cr, cg, cb);
            spr.custom_size = Some(Vec2::new(r * 2.0, r * 2.0));
            commands.spawn((DynBody, pb, spr, Transform::from_position(Vec3::new(x, y, 0.1))));
        }
    }

    fn click_drag(
        input: Res<InputSystem>, cam: Res<Camera2d>,
        mut physics: ResMut<PhysicsWorld>, mut st: ResMut<PgState>,
        mut commands: Commands,
    ) {
        let mpos = Vec2::new(input.mouse.position().x, input.mouse.position().y);
        let wpos = cam.screen_to_world(mpos);
        if input.mouse.just_pressed(MouseButton::Left) { st.drag = Some(wpos); }
        if input.mouse.just_released(MouseButton::Left) {
            let start = st.drag.take().unwrap_or(wpos);
            let vel = Vec2::new((wpos.x - start.x) * 3.0, (wpos.y - start.y) * 3.0);
            let is_circle = st.count % 2 == 0;
            let mut body = RigidBodyData::new_dynamic(start, 1.0, 1.0);
            body.linear_velocity = vel;
            let h = physics.add_body(body);
            let r = st.rng.next_range_f32(8.0, 22.0);
            if is_circle { physics.set_collider(h, Collider::circle(r)); }
            else { physics.set_collider(h, Collider::aabb(Vec2::new(r, r))); }
            let mut pb = PhysicsBody::dynamic(1.0, 1.0);
            pb.state = PhysicsBodyState::Active(h);
            let hue = (st.count as f32 * 30.0) % 360.0;
            let (cr, cg, cb) = hsv(hue, 0.8, 1.0);
            let mut spr = Sprite::new(TextureHandle(0));
            spr.color = Color::rgb(cr, cg, cb);
            spr.custom_size = Some(Vec2::new(r * 2.0, r * 2.0));
            commands.spawn((DynBody, pb, spr, Transform::from_position(Vec3::new(start.x, start.y, 0.1))));
            st.count += 1;
        }
    }

    fn color_vel(physics: Res<PhysicsWorld>, mut q: Query<(&DynBody, &PhysicsBody, &mut Sprite)>) {
        for (_, pb, spr) in q.iter_mut() {
            if let PhysicsBodyState::Active(h) = pb.state {
                if let Some(b) = physics.bodies.get(h.0 as usize) {
                    let t = (b.linear_velocity.length() / 500.0).min(1.0);
                    let hue = 240.0 * (1.0 - t);
                    let (r, g, b) = hsv(hue, 0.8, 1.0);
                    spr.color = Color::rgb(r, g, b);
                }
            }
        }
    }

    fn ui_overlay(time: Res<Time>, physics: Res<PhysicsWorld>, mut tb: ResMut<ScreenTextBuffer>) {
        let fps = if time.delta_seconds() > 0.0 { 1.0 / time.delta_seconds() } else { 0.0 };
        tb.draw(format!("FPS: {:.0}", fps), Vec2::new(10.0, 10.0), 16.0, Color::rgb(0.4, 1.0, 0.85));
        tb.draw(format!("Bodies: {}", physics.bodies.len()), Vec2::new(10.0, 30.0), 16.0, Color::rgb(0.4, 1.0, 0.85));
        tb.draw("Click-drag to throw | R = reset", Vec2::new(10.0, 50.0), 12.0, Color::rgb(0.5, 0.5, 0.6));
    }

    // Leak the App so World/Schedule live forever on the WASM heap.
    let app = Box::leak(Box::new(App::new()));
    app.add_plugin(DefaultPlugins);
    app.add_plugin(Physics2dPlugin);
    app.build_plugins();

    // Scale gravity for pixel coordinates (default -9.81 is too weak).
    {
        let physics = app.world.get_resource_mut::<PhysicsWorld>();
        physics.config.gravity = Vec2::new(0.0, 800.0);
    }

    app.add_startup_system(setup);
    app.add_startup_system(setup_walls);
    app.add_startup_system(spawn_initial);
    app.add_system(click_drag);
    app.add_system(color_vel);
    app.add_system(ui_overlay);

    let mut runner = WasmRunner::with_canvas_id(canvas_id);
    runner.run(&mut app.world, &mut app.schedule);
}

/// No-op on native targets so the function signature exists for cross-platform code.
#[cfg(not(all(target_arch = "wasm32", feature = "wasm")))]
pub fn run(_canvas_id: &str) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_compiles_and_modules_accessible() {
        // Verify all public modules are accessible.
        let _ = CanvasConfig::default();
        let _ = WebAudioConfig::default();
        let _ = EventTranslator::new();
        let _ = AssetFetcher::new("https://example.com/assets/");
        let _ = TypeConverter::new();
    }

    #[test]
    fn app_lifecycle_new_start_stop() {
        let opts = ArachneAppOptions::default();
        let mut app = ArachneApp::new("#canvas", opts);
        assert_eq!(app.state(), AppState::Idle);

        app.start();
        assert_eq!(app.state(), AppState::Running);

        app.stop();
        assert_eq!(app.state(), AppState::Stopped);
    }

    #[test]
    fn app_spawn_despawn_entities() {
        let opts = ArachneAppOptions::default();
        let mut app = ArachneApp::new("#canvas", opts);
        app.start();

        let e1 = app.spawn_default();
        let e2 = app.spawn_default();
        assert_ne!(e1, e2);

        assert!(app.despawn(e1));
        assert!(!app.despawn(e1)); // already despawned
        assert!(app.despawn(e2));
    }

    #[test]
    fn app_resize() {
        let opts = ArachneAppOptions::default();
        let mut app = ArachneApp::new("#canvas", opts);
        app.start();
        app.resize(1920, 1080);
        assert_eq!(app.canvas().logical_width(), 1920);
        assert_eq!(app.canvas().logical_height(), 1080);
    }
}
