use std::collections::HashMap;
use arachne_math::Vec2;

use crate::keyboard::KeyState;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    X1,
    X2,
}

#[derive(Clone, Debug)]
pub struct MouseState {
    buttons: HashMap<MouseButton, KeyState>,
    position: Vec2,
    delta: Vec2,
    scroll: Vec2,
}

impl Default for MouseState {
    fn default() -> Self {
        Self {
            buttons: HashMap::new(),
            position: Vec2::ZERO,
            delta: Vec2::ZERO,
            scroll: Vec2::ZERO,
        }
    }
}

impl MouseState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn press(&mut self, button: MouseButton) {
        let state = self.buttons.entry(button).or_insert(KeyState::Idle);
        match *state {
            KeyState::Idle | KeyState::JustReleased => *state = KeyState::JustPressed,
            _ => {}
        }
    }

    pub fn release(&mut self, button: MouseButton) {
        let state = self.buttons.entry(button).or_insert(KeyState::Idle);
        match *state {
            KeyState::JustPressed | KeyState::Held => *state = KeyState::JustReleased,
            _ => {}
        }
    }

    pub fn just_pressed(&self, button: MouseButton) -> bool {
        self.buttons.get(&button) == Some(&KeyState::JustPressed)
    }

    pub fn held(&self, button: MouseButton) -> bool {
        self.buttons.get(&button) == Some(&KeyState::Held)
    }

    pub fn pressed(&self, button: MouseButton) -> bool {
        matches!(
            self.buttons.get(&button),
            Some(&KeyState::JustPressed) | Some(&KeyState::Held)
        )
    }

    pub fn just_released(&self, button: MouseButton) -> bool {
        self.buttons.get(&button) == Some(&KeyState::JustReleased)
    }

    pub fn released(&self, button: MouseButton) -> bool {
        !self.pressed(button)
    }

    pub fn set_position(&mut self, pos: Vec2) {
        self.delta = Vec2::new(pos.x - self.position.x, pos.y - self.position.y);
        self.position = pos;
    }

    pub fn set_scroll(&mut self, scroll: Vec2) {
        self.scroll = scroll;
    }

    pub fn position(&self) -> Vec2 {
        self.position
    }

    pub fn delta(&self) -> Vec2 {
        self.delta
    }

    pub fn scroll(&self) -> Vec2 {
        self.scroll
    }

    pub fn begin_frame(&mut self) {
        self.delta = Vec2::ZERO;
        self.scroll = Vec2::ZERO;
        self.buttons.retain(|_, state| {
            match state {
                KeyState::JustPressed => {
                    *state = KeyState::Held;
                    true
                }
                KeyState::JustReleased => false,
                _ => true,
            }
        });
    }

    pub fn button_state(&self, button: MouseButton) -> KeyState {
        self.buttons.get(&button).copied().unwrap_or(KeyState::Idle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_and_delta() {
        let mut mouse = MouseState::new();
        assert_eq!(mouse.position(), Vec2::ZERO);
        assert_eq!(mouse.delta(), Vec2::ZERO);

        mouse.set_position(Vec2::new(100.0, 200.0));
        assert_eq!(mouse.position(), Vec2::new(100.0, 200.0));
        assert_eq!(mouse.delta(), Vec2::new(100.0, 200.0));

        mouse.set_position(Vec2::new(110.0, 195.0));
        assert_eq!(mouse.position(), Vec2::new(110.0, 195.0));
        assert_eq!(mouse.delta(), Vec2::new(10.0, -5.0));
    }

    #[test]
    fn button_press_release_lifecycle() {
        let mut mouse = MouseState::new();
        mouse.press(MouseButton::Left);
        assert!(mouse.just_pressed(MouseButton::Left));
        assert!(mouse.pressed(MouseButton::Left));

        mouse.begin_frame();
        assert!(mouse.held(MouseButton::Left));
        assert!(!mouse.just_pressed(MouseButton::Left));

        mouse.release(MouseButton::Left);
        assert!(mouse.just_released(MouseButton::Left));

        mouse.begin_frame();
        assert!(!mouse.just_released(MouseButton::Left));
        assert_eq!(mouse.button_state(MouseButton::Left), KeyState::Idle);
    }

    #[test]
    fn scroll_delta() {
        let mut mouse = MouseState::new();
        mouse.set_scroll(Vec2::new(0.0, -3.0));
        assert_eq!(mouse.scroll(), Vec2::new(0.0, -3.0));

        mouse.begin_frame();
        assert_eq!(mouse.scroll(), Vec2::ZERO);
    }

    #[test]
    fn delta_resets_on_frame() {
        let mut mouse = MouseState::new();
        mouse.set_position(Vec2::new(50.0, 50.0));
        assert_eq!(mouse.delta(), Vec2::new(50.0, 50.0));

        mouse.begin_frame();
        assert_eq!(mouse.delta(), Vec2::ZERO);
        assert_eq!(mouse.position(), Vec2::new(50.0, 50.0));
    }

    #[test]
    fn multiple_buttons() {
        let mut mouse = MouseState::new();
        mouse.press(MouseButton::Left);
        mouse.press(MouseButton::Right);
        assert!(mouse.pressed(MouseButton::Left));
        assert!(mouse.pressed(MouseButton::Right));
        assert!(!mouse.pressed(MouseButton::Middle));
    }
}
