//! Plugin trait and feature-gated plugin stubs.

use crate::App;

// ---------------------------------------------------------------------------
// Plugin trait
// ---------------------------------------------------------------------------

/// A modular unit of functionality that configures an [`App`].
///
/// Plugins register resources, systems, and events during `build()`.
/// They are applied in the order they are added to the app.
pub trait Plugin: 'static {
    /// Configure the app: insert resources, add systems, register events, etc.
    fn build(&self, app: &mut App);

    /// Human-readable name for debugging / diagnostics.
    fn name(&self) -> &str {
        std::any::type_name::<Self>()
    }
}

// ---------------------------------------------------------------------------
// Feature-gated plugin stubs
// ---------------------------------------------------------------------------

/// Registers the 2D physics subsystem.
///
/// Physics stepping uses the fixed-timestep accumulator built into
/// [`PhysicsWorld::update`]. The step system runs in **PreUpdate** (after
/// input), and transform sync runs in **PostUpdate** (before rendering).
pub struct Physics2dPlugin;

impl Plugin for Physics2dPlugin {
    fn build(&self, app: &mut App) {
        use arachne_ecs::Stage;
        use arachne_physics::world::PhysicsConfig;
        use arachne_physics::PhysicsWorld;

        if !app.world.has_resource::<PhysicsWorld>() {
            app.world
                .insert_resource(PhysicsWorld::new(PhysicsConfig::default()));
        }

        // Step physics (uses its internal fixed-timestep accumulator).
        app.schedule
            .add_system(Stage::PreUpdate, crate::systems::physics_step_system);

        // Sync physics body state → ECS Transform components.
        app.schedule
            .add_system(Stage::PostUpdate, crate::systems::physics_sync_system)
            .before("transform_propagation");
    }

    fn name(&self) -> &str {
        "Physics2dPlugin"
    }
}

/// Registers the audio subsystem.
pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        use crate::systems::AudioMixerResource;
        use arachne_ecs::Stage;

        if !app.world.has_resource::<AudioMixerResource>() {
            app.world
                .insert_resource(AudioMixerResource(arachne_audio::AudioMixer::new(44100)));
        }

        app.schedule
            .add_system(Stage::PostUpdate, crate::systems::audio_update_system);
    }

    fn name(&self) -> &str {
        "AudioPlugin"
    }
}

/// Registers the UI subsystem.
pub struct UIPlugin;

impl Plugin for UIPlugin {
    fn build(&self, _app: &mut App) {
        // UI context is typically created per-frame or managed externally.
        // This is a placeholder for wiring UIContext into the schedule.
    }

    fn name(&self) -> &str {
        "UIPlugin"
    }
}

/// Registers the particle subsystem.
pub struct ParticlePlugin;

impl Plugin for ParticlePlugin {
    fn build(&self, _app: &mut App) {
        // Particle systems are wired via component queries.
        // This is a placeholder for registering particle simulation systems.
    }

    fn name(&self) -> &str {
        "ParticlePlugin"
    }
}

/// Registers animation systems.
pub struct AnimationPlugin;

impl Plugin for AnimationPlugin {
    fn build(&self, _app: &mut App) {
        // Placeholder for animation system registration.
    }

    fn name(&self) -> &str {
        "AnimationPlugin"
    }
}

/// Registers networking systems.
pub struct NetworkPlugin;

impl Plugin for NetworkPlugin {
    fn build(&self, _app: &mut App) {
        // Placeholder for networking system registration.
    }

    fn name(&self) -> &str {
        "NetworkPlugin"
    }
}
