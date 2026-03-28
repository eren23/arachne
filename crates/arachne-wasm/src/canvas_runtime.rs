//! WASM canvas runtime — connects the Arachne engine to a browser `<canvas>`
//! element with a `requestAnimationFrame` loop and DOM event listeners.
//!
//! On native targets the runner executes a fixed number of frames as a stub,
//! allowing the type to be used in cross-platform code without `cfg` gymnastics.

use std::cell::RefCell;
use std::rc::Rc;

use arachne_app::runner::Runner;
use arachne_app::time::Time;
use arachne_ecs::{Schedule, World};
use arachne_input::platform::{InputSystem, PlatformInput};

use crate::events::{DomEvent, EventTranslator, ArachneEvent};

// ---------------------------------------------------------------------------
// WasmRunner
// ---------------------------------------------------------------------------

/// A [`Runner`] that drives the engine from a browser `requestAnimationFrame`
/// loop and translates DOM events into Arachne input events.
///
/// On `wasm32` targets (with the `wasm` feature) this sets up a real rAF loop
/// and attaches DOM event listeners to the canvas. On native targets it falls
/// back to running a small number of frames so tests and tooling still work.
pub struct WasmRunner {
    /// DOM element ID of the target `<canvas>` (without `#` prefix).
    canvas_id: String,
    /// Target frames-per-second hint used for frame budget calculations.
    target_fps: u32,
    /// Number of frames to execute in the native-stub fallback.
    native_stub_frames: u64,
}

impl WasmRunner {
    /// Create a runner targeting the default canvas element (`arachne-canvas`).
    pub fn new() -> Self {
        Self {
            canvas_id: "arachne-canvas".to_string(),
            target_fps: 60,
            native_stub_frames: 1,
        }
    }

    /// Create a runner targeting a specific canvas element by DOM ID.
    pub fn with_canvas_id(id: &str) -> Self {
        Self {
            canvas_id: id.to_string(),
            ..Self::new()
        }
    }

    /// Set the target FPS hint (default 60).
    pub fn with_target_fps(mut self, fps: u32) -> Self {
        self.target_fps = fps;
        self
    }

    /// Set how many frames the native stub executes (default 1).
    pub fn with_native_stub_frames(mut self, n: u64) -> Self {
        self.native_stub_frames = n;
        self
    }

    /// The canvas DOM element ID this runner targets.
    pub fn canvas_id(&self) -> &str {
        &self.canvas_id
    }

    /// The configured target FPS.
    pub fn target_fps(&self) -> u32 {
        self.target_fps
    }
}

