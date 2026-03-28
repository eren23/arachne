//! 3D product configurator demo.
//!
//! Displays a 3D mesh (unit cube) with an orbit camera. A UI panel offers
//! color swatches and material presets (matte, glossy, metallic). Clicking a
//! swatch changes the material. Mouse drag orbits the camera.
//!
//! Demonstrates: 3D renderer, PBR materials, UI, camera control.

use arachne_app::{
    App, Camera, DefaultPlugins, HeadlessRunner, Transform, Vec3, Color,
    Res, ResMut, Time,
};
use arachne_render::{Camera3d, MeshVertex, PbrMaterial, Albedo};
use arachne_input::{InputSystem, MouseButton};

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

/// Holds the configurator state: selected color, material preset, camera orbit.
struct ConfiguratorState {
    /// Current albedo color.
    color: Color,
    /// Current material preset.
    preset: MaterialPreset,
    /// Orbit camera angles (in radians).
    orbit_yaw: f32,
    orbit_pitch: f32,
    /// Orbit camera distance from target.
    orbit_distance: f32,
    /// The active PBR material (recomputed when color/preset changes).
    material: PbrMaterial,
    /// 3D camera.
    camera: Camera3d,
    /// Available color swatches.
    swatches: Vec<(String, Color)>,
    /// Whether mouse is dragging for orbit.
    dragging: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum MaterialPreset {
    Matte,
    Glossy,
    Metallic,
}

impl ConfiguratorState {
    fn new() -> Self {
        let initial_color = Color::rgb(0.8, 0.2, 0.2);
        let mut state = Self {
            color: initial_color,
            preset: MaterialPreset::Glossy,
            orbit_yaw: 0.5,
            orbit_pitch: 0.3,
            orbit_distance: 5.0,
            material: PbrMaterial::default(),
            camera: Camera3d::new(16.0 / 9.0),
            swatches: vec![
                ("Red".into(), Color::rgb(0.8, 0.2, 0.2)),
                ("Blue".into(), Color::rgb(0.2, 0.3, 0.9)),
                ("Green".into(), Color::rgb(0.2, 0.8, 0.3)),
                ("Gold".into(), Color::rgb(0.9, 0.75, 0.3)),
                ("Silver".into(), Color::rgb(0.75, 0.75, 0.78)),
                ("White".into(), Color::WHITE),
            ],
            dragging: false,
        };
        state.rebuild_material();
        state.update_camera();
        state
    }

    /// Rebuild the PBR material from current color + preset.
    fn rebuild_material(&mut self) {
        let (metallic, roughness) = match self.preset {
            MaterialPreset::Matte => (0.0, 0.9),
            MaterialPreset::Glossy => (0.0, 0.15),
            MaterialPreset::Metallic => (1.0, 0.25),
        };
        self.material = PbrMaterial::new(
            Albedo::Color(self.color),
            metallic,
            roughness,
        );
    }

