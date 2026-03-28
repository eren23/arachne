use std::collections::HashMap;

use crate::gamepad::{GamepadAxis, GamepadButton, GamepadState};
use crate::keyboard::{KeyCode, KeyboardState};
use crate::mouse::{MouseButton, MouseState};
use crate::touch::TouchState;

// ---------------------------------------------------------------------------
// InputBinding
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub enum InputBinding {
    Key(KeyCode),
    Mouse(MouseButton),
    Gamepad(GamepadButton),
    Touch,
}

// ---------------------------------------------------------------------------
// AxisBinding – maps an abstract axis to input sources
// ---------------------------------------------------------------------------

/// Describes how to derive a floating-point axis value from input.
#[derive(Clone, Debug, PartialEq)]
pub enum AxisBinding {
    /// A gamepad analog axis (value -1..1).
    GamepadAxis(GamepadAxis),
    /// Two keys forming a virtual axis: negative key and positive key.
    KeyboardAxis { negative: KeyCode, positive: KeyCode },
    /// Mouse X or Y delta.
    MouseDelta(MouseAxisKind),
}

/// Which axis of the mouse to read for `AxisBinding::MouseDelta`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseAxisKind {
    X,
    Y,
    ScrollX,
    ScrollY,
}

// ---------------------------------------------------------------------------
// ActionState – tracks the state of a single action
// ---------------------------------------------------------------------------

/// Tracks whether an action is currently active, just activated, or just
/// deactivated within a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ActionState {
    pub active: bool,
    pub just_activated: bool,
    pub just_deactivated: bool,
}

impl ActionState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Update this state based on the new `pressed` value.
    pub fn update(&mut self, pressed: bool) {
        let was_active = self.active;
        self.active = pressed;
        self.just_activated = pressed && !was_active;
        self.just_deactivated = !pressed && was_active;
    }
}

// ---------------------------------------------------------------------------
// ActionMap
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default)]
pub struct ActionMap {
    actions: HashMap<String, Vec<InputBinding>>,
    axis_bindings: HashMap<String, Vec<AxisBinding>>,
    action_states: HashMap<String, ActionState>,
    axis_values: HashMap<String, f32>,
    /// Dead zone for axis values (values below this magnitude are clamped to 0).
    pub axis_deadzone: f32,
}

impl ActionMap {
    pub fn new() -> Self {
        Self {
            axis_deadzone: 0.1,
            ..Self::default()
        }
    }

    // -- Button actions ---------------------------------------------------

    pub fn add_binding(&mut self, action: &str, binding: InputBinding) {
        self.actions
            .entry(action.to_string())
            .or_default()
            .push(binding);
    }

    pub fn remove_binding(&mut self, action: &str, binding: &InputBinding) {
        if let Some(bindings) = self.actions.get_mut(action) {
            bindings.retain(|b| b != binding);
        }
    }

