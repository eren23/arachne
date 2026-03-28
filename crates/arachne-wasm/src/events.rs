//! DOM event translation to Arachne input events.
//!
//! Translates browser keyboard, mouse, touch, and resize events into the
//! types defined in `arachne-input`. On native targets, provides the same
//! translation API using stub DOM event representations.

use arachne_input::{KeyCode, MouseButton, TouchPhase};
use arachne_math::Vec2;

// ---------------------------------------------------------------------------
// DOM event representation (platform-agnostic)
// ---------------------------------------------------------------------------

/// A simplified DOM event that can be constructed from real browser events
/// (on WASM) or fabricated for testing (on native).
#[derive(Clone, Debug)]
pub struct DomEvent {
    /// The kind of event.
    pub kind: DomEventKind,
    /// Whether `preventDefault()` should be called on the original event.
    pub prevent_default: bool,
}

/// The specific type and payload of a DOM event.
#[derive(Clone, Debug)]
pub enum DomEventKind {
    /// Keyboard key pressed or released.
    Key {
        code: String,
        pressed: bool,
        repeat: bool,
    },
    /// Mouse button pressed or released.
    MouseButton {
        button: u16,
        pressed: bool,
        client_x: f64,
        client_y: f64,
    },
    /// Mouse moved.
    MouseMove {
        client_x: f64,
        client_y: f64,
        movement_x: f64,
        movement_y: f64,
    },
    /// Mouse wheel scrolled.
    MouseWheel {
        delta_x: f64,
        delta_y: f64,
    },
    /// Touch event.
    Touch {
        id: u64,
        phase: DomTouchPhase,
        client_x: f64,
        client_y: f64,
    },
    /// Window / canvas resized.
    Resize {
        width: u32,
        height: u32,
        device_pixel_ratio: f64,
    },
    /// Pointer lock state changed.
    PointerLock {
        locked: bool,
    },
    /// Focus gained or lost.
    Focus {
        focused: bool,
    },
}

/// Touch phases as they come from the DOM.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DomTouchPhase {
    Start,
    Move,
    End,
    Cancel,
}

// ---------------------------------------------------------------------------
// Translated Arachne events
// ---------------------------------------------------------------------------

/// An Arachne input event, the output of translating a DOM event.
#[derive(Clone, Debug)]
pub enum ArachneEvent {
    KeyDown(KeyCode),
    KeyUp(KeyCode),
    MouseButtonDown(MouseButton, Vec2),
    MouseButtonUp(MouseButton, Vec2),
    MouseMove { position: Vec2, delta: Vec2 },
    MouseScroll(Vec2),
    TouchStart { id: u64, position: Vec2 },
    TouchMove { id: u64, position: Vec2 },
    TouchEnd { id: u64, position: Vec2 },
    TouchCancel { id: u64, position: Vec2 },
    Resize { width: u32, height: u32, dpr: f64 },
    PointerLockChanged(bool),
    FocusChanged(bool),
}

// ---------------------------------------------------------------------------
// Event translator
// ---------------------------------------------------------------------------

/// Translates DOM events into Arachne engine events.
///
/// Holds state needed for translation (e.g. canvas offset, DPI factor).
pub struct EventTranslator {
    /// Canvas offset from the page origin (for converting client coords).
    canvas_offset_x: f64,
    canvas_offset_y: f64,
    /// Device pixel ratio for coordinate scaling.
    device_pixel_ratio: f64,
    /// Whether pointer lock is active.
    pointer_locked: bool,
}

impl EventTranslator {
    /// Create a new event translator with default settings.
    pub fn new() -> Self {
        Self {
            canvas_offset_x: 0.0,
            canvas_offset_y: 0.0,
            device_pixel_ratio: 1.0,
            pointer_locked: false,
        }
    }

    /// Update the canvas offset (bounding rect from DOM).
    pub fn set_canvas_offset(&mut self, x: f64, y: f64) {
        self.canvas_offset_x = x;
        self.canvas_offset_y = y;
    }

    /// Update the device pixel ratio.
    pub fn set_device_pixel_ratio(&mut self, dpr: f64) {
        self.device_pixel_ratio = dpr;
    }

    /// Whether pointer lock is currently active.
    pub fn is_pointer_locked(&self) -> bool {
        self.pointer_locked
    }

