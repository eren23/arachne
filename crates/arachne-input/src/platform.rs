use arachne_math::Vec2;

use crate::gamepad::{GamepadAxis, GamepadButton, GamepadState};
use crate::input_map::{ActionMap, AxisBinding, InputBinding, MouseAxisKind};
use crate::keyboard::{KeyCode, KeyboardState};
use crate::mouse::{MouseButton, MouseState};
use crate::touch::{TouchPhase, TouchState};

// ---------------------------------------------------------------------------
// Platform event types
// ---------------------------------------------------------------------------

/// A unified input event from any platform source.
#[derive(Clone, Debug)]
pub enum InputEvent {
    KeyDown(KeyCode),
    KeyUp(KeyCode),
    MouseDown(MouseButton),
    MouseUp(MouseButton),
    MouseMove(Vec2),
    MouseScroll(Vec2),
    TouchStart { id: u64, position: Vec2 },
    TouchMove { id: u64, position: Vec2 },
    TouchEnd { id: u64, position: Vec2 },
    TouchCancel { id: u64, position: Vec2 },
    GamepadButtonDown(GamepadButton),
    GamepadButtonUp(GamepadButton),
    GamepadAxisMove(GamepadAxis, f32),
    GamepadConnected,
    GamepadDisconnected,
    /// Window/canvas resize event.
    WindowResize { width: f32, height: f32 },
    /// Window/canvas focus gained.
    FocusGained,
    /// Window/canvas focus lost.
    FocusLost,
}

// ---------------------------------------------------------------------------
// PlatformInput trait
// ---------------------------------------------------------------------------

pub trait PlatformInput {
    fn process_keyboard_event(&mut self, key: KeyCode, pressed: bool);
    fn process_mouse_event(&mut self, button: MouseButton, pressed: bool);
    fn process_mouse_move(&mut self, position: Vec2);
    fn process_mouse_scroll(&mut self, scroll: Vec2);
    fn process_touch_event(&mut self, id: u64, position: Vec2, phase: TouchPhase);
    fn process_gamepad_event(&mut self, button: GamepadButton, pressed: bool);
    fn process_gamepad_axis(&mut self, axis: GamepadAxis, value: f32);
    fn process_gamepad_connection(&mut self, connected: bool);
    fn begin_frame(&mut self);
}

// ---------------------------------------------------------------------------
// InputSystem
// ---------------------------------------------------------------------------

pub struct InputSystem {
    pub keyboard: KeyboardState,
    pub mouse: MouseState,
    pub touch: TouchState,
    pub gamepad: GamepadState,
    pub actions: ActionMap,
    /// Whether the window/canvas currently has focus.
    pub has_focus: bool,
    /// Window/canvas size.
    pub window_size: Vec2,
    /// Queued events for the current frame (for event-based consumption).
    event_queue: Vec<InputEvent>,
    /// Pixel-to-logical coordinate scale factor (DPI).
    pub dpi_scale: f32,
}

impl Default for InputSystem {
    fn default() -> Self {
        Self {
            keyboard: KeyboardState::new(),
            mouse: MouseState::new(),
            touch: TouchState::new(),
            gamepad: GamepadState::new(),
            actions: ActionMap::new(),
            has_focus: true,
            window_size: Vec2::new(800.0, 600.0),
            event_queue: Vec::new(),
            dpi_scale: 1.0,
        }
    }
}

