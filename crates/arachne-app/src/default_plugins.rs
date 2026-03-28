//! DefaultPlugins: registers core plugins in the correct stage order.

use crate::plugin::Plugin;
use crate::App;
use arachne_ecs::Stage;

// ---------------------------------------------------------------------------
// Core plugins (always included)
// ---------------------------------------------------------------------------

/// Registers the [`Time`](crate::time::Time) resource and core frame systems.
struct TimePlugin;

impl Plugin for TimePlugin {
    fn build(&self, app: &mut App) {
        // Time resource is already inserted by App::new(), but ensure it exists.
        if !app.world.has_resource::<crate::time::Time>() {
            app.world.insert_resource(crate::time::Time::new());
        }
    }

    fn name(&self) -> &str {
        "TimePlugin"
    }
}

/// Registers the input system and its PreUpdate frame-start system.
struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        use arachne_input::InputSystem;

        if !app.world.has_resource::<InputSystem>() {
            app.world.insert_resource(InputSystem::new());
        }

        // begin_frame() transitions JustPressed → Held, etc.
        app.schedule
            .add_system(Stage::PreUpdate, crate::systems::input_update_system);
    }

    fn name(&self) -> &str {
        "InputPlugin"
    }
}

/// Registers the asset server polling system.
struct AssetPlugin;

impl Plugin for AssetPlugin {
    fn build(&self, app: &mut App) {
        use crate::systems::AssetServerResource;
        use arachne_asset::{AssetServer, MemoryIo};

        if !app.world.has_resource::<AssetServerResource>() {
            // Default to a MemoryIo backend with a 64 MB cache.
            app.world.insert_resource(AssetServerResource(
                AssetServer::new(MemoryIo::new(), 64 * 1024 * 1024),
            ));
        }

        app.schedule
            .add_system(Stage::PreUpdate, crate::systems::asset_poll_system);
    }

    fn name(&self) -> &str {
        "AssetPlugin"
    }
}

/// Registers transform propagation in PostUpdate.
struct TransformPlugin;

impl Plugin for TransformPlugin {
    fn build(&self, app: &mut App) {
        app.schedule.add_system(
            Stage::PostUpdate,
            crate::systems::transform_propagation_system,
        );
    }

    fn name(&self) -> &str {
        "TransformPlugin"
    }
}

/// Registers sprite draw-call counting and text rendering in the Render stage.
struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        use crate::components::DrawCallCount;
        use crate::systems::ScreenTextBuffer;

        if !app.world.has_resource::<DrawCallCount>() {
            app.world.insert_resource(DrawCallCount::default());
        }

        if !app.world.has_resource::<ScreenTextBuffer>() {
            app.world.insert_resource(ScreenTextBuffer::default());
        }

        app.schedule
            .add_system(Stage::Render, crate::systems::tilemap_render_system);
        app.schedule
            .add_system(Stage::Render, crate::systems::sprite_render_system);
        app.schedule
            .add_system(Stage::Render, crate::systems::text_render_system);
    }

    fn name(&self) -> &str {
        "RenderPlugin"
    }
}

/// Registers camera update in PostUpdate.
struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        use arachne_render::Camera2d;

        if !app.world.has_resource::<Camera2d>() {
            app.world.insert_resource(Camera2d::new(800.0, 600.0));
        }

        app.schedule
            .add_system(Stage::PostUpdate, crate::systems::camera_update_system);
    }

    fn name(&self) -> &str {
        "CameraPlugin"
    }
}

// ---------------------------------------------------------------------------
// DefaultPlugins
// ---------------------------------------------------------------------------

/// Registers core plugins in the correct stage-pipeline order:
///
/// 1. **Time** – global time resource
/// 2. **Input** → PreUpdate: `InputSystem::begin_frame()`
/// 3. **Asset** → PreUpdate: `AssetServer::poll()`
/// 4. **Transform** → PostUpdate: propagate transforms
/// 5. **Camera** → PostUpdate: update camera from entity
/// 6. **Render** → Render: sprite draw-call counting
/// 7. **Diagnostic** → PreUpdate: frame-time recording
pub struct DefaultPlugins;

impl Plugin for DefaultPlugins {
    fn build(&self, app: &mut App) {
        app.add_plugin(TimePlugin);
        app.add_plugin(InputPlugin);
        app.add_plugin(AssetPlugin);
        app.add_plugin(TransformPlugin);
        app.add_plugin(CameraPlugin);
        app.add_plugin(RenderPlugin);
        app.add_plugin(crate::diagnostic::DiagnosticPlugin);
    }

    fn name(&self) -> &str {
        "DefaultPlugins"
    }
}