    /// Update the Camera3d position from orbit angles.
    fn update_camera(&mut self) {
        let x = self.orbit_distance * self.orbit_yaw.cos() * self.orbit_pitch.cos();
        let y = self.orbit_distance * self.orbit_pitch.sin();
        let z = self.orbit_distance * self.orbit_yaw.sin() * self.orbit_pitch.cos();
        self.camera.position = Vec3::new(x, y, z);
        self.camera.target = Vec3::ZERO;
        self.camera.up = Vec3::Y;
    }
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// Handle orbit camera input: mouse drag rotates the view.
fn orbit_camera(input: Res<InputSystem>, mut state: ResMut<ConfiguratorState>) {
    // Begin dragging on left mouse press.
    if input.mouse.just_pressed(MouseButton::Left) {
        state.dragging = true;
    }
    if input.mouse.just_released(MouseButton::Left) {
        state.dragging = false;
    }

    if state.dragging {
        let delta = input.mouse.delta();
        let sensitivity = 0.005;
        state.orbit_yaw += delta.x * sensitivity;
        state.orbit_pitch = (state.orbit_pitch - delta.y * sensitivity)
            .clamp(-1.4, 1.4); // limit pitch to avoid gimbal lock
        state.update_camera();
    }

    // Scroll wheel adjusts distance.
    let scroll = input.mouse.scroll();
    if scroll.y.abs() > 0.01 {
        state.orbit_distance = (state.orbit_distance - scroll.y * 0.5).clamp(2.0, 20.0);
        state.update_camera();
    }
}

/// Draw the UI panel with color swatches and material presets.
fn configurator_ui(mut state: ResMut<ConfiguratorState>) {
    // In a full engine, UIContext would be obtained as a resource and we would draw:
    //
    //   let panel = Panel::new("config_panel", 200.0, 400.0);
    //   let handle = panel.begin(&mut ctx);
    //   Label::new("Color").show(&mut ctx);
    //   for (name, color) in &state.swatches {
    //       if Button::new(name).show(&mut ctx) {
    //           state.color = *color;
    //           state.rebuild_material();
    //       }
    //   }
    //   Label::new("Material").show(&mut ctx);
    //   if Button::new("Matte").show(&mut ctx) { ... }
    //   if Button::new("Glossy").show(&mut ctx) { ... }
    //   if Button::new("Metallic").show(&mut ctx) { ... }
    //   handle.end(&mut ctx);
    //
    // For the headless demo, we cycle through swatches and presets over time.
    // This demonstrates the data flow even without a live renderer.

    // The headless demo will cycle via auto_cycle_demo system below.
}

/// Auto-cycle through configurations for the headless demo.
fn auto_cycle_demo(time: Res<Time>, mut state: ResMut<ConfiguratorState>) {
    let elapsed = time.elapsed_seconds();
    let cycle_period = 1.0; // switch every second

    // Cycle color swatches.
    let swatch_count = state.swatches.len();
    let swatch_idx = ((elapsed / cycle_period) as usize) % swatch_count;
    let new_color = state.swatches[swatch_idx].1;
    if new_color != state.color {
        state.color = new_color;
        state.rebuild_material();
    }

    // Cycle material presets every 3 swatches.
    let preset_idx = ((elapsed / (cycle_period * swatch_count as f32)) as usize) % 3;
    let new_preset = match preset_idx {
        0 => MaterialPreset::Matte,
        1 => MaterialPreset::Glossy,
        _ => MaterialPreset::Metallic,
    };
    if new_preset != state.preset {
        state.preset = new_preset;
        state.rebuild_material();
    }

    // Slowly orbit the camera.
    state.orbit_yaw += time.delta_seconds() * 0.5;
    state.update_camera();
}

/// Report the current configuration (would normally submit to the renderer).
fn report_config(state: Res<ConfiguratorState>) {
    // In a full engine, this would:
    // 1. Update the GPU material uniform buffer with state.material.to_uniform()
    // 2. Set the Camera3d view-projection on the mesh render pass
    // 3. Draw the cube mesh with the current material
    let _uniform = state.material.to_uniform();
    let _vp = state.camera.view_projection();
}

fn main() {
    let mut app = App::new();
    app.add_plugin(DefaultPlugins);

    // Insert configurator state.
    app.insert_resource(ConfiguratorState::new());

    app.add_system(orbit_camera);
    app.add_system(configurator_ui);
    app.add_system(auto_cycle_demo);
    app.add_system(report_config);

    app.build_plugins();

    // Spawn a camera entity.
    app.world.spawn((Camera::new(), Transform::IDENTITY));

    // Generate cube mesh data (for reference; would be uploaded to GPU).
    let (vertices, indices) = MeshVertex::cube();
    println!(
        "Product Configurator: cube mesh has {} vertices, {} indices.",
        vertices.len(),
        indices.len()
    );

    // Run 360 frames at 60fps (6 seconds of auto-cycling).
    app.set_runner(HeadlessRunner::new(360, 1.0 / 60.0));
    app.run();

    let state = app.world.get_resource::<ConfiguratorState>();
    println!(
        "Product Configurator: final color=({:.1},{:.1},{:.1}), preset={:?}, orbit_yaw={:.2}.",
        state.color.r, state.color.g, state.color.b,
        state.preset,
        state.orbit_yaw,
    );
}