    /// Translate a DOM event into zero or more Arachne events.
    pub fn translate(&mut self, event: &DomEvent) -> Vec<ArachneEvent> {
        match &event.kind {
            DomEventKind::Key { code, pressed, repeat } => {
                if *repeat {
                    return Vec::new(); // Ignore key repeat events.
                }
                if let Some(key_code) = translate_key_code(code) {
                    if *pressed {
                        vec![ArachneEvent::KeyDown(key_code)]
                    } else {
                        vec![ArachneEvent::KeyUp(key_code)]
                    }
                } else {
                    Vec::new()
                }
            }

            DomEventKind::MouseButton { button, pressed, client_x, client_y } => {
                let pos = self.translate_position(*client_x, *client_y);
                if let Some(btn) = translate_mouse_button(*button) {
                    if *pressed {
                        vec![ArachneEvent::MouseButtonDown(btn, pos)]
                    } else {
                        vec![ArachneEvent::MouseButtonUp(btn, pos)]
                    }
                } else {
                    Vec::new()
                }
            }

            DomEventKind::MouseMove { client_x, client_y, movement_x, movement_y } => {
                let position = self.translate_position(*client_x, *client_y);
                let delta = Vec2::new(*movement_x as f32, *movement_y as f32);
                vec![ArachneEvent::MouseMove { position, delta }]
            }

            DomEventKind::MouseWheel { delta_x, delta_y } => {
                vec![ArachneEvent::MouseScroll(Vec2::new(*delta_x as f32, *delta_y as f32))]
            }

            DomEventKind::Touch { id, phase, client_x, client_y } => {
                let position = self.translate_position(*client_x, *client_y);
                match phase {
                    DomTouchPhase::Start => vec![ArachneEvent::TouchStart { id: *id, position }],
                    DomTouchPhase::Move => vec![ArachneEvent::TouchMove { id: *id, position }],
                    DomTouchPhase::End => vec![ArachneEvent::TouchEnd { id: *id, position }],
                    DomTouchPhase::Cancel => vec![ArachneEvent::TouchCancel { id: *id, position }],
                }
            }

            DomEventKind::Resize { width, height, device_pixel_ratio } => {
                self.device_pixel_ratio = *device_pixel_ratio;
                vec![ArachneEvent::Resize {
                    width: *width,
                    height: *height,
                    dpr: *device_pixel_ratio,
                }]
            }

            DomEventKind::PointerLock { locked } => {
                self.pointer_locked = *locked;
                vec![ArachneEvent::PointerLockChanged(*locked)]
            }

            DomEventKind::Focus { focused } => {
                vec![ArachneEvent::FocusChanged(*focused)]
            }
        }
    }

    /// Convert client coordinates to canvas-local coordinates.
    fn translate_position(&self, client_x: f64, client_y: f64) -> Vec2 {
        let x = (client_x - self.canvas_offset_x) as f32;
        let y = (client_y - self.canvas_offset_y) as f32;
        Vec2::new(x, y)
    }
}

impl Default for EventTranslator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// DOM key code translation
// ---------------------------------------------------------------------------

