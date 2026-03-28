//! High-level JS API for the Arachne engine.
//!
//! This module provides the `ArachneApp` class that is exposed to JavaScript
//! via wasm-bindgen. It is the primary public interface for embedding Arachne
//! in web pages.
//!
//! On native targets, the same API is available as a Rust struct for testing
//! and headless use.

use arachne_app::App;
use arachne_audio::AudioBackend;
use arachne_ecs::Entity;
use arachne_math::{Transform, Vec3};

#[cfg(all(target_arch = "wasm32", feature = "wasm"))]
use wasm_bindgen::prelude::*;

use crate::canvas::{CanvasConfig, CanvasHandle};
use crate::events::EventTranslator;
use crate::audio_backend::WebAudioBackend;
use crate::fetch::AssetFetcher;

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

/// The lifecycle state of an ArachneApp.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(all(target_arch = "wasm32", feature = "wasm"), wasm_bindgen)]
pub enum AppState {
    /// Created but not started.
    Idle,
    /// Running the main loop.
    Running,
    /// Stopped / paused.
    Stopped,
}

// ---------------------------------------------------------------------------
// App options
// ---------------------------------------------------------------------------

/// Options for creating an ArachneApp.
#[derive(Clone, Debug)]
pub struct ArachneAppOptions {
    /// Initial canvas width in CSS pixels.
    pub width: u32,
    /// Initial canvas height in CSS pixels.
    pub height: u32,
    /// Whether to enable high-DPI rendering.
    pub high_dpi: bool,
    /// Whether to enable audio.
    pub audio: bool,
    /// Base URL for asset loading.
    pub asset_base_url: String,
    /// Whether to prevent the default browser context menu.
    pub prevent_context_menu: bool,
    /// Target frames per second (0 = uncapped / requestAnimationFrame).
    pub target_fps: u32,
    /// Whether to auto-start on creation.
    pub auto_start: bool,
}

