//! The main App struct: builder pattern for assembling world, schedule, plugins, runner.

use arachne_ecs::{IntoSystem, Schedule, Stage, World};

use crate::plugin::Plugin;
use crate::runner::{HeadlessRunner, Runner};
use crate::time::Time;

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

/// The central application container.
///
/// Holds the ECS [`World`], the [`Schedule`], registered plugins, and the
/// runner that drives the main loop.
pub struct App {
    pub world: World,
    pub schedule: Schedule,
    plugins: Vec<Box<dyn Plugin>>,
    runner: Box<dyn Runner>,
    plugins_built: bool,
}

impl App {
    /// Create a new, empty app with a default headless runner.
    pub fn new() -> Self {
        let mut world = World::new();
        world.insert_resource(Time::new());

        Self {
            world,
            schedule: Schedule::new(),
            plugins: Vec::new(),
            runner: Box::new(HeadlessRunner::default()),
            plugins_built: false,
        }
    }

    /// Add a plugin. Plugins are built (in registration order) when `run()` is
    /// called, before the first frame.
    pub fn add_plugin<P: Plugin>(&mut self, plugin: P) -> &mut Self {
        self.plugins.push(Box::new(plugin));
        self
    }

    /// Add a system to the **Update** stage.
    pub fn add_system<M>(&mut self, system: impl IntoSystem<M>) -> &mut Self {
        self.schedule.add_system(Stage::Update, system);
        self
    }

    /// Add a system to the **Startup** stage (runs once before the first frame).
    pub fn add_startup_system<M>(&mut self, system: impl IntoSystem<M>) -> &mut Self {
        self.schedule.add_system(Stage::Startup, system);
        self
    }

    /// Add a system to a specific stage.
    pub fn add_system_to_stage<M>(
        &mut self,
        stage: Stage,
        system: impl IntoSystem<M>,
    ) -> &mut Self {
        self.schedule.add_system(stage, system);
        self
    }

    /// Insert a resource into the world.
    pub fn insert_resource<T: 'static + Send + Sync>(&mut self, resource: T) -> &mut Self {
        self.world.insert_resource(resource);
        self
    }

    /// Set the runner (replaces the default HeadlessRunner).
    pub fn set_runner<R: Runner>(&mut self, runner: R) -> &mut Self {
        self.runner = Box::new(runner);
        self
    }

    /// Build all plugins (in registration order) then run the app.
    ///
    /// This consumes the main loop — for headless runners it returns when
    /// frames are exhausted; for native runners it may never return.
    pub fn run(&mut self) {
        self.build_plugins();

        // Take runner out temporarily to avoid borrow conflict.
        let mut runner = std::mem::replace(
            &mut self.runner,
            Box::new(HeadlessRunner::default()),
        );
        runner.run(&mut self.world, &mut self.schedule);
        self.runner = runner;
    }

    /// Build all pending plugins. Called automatically by `run()` but can be
    /// called manually for testing.
    ///
    /// Handles nested plugin registration: if a plugin's `build()` calls
    /// `add_plugin()`, the new sub-plugins are built in subsequent passes.
    pub fn build_plugins(&mut self) {
        if self.plugins_built {
            return;
        }
        self.plugins_built = true;

        // Use an external queue so plugin.build() can call app.add_plugin()
        // without conflicting with the iteration. Newly added plugins go into
        // self.plugins (which is empty during each round) and are drained in
        // subsequent rounds.
        let mut built: Vec<Box<dyn Plugin>> = Vec::new();
        loop {
            let pending = std::mem::take(&mut self.plugins);
            if pending.is_empty() {
                break;
            }
            for plugin in pending {
                plugin.build(self);
                built.push(plugin);
            }
            // Any sub-plugins added by build() are now in self.plugins,
            // which will be drained in the next loop iteration.
        }
        self.plugins = built;
    }

    /// Get the world (immutable).
    pub fn world(&self) -> &World {
        &self.world
    }

    /// Get the world (mutable).
    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    /// Number of registered plugins.
    pub fn plugin_count(&self) -> usize {
        self.plugins.len()
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
