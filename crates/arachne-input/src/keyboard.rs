use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyCode {
    // Letters
    A, B, C, D, E, F, G, H, I, J, K, L, M,
    N, O, P, Q, R, S, T, U, V, W, X, Y, Z,

    // Digits
    Key0, Key1, Key2, Key3, Key4, Key5, Key6, Key7, Key8, Key9,

    // Function keys
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,

    // Arrow keys
    Up, Down, Left, Right,

    // Modifiers
    LeftShift, RightShift,
    LeftCtrl, RightCtrl,
    LeftAlt, RightAlt,
    LeftSuper, RightSuper,

    // Whitespace / editing
    Space, Tab, Enter, Backspace, Delete, Insert,

    // Navigation
    Home, End, PageUp, PageDown,

    // Punctuation / symbols
    Escape, CapsLock, NumLock, ScrollLock, PrintScreen, Pause,
    Comma, Period, Slash, Backslash, Semicolon, Apostrophe,
    LeftBracket, RightBracket, GraveAccent, Minus, Equal,

    // Numpad
    Numpad0, Numpad1, Numpad2, Numpad3, Numpad4,
    Numpad5, Numpad6, Numpad7, Numpad8, Numpad9,
    NumpadAdd, NumpadSubtract, NumpadMultiply, NumpadDivide,
    NumpadEnter, NumpadDecimal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyState {
    JustPressed,
    Held,
    JustReleased,
    Idle,
}

impl Default for KeyState {
    fn default() -> Self {
        KeyState::Idle
    }
}

#[derive(Clone, Debug, Default)]
pub struct KeyboardState {
    keys: HashMap<KeyCode, KeyState>,
}

impl KeyboardState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn press(&mut self, key: KeyCode) {
        let state = self.keys.entry(key).or_default();
        match *state {
            KeyState::Idle | KeyState::JustReleased => *state = KeyState::JustPressed,
            _ => {}
        }
    }

    pub fn release(&mut self, key: KeyCode) {
        let state = self.keys.entry(key).or_default();
        match *state {
            KeyState::JustPressed | KeyState::Held => *state = KeyState::JustReleased,
            _ => {}
        }
    }

    pub fn just_pressed(&self, key: KeyCode) -> bool {
        self.keys.get(&key) == Some(&KeyState::JustPressed)
    }

    pub fn held(&self, key: KeyCode) -> bool {
        self.keys.get(&key) == Some(&KeyState::Held)
    }

    pub fn pressed(&self, key: KeyCode) -> bool {
        matches!(
            self.keys.get(&key),
            Some(&KeyState::JustPressed) | Some(&KeyState::Held)
        )
    }

    pub fn just_released(&self, key: KeyCode) -> bool {
        self.keys.get(&key) == Some(&KeyState::JustReleased)
    }

    pub fn released(&self, key: KeyCode) -> bool {
        !self.pressed(key)
    }

    pub fn begin_frame(&mut self) {
        self.keys.retain(|_, state| {
            match state {
                KeyState::JustPressed => {
                    *state = KeyState::Held;
                    true
                }
                KeyState::JustReleased => false, // remove, equivalent to Idle
                _ => true,
            }
        });
    }

    pub fn state(&self, key: KeyCode) -> KeyState {
        self.keys.get(&key).copied().unwrap_or(KeyState::Idle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn press_just_pressed_then_held() {
        let mut kb = KeyboardState::new();
        kb.press(KeyCode::Space);
        assert!(kb.just_pressed(KeyCode::Space));
        assert!(kb.pressed(KeyCode::Space));
        assert!(!kb.held(KeyCode::Space));

        kb.begin_frame();
        assert!(!kb.just_pressed(KeyCode::Space));
        assert!(kb.held(KeyCode::Space));
        assert!(kb.pressed(KeyCode::Space));
    }

    #[test]
    fn release_just_released_then_idle() {
        let mut kb = KeyboardState::new();
        kb.press(KeyCode::A);
        kb.begin_frame(); // JustPressed -> Held
        kb.release(KeyCode::A);
        assert!(kb.just_released(KeyCode::A));
        assert!(!kb.pressed(KeyCode::A));

        kb.begin_frame(); // JustReleased -> Idle
        assert!(!kb.just_released(KeyCode::A));
        assert_eq!(kb.state(KeyCode::A), KeyState::Idle);
    }

    #[test]
    fn unpressed_key_is_idle() {
        let kb = KeyboardState::new();
        assert_eq!(kb.state(KeyCode::W), KeyState::Idle);
        assert!(!kb.pressed(KeyCode::W));
        assert!(kb.released(KeyCode::W));
    }

    #[test]
    fn multiple_keys_independent() {
        let mut kb = KeyboardState::new();
        kb.press(KeyCode::W);
        kb.press(KeyCode::A);
        assert!(kb.just_pressed(KeyCode::W));
        assert!(kb.just_pressed(KeyCode::A));

        kb.begin_frame();
        kb.release(KeyCode::W);
        assert!(kb.just_released(KeyCode::W));
        assert!(kb.held(KeyCode::A));
    }

    #[test]
    fn frame_reset_clears_just_states() {
        let mut kb = KeyboardState::new();
        kb.press(KeyCode::Space);
        kb.begin_frame(); // JustPressed -> Held
        kb.release(KeyCode::Space);
        kb.begin_frame(); // JustReleased -> Idle

        assert!(!kb.just_pressed(KeyCode::Space));
        assert!(!kb.just_released(KeyCode::Space));
        assert!(!kb.pressed(KeyCode::Space));
    }

    #[test]
    fn double_press_no_change() {
        let mut kb = KeyboardState::new();
        kb.press(KeyCode::Enter);
        kb.press(KeyCode::Enter); // should not change state
        assert!(kb.just_pressed(KeyCode::Enter));
    }
}
