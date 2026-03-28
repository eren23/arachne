//! Bridge layer translating winit window events into arachne InputSystem events.
//!
//! All code in this module is gated behind the `windowed` feature.

use crate::keyboard::KeyCode;
use crate::mouse::MouseButton;
use crate::platform::{InputSystem, PlatformInput};
use crate::touch::TouchPhase;
use arachne_math::Vec2;
use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
use winit::keyboard::{KeyCode as WinitKeyCode, PhysicalKey};

/// Translate a winit [`KeyCode`](WinitKeyCode) into an arachne [`KeyCode`].
///
/// Returns `None` for unmapped keys.
pub fn translate_key(winit_key: WinitKeyCode) -> Option<KeyCode> {
    Some(match winit_key {
        // Letters (26)
        WinitKeyCode::KeyA => KeyCode::A,
        WinitKeyCode::KeyB => KeyCode::B,
        WinitKeyCode::KeyC => KeyCode::C,
        WinitKeyCode::KeyD => KeyCode::D,
        WinitKeyCode::KeyE => KeyCode::E,
        WinitKeyCode::KeyF => KeyCode::F,
        WinitKeyCode::KeyG => KeyCode::G,
        WinitKeyCode::KeyH => KeyCode::H,
        WinitKeyCode::KeyI => KeyCode::I,
        WinitKeyCode::KeyJ => KeyCode::J,
        WinitKeyCode::KeyK => KeyCode::K,
        WinitKeyCode::KeyL => KeyCode::L,
        WinitKeyCode::KeyM => KeyCode::M,
        WinitKeyCode::KeyN => KeyCode::N,
        WinitKeyCode::KeyO => KeyCode::O,
        WinitKeyCode::KeyP => KeyCode::P,
        WinitKeyCode::KeyQ => KeyCode::Q,
        WinitKeyCode::KeyR => KeyCode::R,
        WinitKeyCode::KeyS => KeyCode::S,
        WinitKeyCode::KeyT => KeyCode::T,
        WinitKeyCode::KeyU => KeyCode::U,
        WinitKeyCode::KeyV => KeyCode::V,
        WinitKeyCode::KeyW => KeyCode::W,
        WinitKeyCode::KeyX => KeyCode::X,
        WinitKeyCode::KeyY => KeyCode::Y,
        WinitKeyCode::KeyZ => KeyCode::Z,

        // Digits (10)
        WinitKeyCode::Digit0 => KeyCode::Key0,
        WinitKeyCode::Digit1 => KeyCode::Key1,
        WinitKeyCode::Digit2 => KeyCode::Key2,
        WinitKeyCode::Digit3 => KeyCode::Key3,
        WinitKeyCode::Digit4 => KeyCode::Key4,
        WinitKeyCode::Digit5 => KeyCode::Key5,
        WinitKeyCode::Digit6 => KeyCode::Key6,
        WinitKeyCode::Digit7 => KeyCode::Key7,
        WinitKeyCode::Digit8 => KeyCode::Key8,
        WinitKeyCode::Digit9 => KeyCode::Key9,

        // Arrow keys (4)
        WinitKeyCode::ArrowUp => KeyCode::Up,
        WinitKeyCode::ArrowDown => KeyCode::Down,
        WinitKeyCode::ArrowLeft => KeyCode::Left,
        WinitKeyCode::ArrowRight => KeyCode::Right,

        // Modifiers (8)
        WinitKeyCode::ShiftLeft => KeyCode::LeftShift,
        WinitKeyCode::ShiftRight => KeyCode::RightShift,
        WinitKeyCode::ControlLeft => KeyCode::LeftCtrl,
        WinitKeyCode::ControlRight => KeyCode::RightCtrl,
        WinitKeyCode::AltLeft => KeyCode::LeftAlt,
        WinitKeyCode::AltRight => KeyCode::RightAlt,
        WinitKeyCode::SuperLeft => KeyCode::LeftSuper,
        WinitKeyCode::SuperRight => KeyCode::RightSuper,

        // Function keys (12)
        WinitKeyCode::F1 => KeyCode::F1,
        WinitKeyCode::F2 => KeyCode::F2,
        WinitKeyCode::F3 => KeyCode::F3,
        WinitKeyCode::F4 => KeyCode::F4,
        WinitKeyCode::F5 => KeyCode::F5,
        WinitKeyCode::F6 => KeyCode::F6,
        WinitKeyCode::F7 => KeyCode::F7,
        WinitKeyCode::F8 => KeyCode::F8,
        WinitKeyCode::F9 => KeyCode::F9,
        WinitKeyCode::F10 => KeyCode::F10,
        WinitKeyCode::F11 => KeyCode::F11,
        WinitKeyCode::F12 => KeyCode::F12,

        // Common keys (11)
        WinitKeyCode::Space => KeyCode::Space,
        WinitKeyCode::Enter => KeyCode::Enter,
        WinitKeyCode::Escape => KeyCode::Escape,
        WinitKeyCode::Tab => KeyCode::Tab,
        WinitKeyCode::Backspace => KeyCode::Backspace,
        WinitKeyCode::Delete => KeyCode::Delete,
        WinitKeyCode::Insert => KeyCode::Insert,
        WinitKeyCode::Home => KeyCode::Home,
        WinitKeyCode::End => KeyCode::End,
        WinitKeyCode::PageUp => KeyCode::PageUp,
        WinitKeyCode::PageDown => KeyCode::PageDown,

        // Lock & special keys (5)
        WinitKeyCode::CapsLock => KeyCode::CapsLock,
        WinitKeyCode::NumLock => KeyCode::NumLock,
        WinitKeyCode::ScrollLock => KeyCode::ScrollLock,
        WinitKeyCode::PrintScreen => KeyCode::PrintScreen,
        WinitKeyCode::Pause => KeyCode::Pause,

        // Punctuation / symbols (11)
        WinitKeyCode::Comma => KeyCode::Comma,
        WinitKeyCode::Period => KeyCode::Period,
        WinitKeyCode::Slash => KeyCode::Slash,
        WinitKeyCode::Backslash => KeyCode::Backslash,
        WinitKeyCode::Semicolon => KeyCode::Semicolon,
        WinitKeyCode::Quote => KeyCode::Apostrophe,
        WinitKeyCode::BracketLeft => KeyCode::LeftBracket,
        WinitKeyCode::BracketRight => KeyCode::RightBracket,
        WinitKeyCode::Backquote => KeyCode::GraveAccent,
        WinitKeyCode::Minus => KeyCode::Minus,
        WinitKeyCode::Equal => KeyCode::Equal,

        // Numpad (16)
        WinitKeyCode::Numpad0 => KeyCode::Numpad0,
        WinitKeyCode::Numpad1 => KeyCode::Numpad1,
        WinitKeyCode::Numpad2 => KeyCode::Numpad2,
        WinitKeyCode::Numpad3 => KeyCode::Numpad3,
        WinitKeyCode::Numpad4 => KeyCode::Numpad4,
        WinitKeyCode::Numpad5 => KeyCode::Numpad5,
        WinitKeyCode::Numpad6 => KeyCode::Numpad6,
        WinitKeyCode::Numpad7 => KeyCode::Numpad7,
        WinitKeyCode::Numpad8 => KeyCode::Numpad8,
        WinitKeyCode::Numpad9 => KeyCode::Numpad9,
        WinitKeyCode::NumpadAdd => KeyCode::NumpadAdd,
        WinitKeyCode::NumpadSubtract => KeyCode::NumpadSubtract,
        WinitKeyCode::NumpadMultiply => KeyCode::NumpadMultiply,
        WinitKeyCode::NumpadDivide => KeyCode::NumpadDivide,
        WinitKeyCode::NumpadEnter => KeyCode::NumpadEnter,
        WinitKeyCode::NumpadDecimal => KeyCode::NumpadDecimal,

        _ => return None,
    })
}

