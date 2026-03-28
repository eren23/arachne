//! Minimal Arachne example: create an App, spawn a Camera2d and a colored
//! triangle (represented as three sprites forming a triangular arrangement).
//!
//! Demonstrates: App, DefaultPlugins, Renderer, basic shapes, Transform.
//!
//! Run headless (default):
//!     cargo run --example hello_triangle
//!
//! Run with a window:
//!     cargo run --example hello_triangle --features windowed

use arachne_render::{Sprite, TextureHandle};

// =========================================================================
// Windowed mode (requires `windowed` feature)
// =========================================================================

#[cfg(feature = "windowed")]
fn main() {
    use arachne_app::{
        App, AppExit, Camera, Commands, DefaultPlugins, ScreenTextBuffer,
        Transform, Vec2, Vec3, Color, ResMut, WindowedRunner,
    };
    use arachne_input::{InputSystem, KeyCode};
    use arachne_window::WindowConfig;

    fn setup(mut commands: Commands) {
        // Spawn a 2D camera at the origin.
        commands.spawn((Camera::new(), Transform::IDENTITY));

        // Three vertices of a triangle, each a colored sprite.
        let positions = [
            Vec3::new(0.0, 200.0, 0.0),    // top
            Vec3::new(-200.0, -100.0, 0.0), // bottom-left
            Vec3::new(200.0, -100.0, 0.0),  // bottom-right
        ];
        let colors = [Color::RED, Color::GREEN, Color::BLUE];

        for (pos, color) in positions.iter().zip(colors.iter()) {
            let mut sprite = Sprite::new(TextureHandle(0));
            sprite.color = *color;
            sprite.custom_size = Some(arachne_math::Vec2::new(80.0, 80.0));
            commands.spawn((sprite, Transform::from_position(*pos)));
        }
    }

    /// Each frame, push text draw requests into ScreenTextBuffer.
    fn draw_text_system(mut text_buf: ResMut<ScreenTextBuffer>) {
        // Title text centered near top of screen (world coordinates).
        text_buf.draw(
            "Hello Arachne!",
            Vec2::new(-100.0, 260.0),
            24.0,
            Color::WHITE,
        );

        // Subtitle text below
        text_buf.draw(
            "Text rendering is alive!",
            Vec2::new(-170.0, -220.0),
            24.0,
            Color::new(0.3, 1.0, 0.5, 1.0),
        );

        // Instruction text
        text_buf.draw(
            "Press ESC to exit",
            Vec2::new(-120.0, -260.0),
            16.0,
            Color::new(0.7, 0.7, 0.7, 1.0),
        );
    }

    fn escape_to_exit(input: Res<InputSystem>, mut commands: Commands) {
        if input.keyboard.just_pressed(KeyCode::Escape) {
            commands.insert_resource(AppExit);
        }
    }

    use arachne_app::Res;

    let config = WindowConfig::default()
        .with_title("Hello Triangle")
        .with_size(800, 600);

    let mut app = App::new();
    app.add_plugin(DefaultPlugins);
    app.set_runner(WindowedRunner::new(config));
    app.add_startup_system(setup);
    app.add_system(escape_to_exit);
    app.add_system(draw_text_system);
    app.run();
}

// =========================================================================
// Headless mode (default, no windowed feature)
// =========================================================================

#[cfg(not(feature = "windowed"))]
fn main() {
    use arachne_app::{
        App, Camera, DefaultPlugins, HeadlessRunner, Transform, Vec3, Color,
    };

    /// Startup system: spawn the camera and three colored sprites in a triangle.
    fn setup(world: &mut arachne_ecs::World) {
        // Spawn a 2D camera at the origin.
        world.spawn((Camera::new(), Transform::IDENTITY));

        // Three vertices of a triangle, each a colored sprite.
        let positions = [
            Vec3::new(0.0, 1.0, 0.0),   // top
            Vec3::new(-0.87, -0.5, 0.0), // bottom-left
            Vec3::new(0.87, -0.5, 0.0),  // bottom-right
        ];
        let colors = [Color::RED, Color::GREEN, Color::BLUE];

        for (pos, color) in positions.iter().zip(colors.iter()) {
            let mut sprite = Sprite::new(TextureHandle(0));
            sprite.color = *color;
            world.spawn((sprite, Transform::from_position(*pos)));
        }
    }

    let mut app = App::new();
    app.add_plugin(DefaultPlugins);

    // Use a headless runner for demonstration (1 frame at 60fps).
    // In a real application, use NativeRunner for a window loop.
    app.set_runner(HeadlessRunner::new(1, 1.0 / 60.0));
    app.build_plugins();

    // Run the setup directly on the world before entering the loop.
    setup(&mut app.world);

    app.run();

    // Verify: 3 sprites + 1 camera = 4 entities total.
    let count = app.world.entity_count();
    println!("Hello Triangle: spawned {} entities, frame complete.", count);
}