    pub fn bindings(&self, action: &str) -> &[InputBinding] {
        self.actions
            .get(action)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    pub fn clear_action(&mut self, action: &str) {
        self.actions.remove(action);
        self.action_states.remove(action);
    }

    pub fn clear_all(&mut self) {
        self.actions.clear();
        self.axis_bindings.clear();
        self.action_states.clear();
        self.axis_values.clear();
    }

    /// Returns all registered action names.
    pub fn action_names(&self) -> Vec<&str> {
        self.actions.keys().map(|s| s.as_str()).collect()
    }

    pub fn pressed(
        &self,
        action: &str,
        keyboard: &KeyboardState,
        mouse: &MouseState,
        gamepad: &GamepadState,
        touch: &TouchState,
    ) -> bool {
        let bindings = match self.actions.get(action) {
            Some(b) => b,
            None => return false,
        };
        bindings.iter().any(|b| match b {
            InputBinding::Key(key) => keyboard.pressed(*key),
            InputBinding::Mouse(btn) => mouse.pressed(*btn),
            InputBinding::Gamepad(btn) => gamepad.pressed(*btn),
            InputBinding::Touch => touch.any_touch_active(),
        })
    }

    pub fn just_pressed(
        &self,
        action: &str,
        keyboard: &KeyboardState,
        mouse: &MouseState,
        gamepad: &GamepadState,
        touch: &TouchState,
    ) -> bool {
        let bindings = match self.actions.get(action) {
            Some(b) => b,
            None => return false,
        };
        bindings.iter().any(|b| match b {
            InputBinding::Key(key) => keyboard.just_pressed(*key),
            InputBinding::Mouse(btn) => mouse.just_pressed(*btn),
            InputBinding::Gamepad(btn) => gamepad.just_pressed(*btn),
            InputBinding::Touch => touch.any_touch_active(),
        })
    }

    /// Query the cached action state (requires `update_states` to be called).
    pub fn action_state(&self, action: &str) -> ActionState {
        self.action_states
            .get(action)
            .copied()
            .unwrap_or_default()
    }

    // -- Axis bindings ----------------------------------------------------

    pub fn add_axis_binding(&mut self, name: &str, binding: AxisBinding) {
        self.axis_bindings
            .entry(name.to_string())
            .or_default()
            .push(binding);
    }

    pub fn remove_axis_binding(&mut self, name: &str, binding: &AxisBinding) {
        if let Some(bindings) = self.axis_bindings.get_mut(name) {
            bindings.retain(|b| b != binding);
        }
    }

    /// Returns all registered axis names.
    pub fn axis_names(&self) -> Vec<&str> {
        self.axis_bindings.keys().map(|s| s.as_str()).collect()
    }

    /// Read the current axis value for a named axis.
    pub fn axis(
        &self,
        name: &str,
        keyboard: &KeyboardState,
        mouse: &MouseState,
        gamepad: &GamepadState,
    ) -> f32 {
        let bindings = match self.axis_bindings.get(name) {
            Some(b) => b,
            None => return 0.0,
        };

        let mut value = 0.0f32;
        for binding in bindings {
            let v = match binding {
                AxisBinding::GamepadAxis(axis) => gamepad.axis(*axis),
                AxisBinding::KeyboardAxis { negative, positive } => {
                    let neg = if keyboard.pressed(*negative) { -1.0 } else { 0.0 };
                    let pos = if keyboard.pressed(*positive) { 1.0 } else { 0.0 };
                    neg + pos
                }
                AxisBinding::MouseDelta(kind) => match kind {
                    MouseAxisKind::X => mouse.delta().x,
                    MouseAxisKind::Y => mouse.delta().y,
                    MouseAxisKind::ScrollX => mouse.scroll().x,
                    MouseAxisKind::ScrollY => mouse.scroll().y,
                },
            };
            // Take the value with the largest magnitude
            if v.abs() > value.abs() {
                value = v;
            }
        }

        // Apply deadzone
        if value.abs() < self.axis_deadzone {
            0.0
        } else {
            value.clamp(-1.0, 1.0)
        }
    }

    /// Query the cached axis value (requires `update_axes` to be called).
    pub fn axis_value(&self, name: &str) -> f32 {
        self.axis_values.get(name).copied().unwrap_or(0.0)
    }

    // -- Frame update -----------------------------------------------------

    /// Update all cached action states. Call once per frame.
    pub fn update_states(
        &mut self,
        keyboard: &KeyboardState,
        mouse: &MouseState,
        gamepad: &GamepadState,
        touch: &TouchState,
    ) {
        let action_names: Vec<String> = self.actions.keys().cloned().collect();
        for action_name in &action_names {
            let pressed = self.pressed(action_name, keyboard, mouse, gamepad, touch);
            let state = self.action_states.entry(action_name.clone()).or_default();
            state.update(pressed);
        }
    }

    /// Update all cached axis values. Call once per frame.
    pub fn update_axes(
        &mut self,
        keyboard: &KeyboardState,
        mouse: &MouseState,
        gamepad: &GamepadState,
    ) {
        let axis_names: Vec<String> = self.axis_bindings.keys().cloned().collect();
        for name in &axis_names {
            let value = self.axis(name, keyboard, mouse, gamepad);
            self.axis_values.insert(name.clone(), value);
        }
    }
}

// ---------------------------------------------------------------------------
// InputPreset – common binding presets
// ---------------------------------------------------------------------------

/// Pre-built binding configurations for common control schemes.
pub struct InputPreset;

impl InputPreset {
    /// WASD movement: binds "move_x" and "move_y" axes.
    pub fn wasd_movement(map: &mut ActionMap) {
        map.add_axis_binding(
            "move_x",
            AxisBinding::KeyboardAxis {
                negative: KeyCode::A,
                positive: KeyCode::D,
            },
        );
        map.add_axis_binding(
            "move_y",
            AxisBinding::KeyboardAxis {
                negative: KeyCode::S,
                positive: KeyCode::W,
            },
        );
    }

