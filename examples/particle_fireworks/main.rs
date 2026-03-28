//! Particle fireworks demo.
//!
//! Click to spawn firework emitters. Particles burst outward, fade color over
//! their lifetime (bright yellow -> dim red -> transparent), shrink and die.
//! Multiple simultaneous fireworks are supported.
//!
//! Demonstrates: particles, input, color-over-life, size-over-life, burst emission.

use arachne_app::{
    App, Camera, DefaultPlugins, HeadlessRunner, Transform, Vec2, Vec3, Color,
    Res, ResMut, Time,
};
use arachne_math::Rng;
use arachne_particles::{
    CpuSimulator, ColorOverLifeModule, GravityModule, ModuleList,
    ParticleEmitter, ParticlePool, SizeOverLifeModule,
};

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

/// Holds all active firework instances.
///
/// Contains a `ModuleList` which uses `Box<dyn ParticleModule>`. Since
/// ParticleModule does not require Send+Sync, we must assert safety manually.
/// This is safe because FireworksState is only ever accessed from the
/// single-threaded ECS schedule via `ResMut`.
struct FireworksState {
    /// Active emitters (position + time-to-live).
    emitters: Vec<FireworkInstance>,
    /// Shared particle pool for all fireworks.
    pool: ParticlePool,
    /// Particle behaviour modules.
    modules: ModuleList,
    /// CPU particle simulator.
    simulator: CpuSimulator,
    /// Deterministic RNG.
    rng: Rng,
}

// SAFETY: FireworksState is only accessed via &mut through ResMut in a
// single-threaded schedule. The trait objects inside ModuleList are never
// shared across threads.
unsafe impl Send for FireworksState {}
unsafe impl Sync for FireworksState {}

struct FireworkInstance {
    emitter: ParticleEmitter,
    /// Remaining time before this emitter stops spawning (seconds).
    ttl: f32,
    /// Whether the initial burst has been emitted.
    burst_done: bool,
}

impl FireworksState {
    fn new() -> Self {
        // Configure particle modules.
        let mut modules = ModuleList::new();
        modules.add(GravityModule::new(Vec2::new(0.0, -9.81)));
        modules.add(ColorOverLifeModule::new(
            Color::new(1.0, 0.9, 0.2, 1.0), // bright yellow
            Color::new(0.8, 0.1, 0.0, 0.0),  // dim red, fully transparent
        ));
        modules.add(SizeOverLifeModule::linear(2.0, 0.0)); // shrink to nothing

        Self {
            emitters: Vec::new(),
            pool: ParticlePool::new(10_000), // up to 10k particles
            modules,
            simulator: CpuSimulator::new(),
            rng: Rng::seed(7777),
        }
    }

    /// Spawn a new firework at the given world position.
    fn spawn_firework(&mut self, position: Vec2) {
        let mut emitter = ParticleEmitter::new();
        emitter.position = position;
        emitter.burst_count = 200;
        emitter.spawn_rate = 0.0; // only burst, no continuous emission
        emitter.speed_range = (20.0, 60.0);
        emitter.spread_angle = std::f32::consts::PI; // full 360-degree spread
        emitter.direction = Vec2::Y;
        emitter.lifetime_range = (0.5, 1.5);
        emitter.color = Color::new(1.0, 0.9, 0.2, 1.0);
        emitter.size = 2.0;

        self.emitters.push(FireworkInstance {
            emitter,
            ttl: 2.0, // emitter lives for 2 seconds
            burst_done: false,
        });
    }
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// Emit particles from all active firework emitters.
fn emit_fireworks(mut state: ResMut<FireworksState>, time: Res<Time>) {
    let dt = time.delta_seconds();

    // Destructure to allow simultaneous borrows of different fields.
    let FireworksState {
        ref mut emitters,
        ref mut pool,
        ref mut rng,
        ..
    } = *state;

    for fw in emitters.iter_mut() {
        if !fw.burst_done {
            fw.emitter.burst(pool, rng);
            fw.burst_done = true;
        }
        fw.ttl -= dt;
    }

    // Remove expired emitters (particles continue to live in the pool).
    emitters.retain(|fw| fw.ttl > 0.0);
}

/// Step the particle simulation forward.
fn simulate_particles(mut state: ResMut<FireworksState>, time: Res<Time>) {
    let dt = time.delta_seconds();
    // Destructure to avoid multiple borrows of state.
    let FireworksState {
        ref mut pool,
        ref modules,
        ref mut simulator,
        ..
    } = *state;
    simulator.step(pool, modules, dt);
}

/// Spawn fireworks at pre-determined positions for the headless demo.
/// In a real app, this would read mouse clicks from InputSystem.
fn auto_spawn_fireworks(mut state: ResMut<FireworksState>, time: Res<Time>) {
    let elapsed = time.elapsed_seconds();

    // Spawn a firework every 0.5 seconds at different positions.
    let interval = 0.5;
    let spawn_index = (elapsed / interval) as u32;

    // Limit to 10 spawns for the demo.
    if spawn_index < 10 {
        // Only spawn if this index hasn't been spawned yet (check by time window).
        let time_in_interval = elapsed - (spawn_index as f32 * interval);
        if time_in_interval < time.delta_seconds() {
            let x = state.rng.next_range_f32(-30.0, 30.0);
            let y = state.rng.next_range_f32(0.0, 20.0);
            state.spawn_firework(Vec2::new(x, y));
        }
    }
}

/// Report particle counts (would normally render particles to screen).
fn report_particles(state: Res<FireworksState>) {
    let alive = state.pool.alive_count();
    if alive > 0 {
        // In a real app, this data feeds into ParticleRenderer for GPU rendering.
        let _ = alive; // suppress unused warning in headless mode
    }
}

fn main() {
    let mut app = App::new();
    app.add_plugin(DefaultPlugins);

    // Insert the fireworks state resource.
    app.insert_resource(FireworksState::new());

    app.add_system(auto_spawn_fireworks);
    app.add_system(emit_fireworks);
    app.add_system(simulate_particles);
    app.add_system(report_particles);

    // Spawn camera.
    app.build_plugins();
    app.world.spawn((Camera::new(), Transform::IDENTITY));

    // Run 300 frames at 60fps (5 seconds of fireworks).
    app.set_runner(HeadlessRunner::new(300, 1.0 / 60.0));
    app.run();

    let state = app.world.get_resource::<FireworksState>();
    println!(
        "Particle Fireworks: {} alive particles, {} active emitters, simulation complete.",
        state.pool.alive_count(),
        state.emitters.len()
    );
}