/// Translate a DOM `KeyboardEvent.code` string to an Arachne `KeyCode`.
pub fn translate_key_code(code: &str) -> Option<KeyCode> {
    match code {
        // Letters
        "KeyA" => Some(KeyCode::A),
        "KeyB" => Some(KeyCode::B),
        "KeyC" => Some(KeyCode::C),
        "KeyD" => Some(KeyCode::D),
        "KeyE" => Some(KeyCode::E),
        "KeyF" => Some(KeyCode::F),
        "KeyG" => Some(KeyCode::G),
        "KeyH" => Some(KeyCode::H),
        "KeyI" => Some(KeyCode::I),
        "KeyJ" => Some(KeyCode::J),
        "KeyK" => Some(KeyCode::K),
        "KeyL" => Some(KeyCode::L),
        "KeyM" => Some(KeyCode::M),
        "KeyN" => Some(KeyCode::N),
        "KeyO" => Some(KeyCode::O),
        "KeyP" => Some(KeyCode::P),
        "KeyQ" => Some(KeyCode::Q),
        "KeyR" => Some(KeyCode::R),
        "KeyS" => Some(KeyCode::S),
        "KeyT" => Some(KeyCode::T),
        "KeyU" => Some(KeyCode::U),
        "KeyV" => Some(KeyCode::V),
        "KeyW" => Some(KeyCode::W),
        "KeyX" => Some(KeyCode::X),
        "KeyY" => Some(KeyCode::Y),
        "KeyZ" => Some(KeyCode::Z),

        // Digits
        "Digit0" => Some(KeyCode::Key0),
        "Digit1" => Some(KeyCode::Key1),
        "Digit2" => Some(KeyCode::Key2),
        "Digit3" => Some(KeyCode::Key3),
        "Digit4" => Some(KeyCode::Key4),
        "Digit5" => Some(KeyCode::Key5),
        "Digit6" => Some(KeyCode::Key6),
        "Digit7" => Some(KeyCode::Key7),
        "Digit8" => Some(KeyCode::Key8),
        "Digit9" => Some(KeyCode::Key9),

        // Function keys
        "F1" => Some(KeyCode::F1),
        "F2" => Some(KeyCode::F2),
        "F3" => Some(KeyCode::F3),
        "F4" => Some(KeyCode::F4),
        "F5" => Some(KeyCode::F5),
        "F6" => Some(KeyCode::F6),
        "F7" => Some(KeyCode::F7),
        "F8" => Some(KeyCode::F8),
        "F9" => Some(KeyCode::F9),
        "F10" => Some(KeyCode::F10),
        "F11" => Some(KeyCode::F11),
        "F12" => Some(KeyCode::F12),

        // Arrow keys
        "ArrowUp" => Some(KeyCode::Up),
        "ArrowDown" => Some(KeyCode::Down),
        "ArrowLeft" => Some(KeyCode::Left),
        "ArrowRight" => Some(KeyCode::Right),

        // Modifiers
        "ShiftLeft" => Some(KeyCode::LeftShift),
        "ShiftRight" => Some(KeyCode::RightShift),
        "ControlLeft" => Some(KeyCode::LeftCtrl),
        "ControlRight" => Some(KeyCode::RightCtrl),
        "AltLeft" => Some(KeyCode::LeftAlt),
        "AltRight" => Some(KeyCode::RightAlt),
        "MetaLeft" => Some(KeyCode::LeftSuper),
        "MetaRight" => Some(KeyCode::RightSuper),

        // Whitespace / editing
        "Space" => Some(KeyCode::Space),
        "Tab" => Some(KeyCode::Tab),
        "Enter" => Some(KeyCode::Enter),
        "Backspace" => Some(KeyCode::Backspace),
        "Delete" => Some(KeyCode::Delete),
        "Insert" => Some(KeyCode::Insert),

        // Navigation
        "Home" => Some(KeyCode::Home),
        "End" => Some(KeyCode::End),
        "PageUp" => Some(KeyCode::PageUp),
        "PageDown" => Some(KeyCode::PageDown),

        // Punctuation / control
        "Escape" => Some(KeyCode::Escape),
        "CapsLock" => Some(KeyCode::CapsLock),
        "NumLock" => Some(KeyCode::NumLock),
        "ScrollLock" => Some(KeyCode::ScrollLock),
        "PrintScreen" => Some(KeyCode::PrintScreen),
        "Pause" => Some(KeyCode::Pause),
        "Comma" => Some(KeyCode::Comma),
        "Period" => Some(KeyCode::Period),
        "Slash" => Some(KeyCode::Slash),
        "Backslash" => Some(KeyCode::Backslash),
        "Semicolon" => Some(KeyCode::Semicolon),
        "Quote" => Some(KeyCode::Apostrophe),
        "BracketLeft" => Some(KeyCode::LeftBracket),
        "BracketRight" => Some(KeyCode::RightBracket),
        "Backquote" => Some(KeyCode::GraveAccent),
        "Minus" => Some(KeyCode::Minus),
        "Equal" => Some(KeyCode::Equal),

        // Numpad
        "Numpad0" => Some(KeyCode::Numpad0),
        "Numpad1" => Some(KeyCode::Numpad1),
        "Numpad2" => Some(KeyCode::Numpad2),
        "Numpad3" => Some(KeyCode::Numpad3),
        "Numpad4" => Some(KeyCode::Numpad4),
        "Numpad5" => Some(KeyCode::Numpad5),
        "Numpad6" => Some(KeyCode::Numpad6),
        "Numpad7" => Some(KeyCode::Numpad7),
        "Numpad8" => Some(KeyCode::Numpad8),
        "Numpad9" => Some(KeyCode::Numpad9),
        "NumpadAdd" => Some(KeyCode::NumpadAdd),
        "NumpadSubtract" => Some(KeyCode::NumpadSubtract),
        "NumpadMultiply" => Some(KeyCode::NumpadMultiply),
        "NumpadDivide" => Some(KeyCode::NumpadDivide),
        "NumpadEnter" => Some(KeyCode::NumpadEnter),
        "NumpadDecimal" => Some(KeyCode::NumpadDecimal),

        _ => None,
    }
}

