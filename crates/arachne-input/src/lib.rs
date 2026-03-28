pub mod keyboard;
pub mod mouse;
pub mod touch;
pub mod gamepad;
pub mod input_map;
pub mod platform;
#[cfg(feature = "windowed")]
pub mod winit_bridge;

pub use keyboard::{KeyCode, KeyState, KeyboardState};
pub use mouse::{MouseButton, MouseState};
pub use touch::{Touch, TouchPhase, TouchState};
pub use gamepad::{GamepadAxis, GamepadButton, GamepadState};
pub use input_map::{
    ActionMap, ActionState, AxisBinding, InputBinding, InputPreset, MouseAxisKind,
};
pub use platform::{
    InputEvent, InputSystem, PlatformInput, PlatformKind, detect_platform,
    default_bindings_for_platform,
};
#[cfg(feature = "windowed")]
pub use winit_bridge::process_window_event;
