//! CPU particle simulation loop (WASM fallback).

use crate::module::ModuleList;
use crate::particle::ParticlePool;

/// CPU-side particle simulation.
///
/// Each frame: apply all modules, integrate position, kill expired particles,
/// and sort alive particles by depth for correct alpha blending.
pub struct CpuSimulator {
    /// Sorted indices of alive particles (back-to-front by y for 2D).
    sorted_indices: Vec<usize>,
}

impl CpuSimulator {
    pub fn new() -> Self {
        Self {
            sorted_indices: Vec::new(),
        }
    }

    /// Steps the simulation forward by `dt` seconds.
    ///
    /// 1. Apply all modules to each alive particle.
    /// 2. Integrate position (Euler).
    /// 3. Advance age and kill expired particles.
    /// 4. Sort alive particles by depth (back-to-front).
    pub fn step(&mut self, pool: &mut ParticlePool, modules: &ModuleList, dt: f32) {
        let cap = pool.capacity();

        // Phase 1+2: Apply modules and integrate
        for i in 0..cap {
            if !pool.is_alive(i) {
                continue;
            }

            let p = pool.get_mut(i);

            // Apply modules
            modules.apply_all(p, dt);

            // Euler integration
            p.position += p.velocity * dt;

            // Advance age
            p.age += dt;
        }

        // Phase 3: Kill expired
        pool.reap_dead();

        // Phase 4: Sort alive by y (back-to-front for 2D: lower y = farther)
        self.sort_by_depth(pool);
    }

    /// Sorts alive particle indices by y-position (ascending = back-to-front).
    fn sort_by_depth(&mut self, pool: &ParticlePool) {
        self.sorted_indices.clear();
        self.sorted_indices.extend(pool.alive_indices());

        let particles = pool.particles();
        self.sorted_indices.sort_unstable_by(|&a, &b| {
            particles[a]
                .position
                .y
                .partial_cmp(&particles[b].position.y)
                .unwrap_or(core::cmp::Ordering::Equal)
        });
    }

    /// Returns sorted alive indices (back-to-front).
    pub fn sorted_indices(&self) -> &[usize] {
        &self.sorted_indices
    }
}