    /// Arrow key movement: binds "move_x" and "move_y" axes.
    pub fn arrow_movement(map: &mut ActionMap) {
        map.add_axis_binding(
            "move_x",
            AxisBinding::KeyboardAxis {
                negative: KeyCode::Left,
                positive: KeyCode::Right,
            },
        );
        map.add_axis_binding(
            "move_y",
            AxisBinding::KeyboardAxis {
                negative: KeyCode::Down,
                positive: KeyCode::Up,
            },
        );
    }

    /// Gamepad left stick: binds "move_x" and "move_y" axes.
    pub fn gamepad_left_stick(map: &mut ActionMap) {
        map.add_axis_binding("move_x", AxisBinding::GamepadAxis(GamepadAxis::LeftStickX));
        map.add_axis_binding("move_y", AxisBinding::GamepadAxis(GamepadAxis::LeftStickY));
    }

    /// Common platformer bindings: jump (Space/South), move (WASD + left stick).
    pub fn platformer(map: &mut ActionMap) {
        Self::wasd_movement(map);
        Self::gamepad_left_stick(map);
        map.add_binding("jump", InputBinding::Key(KeyCode::Space));
        map.add_binding("jump", InputBinding::Gamepad(GamepadButton::South));
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use arachne_math::Vec2;
    use crate::touch::TouchPhase;

    #[test]
    fn bind_jump_to_space_and_gamepad_a() {
        let mut map = ActionMap::new();
        map.add_binding("jump", InputBinding::Key(KeyCode::Space));
        map.add_binding("jump", InputBinding::Gamepad(GamepadButton::South));

        let mut kb = KeyboardState::new();
        let mouse = MouseState::new();
        let mut gp = GamepadState::new();
        let touch = TouchState::new();

        // Neither pressed
        assert!(!map.pressed("jump", &kb, &mouse, &gp, &touch));

        // Space pressed
        kb.press(KeyCode::Space);
        assert!(map.pressed("jump", &kb, &mouse, &gp, &touch));

        // Reset keyboard, press gamepad
        kb = KeyboardState::new();
        gp.press(GamepadButton::South);
        assert!(map.pressed("jump", &kb, &mouse, &gp, &touch));
    }

    #[test]
    fn touch_binding() {
        let mut map = ActionMap::new();
        map.add_binding("tap", InputBinding::Touch);

        let kb = KeyboardState::new();
        let mouse = MouseState::new();
        let gp = GamepadState::new();
        let mut touch = TouchState::new();

        assert!(!map.pressed("tap", &kb, &mouse, &gp, &touch));

        touch.process_touch(1, Vec2::new(100.0, 100.0), TouchPhase::Started);
        assert!(map.pressed("tap", &kb, &mouse, &gp, &touch));
    }

    #[test]
    fn remove_binding() {
        let mut map = ActionMap::new();
        map.add_binding("shoot", InputBinding::Key(KeyCode::Space));
        map.add_binding("shoot", InputBinding::Mouse(MouseButton::Left));

        assert_eq!(map.bindings("shoot").len(), 2);

        map.remove_binding("shoot", &InputBinding::Key(KeyCode::Space));
        assert_eq!(map.bindings("shoot").len(), 1);
    }

    #[test]
    fn unknown_action_returns_false() {
        let map = ActionMap::new();
        let kb = KeyboardState::new();
        let mouse = MouseState::new();
        let gp = GamepadState::new();
        let touch = TouchState::new();
        assert!(!map.pressed("nonexistent", &kb, &mouse, &gp, &touch));
    }

    #[test]
    fn just_pressed_action() {
        let mut map = ActionMap::new();
        map.add_binding("jump", InputBinding::Key(KeyCode::Space));

        let mut kb = KeyboardState::new();
        let mouse = MouseState::new();
        let gp = GamepadState::new();
        let touch = TouchState::new();

        kb.press(KeyCode::Space);
        assert!(map.just_pressed("jump", &kb, &mouse, &gp, &touch));

        kb.begin_frame();
        assert!(!map.just_pressed("jump", &kb, &mouse, &gp, &touch));
        assert!(map.pressed("jump", &kb, &mouse, &gp, &touch));
    }

    #[test]
    fn mouse_binding() {
        let mut map = ActionMap::new();
        map.add_binding("shoot", InputBinding::Mouse(MouseButton::Left));

        let kb = KeyboardState::new();
        let mut mouse = MouseState::new();
        let gp = GamepadState::new();
        let touch = TouchState::new();

        mouse.press(MouseButton::Left);
        assert!(map.pressed("shoot", &kb, &mouse, &gp, &touch));
    }

    // -- Axis tests -------------------------------------------------------

    #[test]
    fn keyboard_axis_wasd() {
        let mut map = ActionMap::new();
        map.add_axis_binding(
            "move_x",
            AxisBinding::KeyboardAxis {
                negative: KeyCode::A,
                positive: KeyCode::D,
            },
        );

        let mut kb = KeyboardState::new();
        let mouse = MouseState::new();
        let gp = GamepadState::new();

        // No keys pressed
        assert_eq!(map.axis("move_x", &kb, &mouse, &gp), 0.0);

        // D pressed = positive
        kb.press(KeyCode::D);
        assert_eq!(map.axis("move_x", &kb, &mouse, &gp), 1.0);

        // A also pressed = 0 (cancel out)
        kb.press(KeyCode::A);
        assert_eq!(map.axis("move_x", &kb, &mouse, &gp), 0.0);

        // Only A pressed = negative
        kb = KeyboardState::new();
        kb.press(KeyCode::A);
        assert_eq!(map.axis("move_x", &kb, &mouse, &gp), -1.0);
    }

    #[test]
    fn gamepad_axis_binding() {
        let mut map = ActionMap::new();
        map.add_axis_binding("move_x", AxisBinding::GamepadAxis(GamepadAxis::LeftStickX));

        let kb = KeyboardState::new();
        let mouse = MouseState::new();
        let mut gp = GamepadState::new();

        gp.set_axis(GamepadAxis::LeftStickX, 0.75);
        assert_eq!(map.axis("move_x", &kb, &mouse, &gp), 0.75);
    }

    #[test]
    fn axis_deadzone() {
        let mut map = ActionMap::new();
        map.axis_deadzone = 0.2;
        map.add_axis_binding("move_x", AxisBinding::GamepadAxis(GamepadAxis::LeftStickX));

        let kb = KeyboardState::new();
        let mouse = MouseState::new();
        let mut gp = GamepadState::new();

        // Below deadzone
        gp.set_deadzone(0.0); // Disable gamepad's own deadzone
        gp.set_axis(GamepadAxis::LeftStickX, 0.15);
        assert_eq!(map.axis("move_x", &kb, &mouse, &gp), 0.0);

        // Above deadzone
        gp.set_axis(GamepadAxis::LeftStickX, 0.25);
        assert!((map.axis("move_x", &kb, &mouse, &gp) - 0.25).abs() < 0.01);
    }

    #[test]
    fn multiple_axis_sources_highest_magnitude_wins() {
        let mut map = ActionMap::new();
        map.add_axis_binding(
            "move_x",
            AxisBinding::KeyboardAxis {
                negative: KeyCode::A,
                positive: KeyCode::D,
            },
        );
        map.add_axis_binding("move_x", AxisBinding::GamepadAxis(GamepadAxis::LeftStickX));

        let mut kb = KeyboardState::new();
        let mouse = MouseState::new();
        let mut gp = GamepadState::new();

        // Keyboard: D=1.0, Gamepad: 0.5 -> keyboard wins (1.0)
        kb.press(KeyCode::D);
        gp.set_axis(GamepadAxis::LeftStickX, 0.5);
        assert_eq!(map.axis("move_x", &kb, &mouse, &gp), 1.0);
    }

    #[test]
    fn mouse_delta_axis() {
        let mut map = ActionMap::new();
        map.add_axis_binding("look_x", AxisBinding::MouseDelta(MouseAxisKind::X));

        let kb = KeyboardState::new();
        let mut mouse = MouseState::new();
        let gp = GamepadState::new();

        mouse.set_position(Vec2::new(10.0, 0.0));
        let v = map.axis("look_x", &kb, &mouse, &gp);
        // Delta should be 10.0 but clamped to 1.0
        assert_eq!(v, 1.0);
    }

    // -- ActionState tests ------------------------------------------------

    #[test]
    fn action_state_update() {
        let mut state = ActionState::new();
        assert!(!state.active);
        assert!(!state.just_activated);

        state.update(true);
        assert!(state.active);
        assert!(state.just_activated);
        assert!(!state.just_deactivated);

        state.update(true);
        assert!(state.active);
        assert!(!state.just_activated); // no longer "just"

        state.update(false);
        assert!(!state.active);
        assert!(state.just_deactivated);
        assert!(!state.just_activated);
    }

    #[test]
    fn update_states_caches_action_states() {
        let mut map = ActionMap::new();
        map.add_binding("jump", InputBinding::Key(KeyCode::Space));

        let mut kb = KeyboardState::new();
        let mouse = MouseState::new();
        let gp = GamepadState::new();
        let touch = TouchState::new();

        kb.press(KeyCode::Space);
        map.update_states(&kb, &mouse, &gp, &touch);

        let state = map.action_state("jump");
        assert!(state.active);
        assert!(state.just_activated);
    }

    #[test]
    fn update_axes_caches_values() {
        let mut map = ActionMap::new();
        map.add_axis_binding("move_x", AxisBinding::GamepadAxis(GamepadAxis::LeftStickX));

        let kb = KeyboardState::new();
        let mouse = MouseState::new();
        let mut gp = GamepadState::new();
        gp.set_axis(GamepadAxis::LeftStickX, 0.8);

        map.update_axes(&kb, &mouse, &gp);
        assert!((map.axis_value("move_x") - 0.8).abs() < 0.01);
    }

    // -- Preset tests -----------------------------------------------------

    #[test]
    fn preset_platformer() {
        let mut map = ActionMap::new();
        InputPreset::platformer(&mut map);

        assert_eq!(map.bindings("jump").len(), 2);
        assert!(!map.axis_names().is_empty());
    }

    #[test]
    fn clear_action() {
        let mut map = ActionMap::new();
        map.add_binding("fire", InputBinding::Key(KeyCode::Space));
        assert_eq!(map.bindings("fire").len(), 1);

        map.clear_action("fire");
        assert_eq!(map.bindings("fire").len(), 0);
    }

    #[test]
    fn clear_all() {
        let mut map = ActionMap::new();
        map.add_binding("fire", InputBinding::Key(KeyCode::Space));
        map.add_axis_binding("move_x", AxisBinding::GamepadAxis(GamepadAxis::LeftStickX));

        map.clear_all();
        assert!(map.action_names().is_empty());
        assert!(map.axis_names().is_empty());
    }

    #[test]
    fn unknown_axis_returns_zero() {
        let map = ActionMap::new();
        let kb = KeyboardState::new();
        let mouse = MouseState::new();
        let gp = GamepadState::new();
        assert_eq!(map.axis("nonexistent", &kb, &mouse, &gp), 0.0);
    }
}
