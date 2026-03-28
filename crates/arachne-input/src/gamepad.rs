use std::collections::HashMap;
use crate::keyboard::KeyState;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GamepadButton {
    South,  // A / Cross
    East,   // B / Circle
    West,   // X / Square
    North,  // Y / Triangle
    LeftBumper,
    RightBumper,
    LeftStick,
    RightStick,
    Start,
    Select,
    DPadUp,
    DPadDown,
    DPadLeft,
    DPadRight,
    Home,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GamepadAxis {
    LeftStickX,
    LeftStickY,
    RightStickX,
    RightStickY,
    LeftTrigger,
    RightTrigger,
}

#[derive(Clone, Debug)]
pub struct GamepadState {
    connected: bool,
    buttons: HashMap<GamepadButton, KeyState>,
    axes: HashMap<GamepadAxis, f32>,
    deadzone: f32,
}

impl Default for GamepadState {
    fn default() -> Self {
        Self {
            connected: false,
            buttons: HashMap::new(),
            axes: HashMap::new(),
            deadzone: 0.15,
        }
    }
}

impl GamepadState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_deadzone(mut self, deadzone: f32) -> Self {
        self.deadzone = deadzone;
        self
    }

    pub fn set_connected(&mut self, connected: bool) {
        self.connected = connected;
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    pub fn press(&mut self, button: GamepadButton) {
        let state = self.buttons.entry(button).or_insert(KeyState::Idle);
        match *state {
            KeyState::Idle | KeyState::JustReleased => *state = KeyState::JustPressed,
            _ => {}
        }
    }

    pub fn release(&mut self, button: GamepadButton) {
        let state = self.buttons.entry(button).or_insert(KeyState::Idle);
        match *state {
            KeyState::JustPressed | KeyState::Held => *state = KeyState::JustReleased,
            _ => {}
        }
    }

    pub fn just_pressed(&self, button: GamepadButton) -> bool {
        self.buttons.get(&button) == Some(&KeyState::JustPressed)
    }

    pub fn held(&self, button: GamepadButton) -> bool {
        self.buttons.get(&button) == Some(&KeyState::Held)
    }

    pub fn pressed(&self, button: GamepadButton) -> bool {
        matches!(
            self.buttons.get(&button),
            Some(&KeyState::JustPressed) | Some(&KeyState::Held)
        )
    }

    pub fn just_released(&self, button: GamepadButton) -> bool {
        self.buttons.get(&button) == Some(&KeyState::JustReleased)
    }

    pub fn set_axis(&mut self, axis: GamepadAxis, value: f32) {
        let clamped = value.clamp(-1.0, 1.0);
        let dead = if clamped.abs() < self.deadzone { 0.0 } else { clamped };
        self.axes.insert(axis, dead);
    }

    pub fn axis(&self, axis: GamepadAxis) -> f32 {
        self.axes.get(&axis).copied().unwrap_or(0.0)
    }

    pub fn deadzone(&self) -> f32 {
        self.deadzone
    }

    pub fn set_deadzone(&mut self, deadzone: f32) {
        self.deadzone = deadzone;
    }

    pub fn begin_frame(&mut self) {
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

    pub fn button_state(&self, button: GamepadButton) -> KeyState {
        self.buttons.get(&button).copied().unwrap_or(KeyState::Idle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axis_within_range() {
        let mut gp = GamepadState::new();
        gp.set_axis(GamepadAxis::LeftStickX, 0.5);
        assert_eq!(gp.axis(GamepadAxis::LeftStickX), 0.5);

        gp.set_axis(GamepadAxis::LeftStickX, 1.5);
        assert_eq!(gp.axis(GamepadAxis::LeftStickX), 1.0);

        gp.set_axis(GamepadAxis::LeftStickY, -2.0);
        assert_eq!(gp.axis(GamepadAxis::LeftStickY), -1.0);
    }

    #[test]
    fn deadzone_clips_small_values() {
        let mut gp = GamepadState::new(); // default deadzone 0.15
        gp.set_axis(GamepadAxis::LeftStickX, 0.1);
        assert_eq!(gp.axis(GamepadAxis::LeftStickX), 0.0);

        gp.set_axis(GamepadAxis::LeftStickX, -0.05);
        assert_eq!(gp.axis(GamepadAxis::LeftStickX), 0.0);

        gp.set_axis(GamepadAxis::LeftStickX, 0.2);
        assert_eq!(gp.axis(GamepadAxis::LeftStickX), 0.2);
    }

    #[test]
    fn custom_deadzone() {
        let mut gp = GamepadState::new().with_deadzone(0.3);
        gp.set_axis(GamepadAxis::RightStickX, 0.25);
        assert_eq!(gp.axis(GamepadAxis::RightStickX), 0.0);

        gp.set_axis(GamepadAxis::RightStickX, 0.35);
        assert_eq!(gp.axis(GamepadAxis::RightStickX), 0.35);
    }

    #[test]
    fn button_lifecycle() {
        let mut gp = GamepadState::new();
        gp.press(GamepadButton::South);
        assert!(gp.just_pressed(GamepadButton::South));

        gp.begin_frame();
        assert!(gp.held(GamepadButton::South));

        gp.release(GamepadButton::South);
        assert!(gp.just_released(GamepadButton::South));

        gp.begin_frame();
        assert_eq!(gp.button_state(GamepadButton::South), KeyState::Idle);
    }

    #[test]
    fn connected_disconnected() {
        let mut gp = GamepadState::new();
        assert!(!gp.is_connected());

        gp.set_connected(true);
        assert!(gp.is_connected());

        gp.set_connected(false);
        assert!(!gp.is_connected());
    }

    #[test]
    fn trigger_axis_values() {
        let mut gp = GamepadState::new();
        gp.set_axis(GamepadAxis::LeftTrigger, 0.8);
        gp.set_axis(GamepadAxis::RightTrigger, 1.0);
        assert_eq!(gp.axis(GamepadAxis::LeftTrigger), 0.8);
        assert_eq!(gp.axis(GamepadAxis::RightTrigger), 1.0);
    }

    #[test]
    fn unset_axis_returns_zero() {
        let gp = GamepadState::new();
        assert_eq!(gp.axis(GamepadAxis::LeftStickX), 0.0);
    }
}