impl Default for ArachneAppOptions {
    fn default() -> Self {
        Self {
            width: 800,
            height: 600,
            high_dpi: true,
            audio: true,
            asset_base_url: "./assets/".to_string(),
            prevent_context_menu: true,
            target_fps: 0,
            auto_start: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Update callback
// ---------------------------------------------------------------------------

/// A stored update callback. On WASM this would hold a JS closure;
/// on native it holds a Rust function.
type UpdateCallback = Box<dyn FnMut(f64)>;

// ---------------------------------------------------------------------------
// ArachneApp
// ---------------------------------------------------------------------------

/// The main Arachne application exposed to JavaScript.
///
/// This wraps the engine's `App`, canvas management, event translation,
/// audio backend, and asset fetcher into a single cohesive API.
///
/// ## JS API (when compiled to WASM with wasm-bindgen):
///
/// ```js
/// const app = new ArachneApp("#canvas", { width: 800, height: 600 });
/// const entityId = app.spawn({ position: [1, 2, 3] });
/// app.onUpdate((dt) => { });
/// app.start();
/// app.despawn(entityId);
/// app.stop();
/// ```
#[cfg_attr(all(target_arch = "wasm32", feature = "wasm"), wasm_bindgen)]
pub struct ArachneApp {
    /// The underlying Arachne engine app.
    app: App,
    /// Canvas handle.
    canvas: CanvasHandle,
    /// DOM event translator.
    event_translator: EventTranslator,
    /// WebAudio backend.
    audio_backend: Option<WebAudioBackend>,
    /// Asset fetcher.
    fetcher: AssetFetcher,
    /// Current lifecycle state.
    state: AppState,
    /// Registered update callbacks.
    update_callbacks: Vec<UpdateCallback>,
    /// Entity counter for tracking spawned entities.
    spawned_entities: Vec<Entity>,
    /// The canvas CSS selector.
    selector: String,
    /// Options used to create this app.
    options: ArachneAppOptions,
}

impl ArachneApp {
    /// Create a new ArachneApp targeting the given canvas selector.
    ///
    /// In JS: `new ArachneApp("#my-canvas", options)`
    pub fn new(canvas_selector: &str, options: ArachneAppOptions) -> Self {
        let canvas_config = CanvasConfig {
            selector: canvas_selector.to_string(),
            width: options.width,
            height: options.height,
            high_dpi: options.high_dpi,
            fullscreen: false,
            prevent_context_menu: options.prevent_context_menu,
        };

        let canvas = CanvasHandle::new(canvas_config);
        let event_translator = EventTranslator::new();

        let audio_backend = if options.audio {
            Some(WebAudioBackend::new())
        } else {
            None
        };

        let fetcher = AssetFetcher::new(&options.asset_base_url);

        let app = App::new();

        Self {
            app,
            canvas,
            event_translator,
            audio_backend,
            fetcher,
            state: AppState::Idle,
            update_callbacks: Vec::new(),
            spawned_entities: Vec::new(),
            selector: canvas_selector.to_string(),
            options,
        }
    }

    // -----------------------------------------------------------------------
    // Lifecycle
    // -----------------------------------------------------------------------

    /// Start the application main loop.
    ///
    /// In JS: `app.start()`
    pub fn start(&mut self) {
        if self.state == AppState::Running {
            return;
        }

        // Initialize canvas.
        let _ = self.canvas.init();

        // Initialize audio if enabled.
        if let Some(ref mut audio) = self.audio_backend {
            let config = arachne_audio::BackendConfig::default();
            let _ = audio.init(config);
        }

        // Register DOM event listeners.
        #[cfg(all(target_arch = "wasm32", feature = "wasm"))]
        {
            let _ = crate::events::register_dom_listeners(&self.selector);
        }

        self.state = AppState::Running;

        #[cfg(all(target_arch = "wasm32", feature = "wasm"))]
        {
            // Real WASM would set up requestAnimationFrame loop here.
        }
    }

    /// Stop the application.
    ///
    /// In JS: `app.stop()`
    pub fn stop(&mut self) {
        if self.state != AppState::Running {
            return;
        }

        // Shutdown audio.
        if let Some(ref mut audio) = self.audio_backend {
            let _ = audio.shutdown();
        }

        self.state = AppState::Stopped;
    }

    /// Get the current lifecycle state.
    pub fn state(&self) -> AppState {
        self.state
    }

    /// Whether the app is currently running.
    pub fn is_running(&self) -> bool {
        self.state == AppState::Running
    }

    // -----------------------------------------------------------------------
    // Entity management
    // -----------------------------------------------------------------------

    /// Spawn a new entity with a default transform.
    ///
    /// Returns an entity ID (as u64 for JS interop: index in low 32 bits,
    /// generation in high 32 bits).
    pub fn spawn_default(&mut self) -> u64 {
        let entity = self.app.world.spawn((Transform::IDENTITY,));
        self.spawned_entities.push(entity);
        entity_to_u64(entity)
    }

    /// Spawn a new entity at the given position.
    pub fn spawn_at(&mut self, x: f32, y: f32, z: f32) -> u64 {
        let transform = Transform::from_position(Vec3::new(x, y, z));
        let entity = self.app.world.spawn((transform,));
        self.spawned_entities.push(entity);
        entity_to_u64(entity)
    }

    /// Despawn an entity by its ID.
    ///
    /// Returns true if the entity existed and was removed.
    pub fn despawn(&mut self, entity_id: u64) -> bool {
        let entity = u64_to_entity(entity_id);
        if let Some(pos) = self.spawned_entities.iter().position(|&e| e == entity) {
            self.app.world.despawn(entity);
            self.spawned_entities.remove(pos);
            true
        } else {
            false
        }
    }

    /// Get the number of spawned entities.
    pub fn entity_count(&self) -> usize {
        self.app.world.entity_count() as usize
    }

    // -----------------------------------------------------------------------
    // Scene loading
    // -----------------------------------------------------------------------

    /// Load a scene from a URL. Returns the resolved URL.
    ///
    /// In JS: `await app.loadScene("levels/level1.json")`
    pub fn load_scene(&mut self, url: &str) -> String {
        self.fetcher.fetch(url)
    }

    /// Check if a scene load is complete.
    pub fn is_scene_loaded(&self, url: &str) -> bool {
        self.fetcher.is_ok(url)
    }

    // -----------------------------------------------------------------------
    // Callbacks
    // -----------------------------------------------------------------------

    /// Register an update callback called each frame with delta time.
    ///
    /// In JS: `app.onUpdate((dt) => { ... })`
    pub fn on_update(&mut self, callback: impl FnMut(f64) + 'static) {
        self.update_callbacks.push(Box::new(callback));
    }

    /// Fire all update callbacks (called internally each frame).
    pub fn fire_update_callbacks(&mut self, delta_time: f64) {
        for cb in &mut self.update_callbacks {
            cb(delta_time);
        }
    }

    // -----------------------------------------------------------------------
    // Canvas operations
    // -----------------------------------------------------------------------

    /// Resize the canvas.
    ///
    /// In JS: `app.resize(1920, 1080)`
    pub fn resize(&mut self, width: u32, height: u32) {
        self.canvas.resize(width, height);
    }

    /// Get a reference to the canvas handle.
    pub fn canvas(&self) -> &CanvasHandle {
        &self.canvas
    }

    /// Get a mutable reference to the canvas handle.
    pub fn canvas_mut(&mut self) -> &mut CanvasHandle {
        &mut self.canvas
    }

    // -----------------------------------------------------------------------
    // Audio
    // -----------------------------------------------------------------------

    /// Get a reference to the audio backend (if enabled).
    pub fn audio_backend(&self) -> Option<&WebAudioBackend> {
        self.audio_backend.as_ref()
    }

    /// Get a mutable reference to the audio backend (if enabled).
    pub fn audio_backend_mut(&mut self) -> Option<&mut WebAudioBackend> {
        self.audio_backend.as_mut()
    }

    // -----------------------------------------------------------------------
    // Event handling
    // -----------------------------------------------------------------------

    /// Get a reference to the event translator.
    pub fn event_translator(&self) -> &EventTranslator {
        &self.event_translator
    }

    /// Get a mutable reference to the event translator.
    pub fn event_translator_mut(&mut self) -> &mut EventTranslator {
        &mut self.event_translator
    }

    // -----------------------------------------------------------------------
    // Asset fetching
    // -----------------------------------------------------------------------

    /// Get a reference to the asset fetcher.
    pub fn fetcher(&self) -> &AssetFetcher {
        &self.fetcher
    }

    /// Get a mutable reference to the asset fetcher.
    pub fn fetcher_mut(&mut self) -> &mut AssetFetcher {
        &mut self.fetcher
    }

    // -----------------------------------------------------------------------
    // Engine access
    // -----------------------------------------------------------------------

    /// Get a reference to the underlying Arachne App.
    pub fn engine(&self) -> &App {
        &self.app
    }

    /// Get a mutable reference to the underlying Arachne App.
    pub fn engine_mut(&mut self) -> &mut App {
        &mut self.app
    }

    /// Get the options this app was created with.
    pub fn options(&self) -> &ArachneAppOptions {
        &self.options
    }

    /// Get the CSS selector for this app's canvas.
    pub fn selector(&self) -> &str {
        &self.selector
    }
}

// ---------------------------------------------------------------------------
// WASM-specific JS API methods
// ---------------------------------------------------------------------------

#[cfg(all(target_arch = "wasm32", feature = "wasm"))]
#[wasm_bindgen]
impl ArachneApp {
    /// Create a new ArachneApp targeting the given canvas selector.
    ///
    /// JS: `const app = new ArachneApp("#my-canvas");`
    #[wasm_bindgen(constructor)]
    pub fn js_new(canvas_selector: &str) -> Self {
        Self::new(canvas_selector, ArachneAppOptions::default())
    }

    /// Create with custom dimensions.
    ///
    /// JS: `const app = ArachneApp.withSize("#canvas", 1024, 768);`
    #[wasm_bindgen(js_name = "withSize")]
    pub fn js_with_size(canvas_selector: &str, width: u32, height: u32) -> Self {
        let opts = ArachneAppOptions {
            width,
            height,
            ..ArachneAppOptions::default()
        };
        Self::new(canvas_selector, opts)
    }

    /// Start the application main loop.
    #[wasm_bindgen(js_name = "start")]
    pub fn js_start(&mut self) {
        self.start();
    }

    /// Stop the application.
    #[wasm_bindgen(js_name = "stop")]
    pub fn js_stop(&mut self) {
        self.stop();
    }

    /// Get the current lifecycle state as a string.
    #[wasm_bindgen(js_name = "getState")]
    pub fn js_state(&self) -> AppState {
        self.state()
    }

    /// Whether the app is currently running.
    #[wasm_bindgen(js_name = "isRunning")]
    pub fn js_is_running(&self) -> bool {
        self.is_running()
    }

    /// Spawn a new entity with a default transform. Returns entity ID.
    #[wasm_bindgen(js_name = "spawnDefault")]
    pub fn js_spawn_default(&mut self) -> u64 {
        self.spawn_default()
    }

    /// Spawn a new entity at the given position. Returns entity ID.
    #[wasm_bindgen(js_name = "spawnAt")]
    pub fn js_spawn_at(&mut self, x: f32, y: f32, z: f32) -> u64 {
        self.spawn_at(x, y, z)
    }

    /// Despawn an entity by its ID. Returns true if it existed.
    #[wasm_bindgen(js_name = "despawn")]
    pub fn js_despawn(&mut self, entity_id: u64) -> bool {
        self.despawn(entity_id)
    }

    /// Get the number of entities.
    #[wasm_bindgen(js_name = "entityCount")]
    pub fn js_entity_count(&self) -> usize {
        self.entity_count()
    }

    /// Resize the canvas.
    #[wasm_bindgen(js_name = "resize")]
    pub fn js_resize(&mut self, width: u32, height: u32) {
        self.resize(width, height);
    }
}

// ---------------------------------------------------------------------------
// Entity ID conversion helpers
// ---------------------------------------------------------------------------

/// Convert an Entity to a u64 for JS interop.
/// Low 32 bits = index, high 32 bits = generation.
pub fn entity_to_u64(entity: Entity) -> u64 {
    (entity.generation() as u64) << 32 | entity.index() as u64
}

/// Convert a u64 back to an Entity.
pub fn u64_to_entity(id: u64) -> Entity {
    let index = (id & 0xFFFF_FFFF) as u32;
    let generation = (id >> 32) as u32;
    Entity::from_raw(index, generation)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn default_app() -> ArachneApp {
        ArachneApp::new("#canvas", ArachneAppOptions::default())
    }

    #[test]
    fn app_options_default() {
        let opts = ArachneAppOptions::default();
        assert_eq!(opts.width, 800);
        assert_eq!(opts.height, 600);
        assert!(opts.high_dpi);
        assert!(opts.audio);
        assert!(!opts.auto_start);
        assert_eq!(opts.target_fps, 0);
    }

    #[test]
    fn app_lifecycle_idle_start_stop() {
        let mut app = default_app();
        assert_eq!(app.state(), AppState::Idle);
        assert!(!app.is_running());

        app.start();
        assert_eq!(app.state(), AppState::Running);
        assert!(app.is_running());

        app.stop();
        assert_eq!(app.state(), AppState::Stopped);
        assert!(!app.is_running());
    }

    #[test]
    fn app_start_idempotent() {
        let mut app = default_app();
        app.start();
        app.start(); // Should not panic or change state.
        assert_eq!(app.state(), AppState::Running);
    }

    #[test]
    fn app_stop_when_not_running() {
        let mut app = default_app();
        app.stop(); // Should be a no-op when idle.
        assert_eq!(app.state(), AppState::Idle);
    }

    #[test]
    fn app_spawn_default_entity() {
        let mut app = default_app();
        let e1 = app.spawn_default();
        let e2 = app.spawn_default();
        assert_ne!(e1, e2);
        assert_eq!(app.entity_count(), 2);
    }

    #[test]
    fn app_spawn_at_position() {
        let mut app = default_app();
        let eid = app.spawn_at(10.0, 20.0, 30.0);
        assert_eq!(app.entity_count(), 1);

        let entity = u64_to_entity(eid);
        let transform = app.engine().world.get::<Transform>(entity).unwrap();
        assert!((transform.position.x - 10.0).abs() < 1e-6);
        assert!((transform.position.y - 20.0).abs() < 1e-6);
        assert!((transform.position.z - 30.0).abs() < 1e-6);
    }

    #[test]
    fn app_despawn_entity() {
        let mut app = default_app();
        let e1 = app.spawn_default();
        let e2 = app.spawn_default();
        assert_eq!(app.entity_count(), 2);

        assert!(app.despawn(e1));
        assert_eq!(app.entity_count(), 1);

        // Despawn again should return false.
        assert!(!app.despawn(e1));

        assert!(app.despawn(e2));
        assert_eq!(app.entity_count(), 0);
    }

    #[test]
    fn app_resize() {
        let mut app = default_app();
        app.resize(1920, 1080);
        assert_eq!(app.canvas().logical_width(), 1920);
        assert_eq!(app.canvas().logical_height(), 1080);
    }

    #[test]
    fn app_update_callback() {
        let mut app = default_app();
        let mut total_dt = 0.0;
        let total_dt_ptr: *mut f64 = &mut total_dt;

        app.on_update(move |dt| {
            unsafe { *total_dt_ptr += dt; }
        });

        app.fire_update_callbacks(1.0 / 60.0);
        app.fire_update_callbacks(1.0 / 60.0);

        let expected = 2.0 / 60.0;
        assert!((total_dt - expected).abs() < 1e-10);
    }

    #[test]
    fn app_selector() {
        let app = ArachneApp::new("#game-canvas", ArachneAppOptions::default());
        assert_eq!(app.selector(), "#game-canvas");
    }

    #[test]
    fn app_audio_backend_present_when_enabled() {
        let opts = ArachneAppOptions {
            audio: true,
            ..ArachneAppOptions::default()
        };
        let app = ArachneApp::new("#canvas", opts);
        assert!(app.audio_backend().is_some());
    }

    #[test]
    fn app_audio_backend_absent_when_disabled() {
        let opts = ArachneAppOptions {
            audio: false,
            ..ArachneAppOptions::default()
        };
        let app = ArachneApp::new("#canvas", opts);
        assert!(app.audio_backend().is_none());
    }

    #[test]
    fn app_load_scene() {
        let mut app = default_app();
        let url = app.load_scene("levels/level1.json");
        assert!(url.contains("level1.json"));
    }

    #[test]
    fn entity_id_round_trip() {
        let entity = Entity::from_raw(42, 7);
        let id = entity_to_u64(entity);
        let back = u64_to_entity(id);
        assert_eq!(back.index(), 42);
        assert_eq!(back.generation(), 7);
    }

    #[test]
    fn entity_id_zero() {
        let entity = Entity::from_raw(0, 0);
        let id = entity_to_u64(entity);
        assert_eq!(id, 0);
        let back = u64_to_entity(id);
        assert_eq!(back.index(), 0);
        assert_eq!(back.generation(), 0);
    }

    #[test]
    fn entity_id_max_values() {
        let entity = Entity::from_raw(u32::MAX, u32::MAX);
        let id = entity_to_u64(entity);
        let back = u64_to_entity(id);
        assert_eq!(back.index(), u32::MAX);
        assert_eq!(back.generation(), u32::MAX);
    }

    #[test]
    fn app_engine_access() {
        let mut app = default_app();
        assert_eq!(app.engine().world.entity_count(), 0);

        app.spawn_default();
        assert_eq!(app.engine().world.entity_count(), 1);

        // Mutable access.
        let _ = app.engine_mut();
    }

    #[test]
    fn app_options_stored() {
        let opts = ArachneAppOptions {
            width: 1024,
            height: 768,
            target_fps: 30,
            ..ArachneAppOptions::default()
        };
        let app = ArachneApp::new("#canvas", opts);
        assert_eq!(app.options().width, 1024);
        assert_eq!(app.options().target_fps, 30);
    }

    #[test]
    fn app_no_audio_does_not_create_backend() {
        let opts = ArachneAppOptions {
            audio: false,
            ..ArachneAppOptions::default()
        };
        let mut app = ArachneApp::new("#canvas", opts);
        app.start();
        assert!(app.audio_backend().is_none());
        app.stop(); // Should not panic even without audio.
    }
}
