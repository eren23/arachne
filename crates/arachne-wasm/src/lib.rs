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