impl InputSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_window_size(mut self, width: f32, height: f32) -> Self {
        self.window_size = Vec2::new(width, height);
        self
    }

    pub fn with_dpi_scale(mut self, scale: f32) -> Self {
        self.dpi_scale = scale;
        self
    }

    pub fn action_pressed(&self, action: &str) -> bool {
        self.actions
            .pressed(action, &self.keyboard, &self.mouse, &self.gamepad, &self.touch)
    }

    pub fn action_just_pressed(&self, action: &str) -> bool {
        self.actions
            .just_pressed(action, &self.keyboard, &self.mouse, &self.gamepad, &self.touch)
    }

    /// Read an axis value from the action map.
    pub fn axis(&self, name: &str) -> f32 {
        self.actions
            .axis(name, &self.keyboard, &self.mouse, &self.gamepad)
    }

    /// Process a unified InputEvent.
    pub fn process_event(&mut self, event: InputEvent) {
        match &event {
            InputEvent::KeyDown(key) => self.keyboard.press(*key),
            InputEvent::KeyUp(key) => self.keyboard.release(*key),
            InputEvent::MouseDown(btn) => self.mouse.press(*btn),
            InputEvent::MouseUp(btn) => self.mouse.release(*btn),
            InputEvent::MouseMove(pos) => {
                let logical = Vec2::new(pos.x / self.dpi_scale, pos.y / self.dpi_scale);
                self.mouse.set_position(logical);
            }
            InputEvent::MouseScroll(scroll) => self.mouse.set_scroll(*scroll),
            InputEvent::TouchStart { id, position } => {
                let logical = Vec2::new(position.x / self.dpi_scale, position.y / self.dpi_scale);
                self.touch.process_touch(*id, logical, TouchPhase::Started);
            }
            InputEvent::TouchMove { id, position } => {
                let logical = Vec2::new(position.x / self.dpi_scale, position.y / self.dpi_scale);
                self.touch.process_touch(*id, logical, TouchPhase::Moved);
            }
            InputEvent::TouchEnd { id, position } => {
                let logical = Vec2::new(position.x / self.dpi_scale, position.y / self.dpi_scale);
                self.touch.process_touch(*id, logical, TouchPhase::Ended);
            }
            InputEvent::TouchCancel { id, position } => {
                let logical = Vec2::new(position.x / self.dpi_scale, position.y / self.dpi_scale);
                self.touch
                    .process_touch(*id, logical, TouchPhase::Cancelled);
            }
            InputEvent::GamepadButtonDown(btn) => self.gamepad.press(*btn),
            InputEvent::GamepadButtonUp(btn) => self.gamepad.release(*btn),
            InputEvent::GamepadAxisMove(axis, value) => self.gamepad.set_axis(*axis, *value),
            InputEvent::GamepadConnected => self.gamepad.set_connected(true),
            InputEvent::GamepadDisconnected => self.gamepad.set_connected(false),
            InputEvent::WindowResize { width, height } => {
                self.window_size = Vec2::new(*width, *height);
            }
            InputEvent::FocusGained => self.has_focus = true,
            InputEvent::FocusLost => self.has_focus = false,
        }
        self.event_queue.push(event);
    }

    /// Drain all events queued this frame.
    pub fn drain_events(&mut self) -> Vec<InputEvent> {
        std::mem::take(&mut self.event_queue)
    }

    /// Read events without consuming them.
    pub fn events(&self) -> &[InputEvent] {
        &self.event_queue
    }

    /// Convert a screen-space position to logical coordinates.
    pub fn screen_to_logical(&self, screen_pos: Vec2) -> Vec2 {
        Vec2::new(
            screen_pos.x / self.dpi_scale,
            screen_pos.y / self.dpi_scale,
        )
    }

    /// Convert a logical position to normalized device coordinates (0..1).
    pub fn logical_to_normalized(&self, logical_pos: Vec2) -> Vec2 {
        Vec2::new(
            logical_pos.x / self.window_size.x,
            logical_pos.y / self.window_size.y,
        )
    }

    /// Update all cached action states and axis values.
    pub fn update_action_states(&mut self) {
        self.actions
            .update_states(&self.keyboard, &self.mouse, &self.gamepad, &self.touch);
        self.actions
            .update_axes(&self.keyboard, &self.mouse, &self.gamepad);
    }
}

impl PlatformInput for InputSystem {
    fn process_keyboard_event(&mut self, key: KeyCode, pressed: bool) {
        if pressed {
            self.keyboard.press(key);
        } else {
            self.keyboard.release(key);
        }
    }

    fn process_mouse_event(&mut self, button: MouseButton, pressed: bool) {
        if pressed {
            self.mouse.press(button);
        } else {
            self.mouse.release(button);
        }
    }

    fn process_mouse_move(&mut self, position: Vec2) {
        self.mouse.set_position(position);
    }

    fn process_mouse_scroll(&mut self, scroll: Vec2) {
        self.mouse.set_scroll(scroll);
    }

    fn process_touch_event(&mut self, id: u64, position: Vec2, phase: TouchPhase) {
        self.touch.process_touch(id, position, phase);
    }

    fn process_gamepad_event(&mut self, button: GamepadButton, pressed: bool) {
        if pressed {
            self.gamepad.press(button);
        } else {
            self.gamepad.release(button);
        }
    }

    fn process_gamepad_axis(&mut self, axis: GamepadAxis, value: f32) {
        self.gamepad.set_axis(axis, value);
    }

    fn process_gamepad_connection(&mut self, connected: bool) {
        self.gamepad.set_connected(connected);
    }

    fn begin_frame(&mut self) {
        self.keyboard.begin_frame();
        self.mouse.begin_frame();
        self.touch.begin_frame();
        self.gamepad.begin_frame();
        self.event_queue.clear();
    }
}

// ---------------------------------------------------------------------------
// Platform detection helpers
// ---------------------------------------------------------------------------

/// Detected platform kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlatformKind {
    Desktop,
    Mobile,
    Web,
    Unknown,
}