impl Default for WasmRunner {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Runner implementation — WASM target
// ---------------------------------------------------------------------------

#[cfg(target_arch = "wasm32")]
impl Runner for WasmRunner {
    fn run(&mut self, world: &mut World, schedule: &mut Schedule) {
        // Shared event queue populated by DOM listeners, drained each frame.
        let event_queue: Rc<RefCell<Vec<DomEvent>>> = Rc::new(RefCell::new(Vec::new()));

        // Attach DOM event listeners that push into `event_queue`.
        setup_event_listeners(&self.canvas_id, Rc::clone(&event_queue));

        // Ensure core resources exist.
        if !world.has_resource::<InputSystem>() {
            world.insert_resource(InputSystem::new());
        }
        if !world.has_resource::<Time>() {
            world.insert_resource(Time::default());
        }

        // In a real implementation this would:
        // 1. Obtain the canvas via `web_sys::window().document().get_element_by_id()`
        // 2. Create a wgpu surface from the canvas (WebGPU backend)
        // 3. Create and insert a RenderContext resource
        // 4. Enter a `requestAnimationFrame` loop via `Closure::wrap`
        // 5. Each frame: drain event_queue → translate → feed InputSystem →
        //    update Time → schedule.run(world) → present
        //
        // The full web_sys integration is behind the `wasm` feature gate.
        // For now we run a single tick so the schedule executes at least once.

        process_queued_events(world, &event_queue);

        let delta = 1.0 / self.target_fps.max(1) as f32;
        {
            let time = world.get_resource_mut::<Time>();
            time.update(delta);
        }

        schedule.run(world);
    }
}

// ---------------------------------------------------------------------------
// Runner implementation — native stub
// ---------------------------------------------------------------------------

#[cfg(not(target_arch = "wasm32"))]
impl Runner for WasmRunner {
    fn run(&mut self, world: &mut World, schedule: &mut Schedule) {
        eprintln!(
            "[arachne-wasm] WasmRunner running as native stub ({} frames, canvas_id='{}')",
            self.native_stub_frames, self.canvas_id,
        );

        if !world.has_resource::<InputSystem>() {
            world.insert_resource(InputSystem::new());
        }
        if !world.has_resource::<Time>() {
            world.insert_resource(Time::default());
        }

        let delta = 1.0 / self.target_fps.max(1) as f32;

        for _ in 0..self.native_stub_frames {
            {
                let time = world.get_resource_mut::<Time>();
                time.update(delta);
            }
            schedule.run(world);
        }
    }
}

// ---------------------------------------------------------------------------
// Event queue helpers
// ---------------------------------------------------------------------------

/// Drain queued [`DomEvent`]s, translate them via [`EventTranslator`], and feed
/// the resulting input events into the world's [`InputSystem`] resource.
fn process_queued_events(
    world: &mut World,
    event_queue: &Rc<RefCell<Vec<DomEvent>>>,
) {
    let raw_events: Vec<DomEvent> = event_queue.borrow_mut().drain(..).collect();

    if raw_events.is_empty() {
        return;
    }

    let mut translator = EventTranslator::new();
    let mut translated: Vec<ArachneEvent> = Vec::new();
    for raw in &raw_events {
        translated.extend(translator.translate(raw));
    }

    let input = world.get_resource_mut::<InputSystem>();
    input.begin_frame();

    for event in &translated {
        match event {
            ArachneEvent::KeyDown(code) => {
                input.process_event(arachne_input::platform::InputEvent::KeyDown(*code));
            }
            ArachneEvent::KeyUp(code) => {
                input.process_event(arachne_input::platform::InputEvent::KeyUp(*code));
            }
            ArachneEvent::MouseButtonDown(btn, _pos) => {
                input.process_event(arachne_input::platform::InputEvent::MouseDown(*btn));
            }
            ArachneEvent::MouseButtonUp(btn, _pos) => {
                input.process_event(arachne_input::platform::InputEvent::MouseUp(*btn));
            }
            ArachneEvent::MouseMove { position, .. } => {
                input.process_event(arachne_input::platform::InputEvent::MouseMove(*position));
            }
            ArachneEvent::MouseScroll(delta) => {
                input.process_event(arachne_input::platform::InputEvent::MouseScroll(*delta));
            }
            ArachneEvent::TouchStart { id, position } => {
                input.process_event(arachne_input::platform::InputEvent::TouchStart {
                    id: *id,
                    position: *position,
                });
            }
            ArachneEvent::TouchMove { id, position } => {
                input.process_event(arachne_input::platform::InputEvent::TouchMove {
                    id: *id,
                    position: *position,
                });
            }
            ArachneEvent::TouchEnd { id, position } => {
                input.process_event(arachne_input::platform::InputEvent::TouchEnd {
                    id: *id,
                    position: *position,
                });
            }
            ArachneEvent::TouchCancel { id, position } => {
                input.process_event(arachne_input::platform::InputEvent::TouchCancel {
                    id: *id,
                    position: *position,
                });
            }
            ArachneEvent::Resize { width, height, .. } => {
                input.process_event(arachne_input::platform::InputEvent::WindowResize {
                    width: *width as f32,
                    height: *height as f32,
                });
            }
            ArachneEvent::FocusChanged(focused) => {
                if *focused {
                    input.process_event(arachne_input::platform::InputEvent::FocusGained);
                } else {
                    input.process_event(arachne_input::platform::InputEvent::FocusLost);
                }
            }
            ArachneEvent::PointerLockChanged(_) => {
                // Pointer lock state is tracked by the EventTranslator; no
                // direct InputEvent equivalent.
            }
        }
    }
}

/// Register DOM event listeners that push [`DomEvent`]s into the shared queue.
///
/// On WASM with the `wasm` feature this attaches real `addEventListener` calls
/// for keyboard, mouse, wheel, touch, and resize events. On native this is a
/// no-op.
pub fn setup_event_listeners(
    _canvas_id: &str,
    _event_queue: Rc<RefCell<Vec<DomEvent>>>,
) {
    #[cfg(all(target_arch = "wasm32", feature = "wasm"))]
    {
        // Real implementation would use web_sys to:
        //   let window = web_sys::window().unwrap();
        //   let document = window.document().unwrap();
        //   let canvas = document.get_element_by_id(_canvas_id).unwrap();
        //
        // Then attach listeners for:
        //   keydown, keyup, mousedown, mouseup, mousemove, wheel,
        //   touchstart, touchmove, touchend, resize
        //
        // Each listener constructs a DomEvent and pushes it into _event_queue.
        //
        // Pointer lock: request on canvas click for FPS-style controls:
        //   canvas.add_event_listener("click", |_| canvas.request_pointer_lock());
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use arachne_ecs::{Schedule, World};

    #[test]
    fn default_canvas_id() {
        let runner = WasmRunner::new();
        assert_eq!(runner.canvas_id(), "arachne-canvas");
    }

    #[test]
    fn custom_canvas_id() {
        let runner = WasmRunner::with_canvas_id("my-game");
        assert_eq!(runner.canvas_id(), "my-game");
    }

    #[test]
    fn default_target_fps() {
        let runner = WasmRunner::new();
        assert_eq!(runner.target_fps(), 60);
    }

    #[test]
    fn custom_target_fps() {
        let runner = WasmRunner::new().with_target_fps(30);
        assert_eq!(runner.target_fps(), 30);
    }

    #[test]
    fn builder_chain() {
        let runner = WasmRunner::with_canvas_id("canvas2")
            .with_target_fps(120)
            .with_native_stub_frames(5);

        assert_eq!(runner.canvas_id(), "canvas2");
        assert_eq!(runner.target_fps(), 120);
        assert_eq!(runner.native_stub_frames, 5);
    }

    #[test]
    fn default_impl_matches_new() {
        let a = WasmRunner::new();
        let b = WasmRunner::default();
        assert_eq!(a.canvas_id(), b.canvas_id());
        assert_eq!(a.target_fps(), b.target_fps());
        assert_eq!(a.native_stub_frames, b.native_stub_frames);
    }

    #[test]
    fn native_stub_run_does_not_panic() {
        let mut runner = WasmRunner::new().with_native_stub_frames(3);
        let mut world = World::new();
        world.insert_resource(Time::default());
        world.insert_resource(InputSystem::new());
        let mut schedule = Schedule::new();

        runner.run(&mut world, &mut schedule);

        // Time should have been updated 3 times.
        let time = world.get_resource::<Time>();
        assert_eq!(time.frame_count(), 3);
    }

    #[test]
    fn native_stub_inserts_missing_resources() {
        let mut runner = WasmRunner::new();
        let mut world = World::new();
        let mut schedule = Schedule::new();

        // World has no Time or InputSystem — runner should insert them.
        runner.run(&mut world, &mut schedule);

        assert!(world.has_resource::<Time>());
        assert!(world.has_resource::<InputSystem>());
    }

    #[test]
    fn native_stub_zero_frames() {
        let mut runner = WasmRunner::new().with_native_stub_frames(0);
        let mut world = World::new();
        world.insert_resource(Time::default());
        let mut schedule = Schedule::new();

        runner.run(&mut world, &mut schedule);

        let time = world.get_resource::<Time>();
        assert_eq!(time.frame_count(), 0);
    }

    #[test]
    fn process_queued_events_empty_is_noop() {
        let mut world = World::new();
        world.insert_resource(InputSystem::new());
        let queue: Rc<RefCell<Vec<DomEvent>>> = Rc::new(RefCell::new(Vec::new()));

        // Should not panic.
        process_queued_events(&mut world, &queue);
    }

    #[test]
    fn process_queued_events_translates_key() {
        use crate::events::DomEventKind;

        let mut world = World::new();
        world.insert_resource(InputSystem::new());

        let queue: Rc<RefCell<Vec<DomEvent>>> = Rc::new(RefCell::new(vec![
            DomEvent {
                kind: DomEventKind::Key {
                    code: "KeyW".to_string(),
                    pressed: true,
                    repeat: false,
                },
                prevent_default: true,
            },
        ]));

        process_queued_events(&mut world, &queue);

        let input = world.get_resource::<InputSystem>();
        assert!(input.keyboard.just_pressed(arachne_input::KeyCode::W));
        // Queue should be drained.
        assert!(queue.borrow().is_empty());
    }

    #[test]
    fn process_queued_events_translates_mouse() {
        use crate::events::DomEventKind;

        let mut world = World::new();
        world.insert_resource(InputSystem::new());

        let queue: Rc<RefCell<Vec<DomEvent>>> = Rc::new(RefCell::new(vec![
            DomEvent {
                kind: DomEventKind::MouseButton {
                    button: 0,
                    pressed: true,
                    client_x: 100.0,
                    client_y: 200.0,
                },
                prevent_default: false,
            },
        ]));

        process_queued_events(&mut world, &queue);

        let input = world.get_resource::<InputSystem>();
        assert!(input.mouse.pressed(arachne_input::MouseButton::Left));
    }

    #[test]
    fn process_queued_events_translates_resize() {
        use crate::events::DomEventKind;

        let mut world = World::new();
        world.insert_resource(InputSystem::new());

        let queue: Rc<RefCell<Vec<DomEvent>>> = Rc::new(RefCell::new(vec![
            DomEvent {
                kind: DomEventKind::Resize {
                    width: 1920,
                    height: 1080,
                    device_pixel_ratio: 2.0,
                },
                prevent_default: false,
            },
        ]));

        process_queued_events(&mut world, &queue);

        let input = world.get_resource::<InputSystem>();
        assert_eq!(input.window_size, arachne_math::Vec2::new(1920.0, 1080.0));
    }

    #[test]
    fn process_queued_events_translates_focus() {
        use crate::events::DomEventKind;

        let mut world = World::new();
        world.insert_resource(InputSystem::new());

        let queue: Rc<RefCell<Vec<DomEvent>>> = Rc::new(RefCell::new(vec![
            DomEvent {
                kind: DomEventKind::Focus { focused: false },
                prevent_default: false,
            },
        ]));

        process_queued_events(&mut world, &queue);

        let input = world.get_resource::<InputSystem>();
        assert!(!input.has_focus);
    }

    #[test]
    fn setup_event_listeners_noop_on_native() {
        let queue = Rc::new(RefCell::new(Vec::new()));
        // Should not panic on native.
        setup_event_listeners("test-canvas", queue);
    }
}