/// Translate a winit [`MouseButton`](winit::event::MouseButton) into an arachne [`MouseButton`].
///
/// Returns `None` for unmapped buttons (e.g. `Other(id)`).
pub fn translate_mouse_button(btn: winit::event::MouseButton) -> Option<MouseButton> {
    match btn {
        winit::event::MouseButton::Left => Some(MouseButton::Left),
        winit::event::MouseButton::Right => Some(MouseButton::Right),
        winit::event::MouseButton::Middle => Some(MouseButton::Middle),
        winit::event::MouseButton::Back => Some(MouseButton::X1),
        winit::event::MouseButton::Forward => Some(MouseButton::X2),
        winit::event::MouseButton::Other(_) => None,
    }
}

/// Process a winit [`WindowEvent`], translating it into arachne input state updates.
///
/// Unrecognised events are silently ignored.
pub fn process_window_event(input: &mut InputSystem, event: &WindowEvent) {
    match event {
        WindowEvent::KeyboardInput {
            event: key_event, ..
        } => {
            if let PhysicalKey::Code(code) = key_event.physical_key {
                if let Some(key) = translate_key(code) {
                    let pressed = key_event.state == ElementState::Pressed;
                    input.process_keyboard_event(key, pressed);
                }
            }
        }
        WindowEvent::MouseInput {
            button, state, ..
        } => {
            if let Some(btn) = translate_mouse_button(*button) {
                let pressed = *state == ElementState::Pressed;
                input.process_mouse_event(btn, pressed);
            }
        }
        WindowEvent::CursorMoved { position, .. } => {
            let pos = Vec2::new(position.x as f32, position.y as f32);
            input.process_mouse_move(pos);
        }
        WindowEvent::MouseWheel { delta, .. } => {
            let scroll = match delta {
                MouseScrollDelta::LineDelta(x, y) => Vec2::new(*x, *y),
                MouseScrollDelta::PixelDelta(pos) => Vec2::new(pos.x as f32, pos.y as f32),
            };
            input.process_mouse_scroll(scroll);
        }
        WindowEvent::Touch(touch) => {
            let phase = match touch.phase {
                winit::event::TouchPhase::Started => TouchPhase::Started,
                winit::event::TouchPhase::Moved => TouchPhase::Moved,
                winit::event::TouchPhase::Ended => TouchPhase::Ended,
                winit::event::TouchPhase::Cancelled => TouchPhase::Cancelled,
            };
            let pos = Vec2::new(touch.location.x as f32, touch.location.y as f32);
            input.process_touch_event(touch.id, pos, phase);
        }
        WindowEvent::Resized(size) => {
            input.window_size = Vec2::new(size.width as f32, size.height as f32);
        }
        WindowEvent::Focused(focused) => {
            input.has_focus = *focused;
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::ManuallyDrop;
    use winit::dpi::{PhysicalPosition, PhysicalSize};
    use winit::event::DeviceId;

    /// Create a `WindowEvent::KeyboardInput` for testing.
    ///
    /// Uses `ManuallyDrop` because `KeyEvent` contains a `pub(crate)` field
    /// (`platform_specific`) that cannot be constructed externally. We zero
    /// the entire struct and overwrite the public fields we need, then prevent
    /// the drop so the zeroed platform data is never touched by a destructor.
    fn make_keyboard_event(
        code: WinitKeyCode,
        state: ElementState,
    ) -> ManuallyDrop<WindowEvent> {
        let key_event = unsafe {
            let mut e: winit::event::KeyEvent = std::mem::zeroed();
            e.physical_key = PhysicalKey::Code(code);
            e.state = state;
            e.repeat = false;
            e
        };
        ManuallyDrop::new(WindowEvent::KeyboardInput {
            device_id: DeviceId::dummy(),
            event: key_event,
            is_synthetic: false,
        })
    }

    // -- translate_key coverage -----------------------------------------------

    #[test]
    fn translate_key_letters() {
        assert_eq!(translate_key(WinitKeyCode::KeyA), Some(KeyCode::A));
        assert_eq!(translate_key(WinitKeyCode::KeyM), Some(KeyCode::M));
        assert_eq!(translate_key(WinitKeyCode::KeyZ), Some(KeyCode::Z));
    }

    #[test]
    fn translate_key_digits() {
        assert_eq!(translate_key(WinitKeyCode::Digit0), Some(KeyCode::Key0));
        assert_eq!(translate_key(WinitKeyCode::Digit5), Some(KeyCode::Key5));
        assert_eq!(translate_key(WinitKeyCode::Digit9), Some(KeyCode::Key9));
    }

    #[test]
    fn translate_key_arrows() {
        assert_eq!(translate_key(WinitKeyCode::ArrowUp), Some(KeyCode::Up));
        assert_eq!(translate_key(WinitKeyCode::ArrowDown), Some(KeyCode::Down));
        assert_eq!(translate_key(WinitKeyCode::ArrowLeft), Some(KeyCode::Left));
        assert_eq!(
            translate_key(WinitKeyCode::ArrowRight),
            Some(KeyCode::Right)
        );
    }

    #[test]
    fn translate_key_common() {
        assert_eq!(translate_key(WinitKeyCode::Space), Some(KeyCode::Space));
        assert_eq!(translate_key(WinitKeyCode::Enter), Some(KeyCode::Enter));
        assert_eq!(translate_key(WinitKeyCode::Escape), Some(KeyCode::Escape));
    }

    #[test]
    fn translate_key_modifiers() {
        assert_eq!(
            translate_key(WinitKeyCode::ShiftLeft),
            Some(KeyCode::LeftShift)
        );
        assert_eq!(
            translate_key(WinitKeyCode::ShiftRight),
            Some(KeyCode::RightShift)
        );
        assert_eq!(
            translate_key(WinitKeyCode::ControlLeft),
            Some(KeyCode::LeftCtrl)
        );
        assert_eq!(
            translate_key(WinitKeyCode::ControlRight),
            Some(KeyCode::RightCtrl)
        );
        assert_eq!(
            translate_key(WinitKeyCode::AltLeft),
            Some(KeyCode::LeftAlt)
        );
        assert_eq!(
            translate_key(WinitKeyCode::AltRight),
            Some(KeyCode::RightAlt)
        );
    }

    #[test]
    fn translate_key_function_keys() {
        assert_eq!(translate_key(WinitKeyCode::F1), Some(KeyCode::F1));
        assert_eq!(translate_key(WinitKeyCode::F6), Some(KeyCode::F6));
        assert_eq!(translate_key(WinitKeyCode::F12), Some(KeyCode::F12));
    }

    #[test]
    fn translate_key_unknown_returns_none() {
        // ContextMenu is not mapped in arachne
        assert_eq!(translate_key(WinitKeyCode::ContextMenu), None);
    }

    // -- translate_mouse_button coverage --------------------------------------

    #[test]
    fn translate_mouse_button_left() {
        assert_eq!(
            translate_mouse_button(winit::event::MouseButton::Left),
            Some(MouseButton::Left)
        );
    }

    #[test]
    fn translate_mouse_button_right() {
        assert_eq!(
            translate_mouse_button(winit::event::MouseButton::Right),
            Some(MouseButton::Right)
        );
    }

    #[test]
    fn translate_mouse_button_middle() {
        assert_eq!(
            translate_mouse_button(winit::event::MouseButton::Middle),
            Some(MouseButton::Middle)
        );
    }

    // -- process_window_event -------------------------------------------------

    #[test]
    fn keyboard_input_pressed() {
        let mut input = InputSystem::new();
        let event = make_keyboard_event(WinitKeyCode::KeyA, ElementState::Pressed);
        process_window_event(&mut input, &event);
        assert!(input.keyboard.pressed(KeyCode::A));
    }

    #[test]
    fn keyboard_input_released() {
        let mut input = InputSystem::new();

        // Press first so there is state to release
        let press = make_keyboard_event(WinitKeyCode::KeyW, ElementState::Pressed);
        process_window_event(&mut input, &press);
        assert!(input.keyboard.pressed(KeyCode::W));

        let release = make_keyboard_event(WinitKeyCode::KeyW, ElementState::Released);
        process_window_event(&mut input, &release);
        assert!(!input.keyboard.pressed(KeyCode::W));
    }

    #[test]
    fn cursor_moved() {
        let mut input = InputSystem::new();
        let event = WindowEvent::CursorMoved {
            device_id: DeviceId::dummy(),
            position: PhysicalPosition::new(320.0, 240.0),
        };
        process_window_event(&mut input, &event);
        assert_eq!(input.mouse.position(), Vec2::new(320.0, 240.0));
    }

    #[test]
    fn mouse_wheel_line_delta() {
        let mut input = InputSystem::new();
        let event = WindowEvent::MouseWheel {
            device_id: DeviceId::dummy(),
            delta: MouseScrollDelta::LineDelta(0.0, -3.0),
            phase: winit::event::TouchPhase::Moved,
        };
        process_window_event(&mut input, &event);
        assert_eq!(input.mouse.scroll(), Vec2::new(0.0, -3.0));
    }

    #[test]
    fn resized() {
        let mut input = InputSystem::new();
        let event = WindowEvent::Resized(PhysicalSize::new(1920, 1080));
        process_window_event(&mut input, &event);
        assert_eq!(input.window_size, Vec2::new(1920.0, 1080.0));
    }

    #[test]
    fn focused_false() {
        let mut input = InputSystem::new();
        assert!(input.has_focus);
        process_window_event(&mut input, &WindowEvent::Focused(false));
        assert!(!input.has_focus);
    }

    #[test]
    fn unhandled_event_no_panic() {
        let mut input = InputSystem::new();
        // CloseRequested is not handled by the bridge
        process_window_event(&mut input, &WindowEvent::CloseRequested);
    }
}