impl Default for CpuSimulator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emitter::ParticleEmitter;
    use crate::module::*;
    use crate::particle::Particle;
    use arachne_math::{Rng, Vec2};

    #[test]
    fn step_integrates_position() {
        let mut pool = ParticlePool::new(10);
        let mut p = Particle::default();
        p.velocity = Vec2::new(10.0, 0.0);
        p.lifetime = 5.0;
        pool.spawn(p);

        let modules = ModuleList::new();
        let mut sim = CpuSimulator::new();

        sim.step(&mut pool, &modules, 1.0);

        let idx = pool.alive_indices().next().unwrap();
        assert!(
            (pool.get(idx).position.x - 10.0).abs() < 1e-3,
            "position.x after 1s at vel=10: {}",
            pool.get(idx).position.x
        );
    }

    #[test]
    fn step_kills_expired() {
        let mut pool = ParticlePool::new(10);
        let mut p = Particle::default();
        p.lifetime = 0.5;
        p.age = 0.0;
        pool.spawn(p);

        let modules = ModuleList::new();
        let mut sim = CpuSimulator::new();

        // Step past lifetime
        sim.step(&mut pool, &modules, 1.0);
        assert_eq!(pool.alive_count(), 0);
    }

    #[test]
    fn step_with_gravity() {
        let mut pool = ParticlePool::new(10);
        let mut p = Particle::default();
        p.position = Vec2::ZERO;
        p.velocity = Vec2::ZERO;
        p.lifetime = 10.0;
        pool.spawn(p);

        let mut modules = ModuleList::new();
        modules.add(GravityModule::new(Vec2::new(0.0, -9.81)));

        let mut sim = CpuSimulator::new();

        // Simulate 1 second in small steps
        let dt = 0.001;
        for _ in 0..1000 {
            sim.step(&mut pool, &modules, dt);
        }

        let idx = pool.alive_indices().next().unwrap();
        let expected_y = 0.5 * -9.81 * 1.0;
        assert!(
            (pool.get(idx).position.y - expected_y).abs() < 1e-2,
            "expected y ~ {expected_y}, got {}",
            pool.get(idx).position.y
        );
    }

    #[test]
    fn sorted_indices_back_to_front() {
        let mut pool = ParticlePool::new(10);

        // Spawn particles at different y positions
        for y in [5.0, 1.0, 3.0, 2.0, 4.0] {
            let mut p = Particle::default();
            p.position = Vec2::new(0.0, y);
            p.lifetime = 10.0;
            pool.spawn(p);
        }

        let modules = ModuleList::new();
        let mut sim = CpuSimulator::new();
        sim.step(&mut pool, &modules, 0.0);

        let indices = sim.sorted_indices();
        // Verify sorted by ascending y
        for w in indices.windows(2) {
            let y_a = pool.get(w[0]).position.y;
            let y_b = pool.get(w[1]).position.y;
            assert!(y_a <= y_b, "not sorted: {y_a} > {y_b}");
        }
    }

    #[test]
    fn emitter_and_sim_integration() {
        let mut emitter = ParticleEmitter::new();
        emitter.spawn_rate = 100.0;
        emitter.lifetime_range = (5.0, 5.0);
        emitter.speed_range = (0.0, 0.0);
        let mut pool = ParticlePool::new(500);
        let mut rng = Rng::seed(42);
        let modules = ModuleList::new();
        let mut sim = CpuSimulator::new();

        // Emit for 1 second
        for _ in 0..100 {
            emitter.emit(0.01, &mut pool, &mut rng);
            sim.step(&mut pool, &modules, 0.01);
        }

        // ~100 particles should be alive (long lifetime)
        let alive = pool.alive_count();
        assert!(
            (alive as i32 - 100).unsigned_abs() <= 5,
            "expected ~100 alive, got {alive}"
        );
    }

    #[test]
    fn bench_cpu_10k_particles() {
        let mut pool = ParticlePool::new(10_000);
        let mut rng = Rng::seed(42);

        // Spawn 10K particles
        for _ in 0..10_000 {
            let mut p = Particle::default();
            p.position = Vec2::new(
                rng.next_range_f32(-100.0, 100.0),
                rng.next_range_f32(-100.0, 100.0),
            );
            p.velocity = Vec2::new(
                rng.next_range_f32(-10.0, 10.0),
                rng.next_range_f32(-10.0, 10.0),
            );
            p.lifetime = rng.next_range_f32(1.0, 5.0);
            pool.spawn(p);
        }
        assert_eq!(pool.alive_count(), 10_000);

        let mut modules = ModuleList::new();
        modules.add(GravityModule::new(Vec2::new(0.0, -9.81)));
        modules.add(ColorOverLifeModule::new(
            arachne_math::Color::WHITE,
            arachne_math::Color::TRANSPARENT,
        ));
        modules.add(RotationModule::new(1.0));

        let mut sim = CpuSimulator::new();

        // Warm up
        sim.step(&mut pool, &modules, 0.016);

        // Benchmark
        let start = std::time::Instant::now();
        let frames = 10;
        for _ in 0..frames {
            sim.step(&mut pool, &modules, 0.016);
        }
        let elapsed = start.elapsed();
        let per_frame = elapsed / frames;

        eprintln!(
            "CPU 10K particles: {:.2}ms/frame ({} frames in {:.2}ms)",
            per_frame.as_secs_f64() * 1000.0,
            frames,
            elapsed.as_secs_f64() * 1000.0,
        );

        // Only enforce hard threshold in release mode (debug has no inlining).
        #[cfg(not(debug_assertions))]
        assert!(
            per_frame.as_millis() < 2,
            "CPU 10K particles took {}ms, expected < 2ms",
            per_frame.as_millis()
        );
    }
}