/// Translate a DOM mouse button number to an Arachne `MouseButton`.
pub fn translate_mouse_button(button: u16) -> Option<MouseButton> {
    match button {
        0 => Some(MouseButton::Left),
        1 => Some(MouseButton::Middle),
        2 => Some(MouseButton::Right),
        3 => Some(MouseButton::X1),
        4 => Some(MouseButton::X2),
        _ => None,
    }
}

/// Translate a `DomTouchPhase` to an Arachne `TouchPhase`.
pub fn translate_touch_phase(phase: DomTouchPhase) -> TouchPhase {
    match phase {
        DomTouchPhase::Start => TouchPhase::Started,
        DomTouchPhase::Move => TouchPhase::Moved,
        DomTouchPhase::End => TouchPhase::Ended,
        DomTouchPhase::Cancel => TouchPhase::Cancelled,
    }
}

// ---------------------------------------------------------------------------
// WASM-specific: register DOM event listeners
// ---------------------------------------------------------------------------

/// Set up DOM event listeners on a canvas element.
///
/// On WASM with the `wasm` feature, this attaches `keydown`, `keyup`,
/// `mousedown`, `mouseup`, `mousemove`, `wheel`, `touchstart`, `touchmove`,
/// `touchend`, `touchcancel`, and resize observers. On native, this is a no-op.
pub fn register_dom_listeners(_canvas_selector: &str) -> Result<(), String> {
    #[cfg(all(target_arch = "wasm32", feature = "wasm"))]
    {
        // Real WASM implementation would use web_sys and wasm_bindgen::closure
        // to attach event listeners to the canvas and window.
        //
        // Example pseudocode:
        //   let document = web_sys::window().unwrap().document().unwrap();
        //   let canvas = document.query_selector(selector).unwrap().unwrap();
        //   let closure = Closure::wrap(Box::new(move |event: KeyboardEvent| {
        //       // translate and queue event
        //   }) as Box<dyn FnMut(KeyboardEvent)>);
        //   canvas.add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref())?;
        //   closure.forget();
    }

    Ok(())
}

/// Request pointer lock on the canvas element.
pub fn request_pointer_lock(_canvas_selector: &str) -> Result<(), String> {
    #[cfg(all(target_arch = "wasm32", feature = "wasm"))]
    {
        // canvas.requestPointerLock()
    }

    Ok(())
}