/// Detect the current platform at compile time.
pub const fn detect_platform() -> PlatformKind {
    #[cfg(target_arch = "wasm32")]
    {
        PlatformKind::Web
    }
    #[cfg(all(
        not(target_arch = "wasm32"),
        any(target_os = "ios", target_os = "android")
    ))]
    {
        PlatformKind::Mobile
    }
    #[cfg(all(
        not(target_arch = "wasm32"),
        not(target_os = "ios"),
        not(target_os = "android")
    ))]
    {
        PlatformKind::Desktop
    }
}

/// Returns suggested default input bindings for the detected platform.
pub fn default_bindings_for_platform(platform: PlatformKind) -> ActionMap {
    let mut map = ActionMap::new();
    match platform {
        PlatformKind::Desktop => {
            crate::input_map::InputPreset::platformer(&mut map);
        }
        PlatformKind::Mobile | PlatformKind::Web => {
            // Touch-based defaults
            map.add_binding("jump", InputBinding::Touch);
            map.add_binding("jump", InputBinding::Key(KeyCode::Space));
        }
        PlatformKind::Unknown => {
            // Minimal defaults
            map.add_binding("jump", InputBinding::Key(KeyCode::Space));
        }
    }
    map
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input_map::InputBinding;

    #[test]
    fn platform_keyboard_event() {
        let mut sys = InputSystem::new();
        sys.process_keyboard_event(KeyCode::W, true);
        assert!(sys.keyboard.just_pressed(KeyCode::W));

        sys.process_keyboard_event(KeyCode::W, false);
        assert!(sys.keyboard.just_released(KeyCode::W));
    }

    #[test]
    fn platform_mouse_event() {
        let mut sys = InputSystem::new();
        sys.process_mouse_move(Vec2::new(100.0, 200.0));
        assert_eq!(sys.mouse.position(), Vec2::new(100.0, 200.0));

        sys.process_mouse_event(MouseButton::Left, true);
        assert!(sys.mouse.pressed(MouseButton::Left));
    }

    #[test]
    fn platform_touch_event() {
        let mut sys = InputSystem::new();
        sys.process_touch_event(1, Vec2::new(50.0, 50.0), TouchPhase::Started);
        assert_eq!(sys.touch.touch_count(), 1);

        sys.process_touch_event(1, Vec2::new(50.0, 50.0), TouchPhase::Ended);
        sys.begin_frame();
        assert_eq!(sys.touch.touch_count(), 0);
    }

    #[test]
    fn platform_gamepad_event() {
        let mut sys = InputSystem::new();
        sys.process_gamepad_connection(true);
        assert!(sys.gamepad.is_connected());

        sys.process_gamepad_event(GamepadButton::South, true);
        assert!(sys.gamepad.pressed(GamepadButton::South));

        sys.process_gamepad_axis(GamepadAxis::LeftStickX, 0.75);
        assert_eq!(sys.gamepad.axis(GamepadAxis::LeftStickX), 0.75);
    }

    #[test]
    fn begin_frame_resets_all() {
        let mut sys = InputSystem::new();

        sys.process_keyboard_event(KeyCode::Space, true);
        sys.process_mouse_event(MouseButton::Left, true);
        sys.process_gamepad_event(GamepadButton::South, true);
        sys.process_mouse_move(Vec2::new(10.0, 20.0));
        sys.process_mouse_scroll(Vec2::new(0.0, 1.0));

        assert!(sys.keyboard.just_pressed(KeyCode::Space));
        assert!(sys.mouse.just_pressed(MouseButton::Left));
        assert!(sys.gamepad.just_pressed(GamepadButton::South));

        sys.begin_frame();

        assert!(!sys.keyboard.just_pressed(KeyCode::Space));
        assert!(sys.keyboard.held(KeyCode::Space));
        assert!(!sys.mouse.just_pressed(MouseButton::Left));
        assert!(sys.mouse.held(MouseButton::Left));
        assert!(!sys.gamepad.just_pressed(GamepadButton::South));
        assert!(sys.gamepad.held(GamepadButton::South));
        assert_eq!(sys.mouse.delta(), Vec2::ZERO);
        assert_eq!(sys.mouse.scroll(), Vec2::ZERO);
    }

    #[test]
    fn action_map_integration() {
        let mut sys = InputSystem::new();
        sys.actions
            .add_binding("jump", InputBinding::Key(KeyCode::Space));
        sys.actions
            .add_binding("jump", InputBinding::Gamepad(GamepadButton::South));

        assert!(!sys.action_pressed("jump"));

        sys.process_keyboard_event(KeyCode::Space, true);
        assert!(sys.action_pressed("jump"));
        assert!(sys.action_just_pressed("jump"));

        sys.begin_frame();
        assert!(sys.action_pressed("jump"));
        assert!(!sys.action_just_pressed("jump"));
    }

    // -- Unified event processing -----------------------------------------

    #[test]
    fn process_event_unified() {
        let mut sys = InputSystem::new();

        sys.process_event(InputEvent::KeyDown(KeyCode::W));
        assert!(sys.keyboard.just_pressed(KeyCode::W));

        sys.process_event(InputEvent::KeyUp(KeyCode::W));
        assert!(sys.keyboard.just_released(KeyCode::W));

        sys.process_event(InputEvent::MouseDown(MouseButton::Left));
        assert!(sys.mouse.pressed(MouseButton::Left));

        sys.process_event(InputEvent::MouseMove(Vec2::new(100.0, 200.0)));
        assert_eq!(sys.mouse.position(), Vec2::new(100.0, 200.0));
    }

    #[test]
    fn event_queue_drains() {
        let mut sys = InputSystem::new();
        sys.process_event(InputEvent::KeyDown(KeyCode::A));
        sys.process_event(InputEvent::KeyDown(KeyCode::B));

        assert_eq!(sys.events().len(), 2);

        let events = sys.drain_events();
        assert_eq!(events.len(), 2);
        assert!(sys.events().is_empty());
    }

    #[test]
    fn window_resize_event() {
        let mut sys = InputSystem::new();
        sys.process_event(InputEvent::WindowResize {
            width: 1920.0,
            height: 1080.0,
        });
        assert_eq!(sys.window_size, Vec2::new(1920.0, 1080.0));
    }

    #[test]
    fn focus_events() {
        let mut sys = InputSystem::new();
        assert!(sys.has_focus);

        sys.process_event(InputEvent::FocusLost);
        assert!(!sys.has_focus);

        sys.process_event(InputEvent::FocusGained);
        assert!(sys.has_focus);
    }

    #[test]
    fn dpi_scaling_touch() {
        let mut sys = InputSystem::new().with_dpi_scale(2.0);
        sys.process_event(InputEvent::TouchStart {
            id: 1,
            position: Vec2::new(200.0, 400.0),
        });

        let touch = sys.touch.get_touch(1).unwrap();
        assert_eq!(touch.position, Vec2::new(100.0, 200.0)); // Scaled by 1/2
    }

    #[test]
    fn screen_to_logical_conversion() {
        let sys = InputSystem::new().with_dpi_scale(2.0);
        let logical = sys.screen_to_logical(Vec2::new(200.0, 400.0));
        assert_eq!(logical, Vec2::new(100.0, 200.0));
    }

    #[test]
    fn logical_to_normalized() {
        let sys = InputSystem::new().with_window_size(800.0, 600.0);
        let norm = sys.logical_to_normalized(Vec2::new(400.0, 300.0));
        assert!((norm.x - 0.5).abs() < 0.001);
        assert!((norm.y - 0.5).abs() < 0.001);
    }

    #[test]
    fn update_action_states() {
        let mut sys = InputSystem::new();
        sys.actions
            .add_binding("jump", InputBinding::Key(KeyCode::Space));
        sys.actions.add_axis_binding(
            "move_x",
            AxisBinding::KeyboardAxis {
                negative: KeyCode::A,
                positive: KeyCode::D,
            },
        );

        sys.process_keyboard_event(KeyCode::Space, true);
        sys.process_keyboard_event(KeyCode::D, true);
        sys.update_action_states();

        let state = sys.actions.action_state("jump");
        assert!(state.active);
        assert!(state.just_activated);

        let axis_val = sys.actions.axis_value("move_x");
        assert_eq!(axis_val, 1.0);
    }

    // -- Platform detection -----------------------------------------------

    #[test]
    fn detect_current_platform() {
        let platform = detect_platform();
        // On test hosts this should be Desktop
        assert_eq!(platform, PlatformKind::Desktop);
    }

    #[test]
    fn default_bindings_desktop() {
        let map = default_bindings_for_platform(PlatformKind::Desktop);
        assert!(map.bindings("jump").len() >= 1);
    }

    #[test]
    fn default_bindings_mobile() {
        let map = default_bindings_for_platform(PlatformKind::Mobile);
        assert!(map.bindings("jump").len() >= 1);
    }

    // -- Axis on InputSystem ----------------------------------------------

    #[test]
    fn input_system_axis() {
        let mut sys = InputSystem::new();
        sys.actions.add_axis_binding(
            "look_y",
            AxisBinding::MouseDelta(MouseAxisKind::ScrollY),
        );

        sys.process_mouse_scroll(Vec2::new(0.0, 0.5));
        let v = sys.axis("look_y");
        assert!((v - 0.5).abs() < 0.01);
    }

    #[test]
    fn begin_frame_clears_event_queue() {
        let mut sys = InputSystem::new();
        sys.process_event(InputEvent::KeyDown(KeyCode::A));
        assert_eq!(sys.events().len(), 1);

        sys.begin_frame();
        assert!(sys.events().is_empty());
    }
}