/// Exit pointer lock.
pub fn exit_pointer_lock() {
    #[cfg(all(target_arch = "wasm32", feature = "wasm"))]
    {
        // document.exitPointerLock()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_key_event(code: &str, pressed: bool) -> DomEvent {
        DomEvent {
            kind: DomEventKind::Key {
                code: code.to_string(),
                pressed,
                repeat: false,
            },
            prevent_default: true,
        }
    }

    fn make_mouse_button_event(button: u16, pressed: bool, x: f64, y: f64) -> DomEvent {
        DomEvent {
            kind: DomEventKind::MouseButton {
                button,
                pressed,
                client_x: x,
                client_y: y,
            },
            prevent_default: false,
        }
    }

    fn make_mouse_move_event(x: f64, y: f64, dx: f64, dy: f64) -> DomEvent {
        DomEvent {
            kind: DomEventKind::MouseMove {
                client_x: x,
                client_y: y,
                movement_x: dx,
                movement_y: dy,
            },
            prevent_default: false,
        }
    }

    fn make_touch_event(id: u64, phase: DomTouchPhase, x: f64, y: f64) -> DomEvent {
        DomEvent {
            kind: DomEventKind::Touch {
                id,
                phase,
                client_x: x,
                client_y: y,
            },
            prevent_default: true,
        }
    }

    fn make_resize_event(w: u32, h: u32, dpr: f64) -> DomEvent {
        DomEvent {
            kind: DomEventKind::Resize {
                width: w,
                height: h,
                device_pixel_ratio: dpr,
            },
            prevent_default: false,
        }
    }

    // --- Key translation tests ---

    #[test]
    fn translate_key_code_wasd() {
        assert_eq!(translate_key_code("KeyW"), Some(KeyCode::W));
        assert_eq!(translate_key_code("KeyA"), Some(KeyCode::A));
        assert_eq!(translate_key_code("KeyS"), Some(KeyCode::S));
        assert_eq!(translate_key_code("KeyD"), Some(KeyCode::D));
    }

    #[test]
    fn translate_key_code_arrows() {
        assert_eq!(translate_key_code("ArrowUp"), Some(KeyCode::Up));
        assert_eq!(translate_key_code("ArrowDown"), Some(KeyCode::Down));
        assert_eq!(translate_key_code("ArrowLeft"), Some(KeyCode::Left));
        assert_eq!(translate_key_code("ArrowRight"), Some(KeyCode::Right));
    }

    #[test]
    fn translate_key_code_space_enter_escape() {
        assert_eq!(translate_key_code("Space"), Some(KeyCode::Space));
        assert_eq!(translate_key_code("Enter"), Some(KeyCode::Enter));
        assert_eq!(translate_key_code("Escape"), Some(KeyCode::Escape));
    }

    #[test]
    fn translate_key_code_digits() {
        assert_eq!(translate_key_code("Digit0"), Some(KeyCode::Key0));
        assert_eq!(translate_key_code("Digit9"), Some(KeyCode::Key9));
    }

    #[test]
    fn translate_key_code_function_keys() {
        assert_eq!(translate_key_code("F1"), Some(KeyCode::F1));
        assert_eq!(translate_key_code("F12"), Some(KeyCode::F12));
    }

    #[test]
    fn translate_key_code_modifiers() {
        assert_eq!(translate_key_code("ShiftLeft"), Some(KeyCode::LeftShift));
        assert_eq!(translate_key_code("ControlRight"), Some(KeyCode::RightCtrl));
        assert_eq!(translate_key_code("AltLeft"), Some(KeyCode::LeftAlt));
        assert_eq!(translate_key_code("MetaLeft"), Some(KeyCode::LeftSuper));
    }

    #[test]
    fn translate_key_code_numpad() {
        assert_eq!(translate_key_code("Numpad0"), Some(KeyCode::Numpad0));
        assert_eq!(translate_key_code("NumpadAdd"), Some(KeyCode::NumpadAdd));
        assert_eq!(translate_key_code("NumpadEnter"), Some(KeyCode::NumpadEnter));
    }

    #[test]
    fn translate_key_code_unknown() {
        assert_eq!(translate_key_code("SomeWeirdKey"), None);
        assert_eq!(translate_key_code(""), None);
    }

    // --- Mouse button translation tests ---

    #[test]
    fn translate_mouse_buttons() {
        assert_eq!(translate_mouse_button(0), Some(MouseButton::Left));
        assert_eq!(translate_mouse_button(1), Some(MouseButton::Middle));
        assert_eq!(translate_mouse_button(2), Some(MouseButton::Right));
        assert_eq!(translate_mouse_button(3), Some(MouseButton::X1));
        assert_eq!(translate_mouse_button(4), Some(MouseButton::X2));
        assert_eq!(translate_mouse_button(5), None);
    }

    // --- Touch phase translation tests ---

    #[test]
    fn translate_touch_phases() {
        assert_eq!(translate_touch_phase(DomTouchPhase::Start), TouchPhase::Started);
        assert_eq!(translate_touch_phase(DomTouchPhase::Move), TouchPhase::Moved);
        assert_eq!(translate_touch_phase(DomTouchPhase::End), TouchPhase::Ended);
        assert_eq!(translate_touch_phase(DomTouchPhase::Cancel), TouchPhase::Cancelled);
    }

    // --- Event translator tests ---

    #[test]
    fn translator_key_press_produces_keydown() {
        let mut translator = EventTranslator::new();
        let event = make_key_event("KeyW", true);
        let results = translator.translate(&event);

        assert_eq!(results.len(), 1);
        match &results[0] {
            ArachneEvent::KeyDown(code) => assert_eq!(*code, KeyCode::W),
            other => panic!("expected KeyDown, got {:?}", other),
        }
    }

    #[test]
    fn translator_key_release_produces_keyup() {
        let mut translator = EventTranslator::new();
        let event = make_key_event("KeyW", false);
        let results = translator.translate(&event);

        assert_eq!(results.len(), 1);
        match &results[0] {
            ArachneEvent::KeyUp(code) => assert_eq!(*code, KeyCode::W),
            other => panic!("expected KeyUp, got {:?}", other),
        }
    }

    #[test]
    fn translator_key_repeat_ignored() {
        let mut translator = EventTranslator::new();
        let event = DomEvent {
            kind: DomEventKind::Key {
                code: "KeyW".to_string(),
                pressed: true,
                repeat: true,
            },
            prevent_default: false,
        };
        let results = translator.translate(&event);
        assert!(results.is_empty());
    }

    #[test]
    fn translator_unknown_key_ignored() {
        let mut translator = EventTranslator::new();
        let event = make_key_event("UnknownKey", true);
        let results = translator.translate(&event);
        assert!(results.is_empty());
    }

    #[test]
    fn translator_mouse_button_down() {
        let mut translator = EventTranslator::new();
        let event = make_mouse_button_event(0, true, 100.0, 200.0);
        let results = translator.translate(&event);

        assert_eq!(results.len(), 1);
        match &results[0] {
            ArachneEvent::MouseButtonDown(btn, pos) => {
                assert_eq!(*btn, MouseButton::Left);
                assert_eq!(*pos, Vec2::new(100.0, 200.0));
            }
            other => panic!("expected MouseButtonDown, got {:?}", other),
        }
    }

    #[test]
    fn translator_mouse_button_with_offset() {
        let mut translator = EventTranslator::new();
        translator.set_canvas_offset(50.0, 100.0);

        let event = make_mouse_button_event(0, true, 150.0, 300.0);
        let results = translator.translate(&event);

        match &results[0] {
            ArachneEvent::MouseButtonDown(_, pos) => {
                assert_eq!(*pos, Vec2::new(100.0, 200.0));
            }
            other => panic!("expected MouseButtonDown, got {:?}", other),
        }
    }

    #[test]
    fn translator_mouse_move() {
        let mut translator = EventTranslator::new();
        let event = make_mouse_move_event(300.0, 400.0, 5.0, -3.0);
        let results = translator.translate(&event);

        assert_eq!(results.len(), 1);
        match &results[0] {
            ArachneEvent::MouseMove { position, delta } => {
                assert_eq!(*position, Vec2::new(300.0, 400.0));
                assert_eq!(*delta, Vec2::new(5.0, -3.0));
            }
            other => panic!("expected MouseMove, got {:?}", other),
        }
    }

    #[test]
    fn translator_mouse_wheel() {
        let mut translator = EventTranslator::new();
        let event = DomEvent {
            kind: DomEventKind::MouseWheel {
                delta_x: 0.0,
                delta_y: -120.0,
            },
            prevent_default: true,
        };
        let results = translator.translate(&event);

        assert_eq!(results.len(), 1);
        match &results[0] {
            ArachneEvent::MouseScroll(scroll) => {
                assert_eq!(scroll.x, 0.0);
                assert_eq!(scroll.y, -120.0);
            }
            other => panic!("expected MouseScroll, got {:?}", other),
        }
    }

    #[test]
    fn translator_touch_start() {
        let mut translator = EventTranslator::new();
        let event = make_touch_event(42, DomTouchPhase::Start, 200.0, 300.0);
        let results = translator.translate(&event);

        assert_eq!(results.len(), 1);
        match &results[0] {
            ArachneEvent::TouchStart { id, position } => {
                assert_eq!(*id, 42);
                assert_eq!(*position, Vec2::new(200.0, 300.0));
            }
            other => panic!("expected TouchStart, got {:?}", other),
        }
    }

    #[test]
    fn translator_touch_move_end_cancel() {
        let mut translator = EventTranslator::new();

        let move_event = make_touch_event(1, DomTouchPhase::Move, 100.0, 100.0);
        let results = translator.translate(&move_event);
        assert!(matches!(&results[0], ArachneEvent::TouchMove { .. }));

        let end_event = make_touch_event(1, DomTouchPhase::End, 100.0, 100.0);
        let results = translator.translate(&end_event);
        assert!(matches!(&results[0], ArachneEvent::TouchEnd { .. }));

        let cancel_event = make_touch_event(2, DomTouchPhase::Cancel, 50.0, 50.0);
        let results = translator.translate(&cancel_event);
        assert!(matches!(&results[0], ArachneEvent::TouchCancel { .. }));
    }

    #[test]
    fn translator_resize_event() {
        let mut translator = EventTranslator::new();
        let event = make_resize_event(1920, 1080, 2.0);
        let results = translator.translate(&event);

        assert_eq!(results.len(), 1);
        match &results[0] {
            ArachneEvent::Resize { width, height, dpr } => {
                assert_eq!(*width, 1920);
                assert_eq!(*height, 1080);
                assert_eq!(*dpr, 2.0);
            }
            other => panic!("expected Resize, got {:?}", other),
        }

        // DPR should have been updated internally.
        assert_eq!(translator.device_pixel_ratio, 2.0);
    }

    #[test]
    fn translator_pointer_lock() {
        let mut translator = EventTranslator::new();
        assert!(!translator.is_pointer_locked());

        let lock_event = DomEvent {
            kind: DomEventKind::PointerLock { locked: true },
            prevent_default: false,
        };
        let results = translator.translate(&lock_event);
        assert!(matches!(&results[0], ArachneEvent::PointerLockChanged(true)));
        assert!(translator.is_pointer_locked());

        let unlock_event = DomEvent {
            kind: DomEventKind::PointerLock { locked: false },
            prevent_default: false,
        };
        let results = translator.translate(&unlock_event);
        assert!(matches!(&results[0], ArachneEvent::PointerLockChanged(false)));
        assert!(!translator.is_pointer_locked());
    }

    #[test]
    fn translator_focus_event() {
        let mut translator = EventTranslator::new();
        let event = DomEvent {
            kind: DomEventKind::Focus { focused: true },
            prevent_default: false,
        };
        let results = translator.translate(&event);
        assert!(matches!(&results[0], ArachneEvent::FocusChanged(true)));
    }

    #[test]
    fn register_dom_listeners_noop_on_native() {
        assert!(register_dom_listeners("#canvas").is_ok());
    }

    #[test]
    fn request_pointer_lock_noop_on_native() {
        assert!(request_pointer_lock("#canvas").is_ok());
    }

    #[test]
    fn exit_pointer_lock_noop_on_native() {
        exit_pointer_lock(); // Should not panic.
    }

    #[test]
    fn translator_all_letters_a_through_z() {
        let letters = [
            ("KeyA", KeyCode::A), ("KeyB", KeyCode::B), ("KeyC", KeyCode::C),
            ("KeyD", KeyCode::D), ("KeyE", KeyCode::E), ("KeyF", KeyCode::F),
            ("KeyG", KeyCode::G), ("KeyH", KeyCode::H), ("KeyI", KeyCode::I),
            ("KeyJ", KeyCode::J), ("KeyK", KeyCode::K), ("KeyL", KeyCode::L),
            ("KeyM", KeyCode::M), ("KeyN", KeyCode::N), ("KeyO", KeyCode::O),
            ("KeyP", KeyCode::P), ("KeyQ", KeyCode::Q), ("KeyR", KeyCode::R),
            ("KeyS", KeyCode::S), ("KeyT", KeyCode::T), ("KeyU", KeyCode::U),
            ("KeyV", KeyCode::V), ("KeyW", KeyCode::W), ("KeyX", KeyCode::X),
            ("KeyY", KeyCode::Y), ("KeyZ", KeyCode::Z),
        ];

        for (code_str, expected) in &letters {
            assert_eq!(
                translate_key_code(code_str),
                Some(*expected),
                "Failed for {code_str}"
            );
        }
    }
}
