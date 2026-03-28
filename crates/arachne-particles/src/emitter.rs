//! Particle emitter: controls how and where particles are spawned.

use arachne_math::{Color, Rng, Vec2};

use crate::particle::{Particle, ParticlePool};

/// Shape from which particles are emitted.
#[derive(Clone, Debug)]
pub enum EmissionShape {
    /// Single point (emitter position).
    Point,
    /// Random point inside a circle of given radius.
    Circle { radius: f32 },
    /// Random point inside a rectangle with given half-extents.
    Rect { half_extents: Vec2 },
    /// Cone along a direction with given half-angle (radians).
    Cone { angle: f32 },
}

impl Default for EmissionShape {
    fn default() -> Self {
        Self::Point
    }
}

/// Configuration for a particle emitter.
#[derive(Clone, Debug)]
pub struct ParticleEmitter {
    /// World-space position of the emitter.
    pub position: Vec2,
    /// Continuous spawn rate (particles per second).
    pub spawn_rate: f32,
    /// Number of particles to emit in a burst (0 = no burst).
    pub burst_count: u32,
    /// Emission shape.
    pub shape: EmissionShape,
    /// Base emission direction (unit vector).
    pub direction: Vec2,
    /// Speed range [min, max] for initial velocity magnitude.
    pub speed_range: (f32, f32),
    /// Spread angle (radians) applied to direction. Particles are emitted within
    /// `direction ± spread_angle`.
    pub spread_angle: f32,
    /// Lifetime range [min, max] in seconds.
    pub lifetime_range: (f32, f32),
    /// Initial color.
    pub color: Color,
    /// Initial size.
    pub size: f32,

    /// Internal accumulator for fractional particle spawning.
    accumulator: f32,
}

impl Default for ParticleEmitter {
    fn default() -> Self {
        Self {
            position: Vec2::ZERO,
            spawn_rate: 100.0,
            burst_count: 0,
            shape: EmissionShape::Point,
            direction: Vec2::Y,
            speed_range: (50.0, 100.0),
            spread_angle: 0.0,
            lifetime_range: (1.0, 2.0),
            color: Color::WHITE,
            size: 1.0,
            accumulator: 0.0,
        }
    }
}

impl ParticleEmitter {
    /// Creates a new emitter with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Resets the fractional spawn accumulator.
    pub fn reset_accumulator(&mut self) {
        self.accumulator = 0.0;
    }

    /// Emits particles based on elapsed `dt` (seconds) into the pool.
    /// Returns the number of particles actually emitted.
    pub fn emit(&mut self, dt: f32, pool: &mut ParticlePool, rng: &mut Rng) -> u32 {
        let mut count = 0u32;

        // Continuous emission via accumulator
        self.accumulator += self.spawn_rate * dt;
        let to_spawn = self.accumulator as u32;
        self.accumulator -= to_spawn as f32;

        for _ in 0..to_spawn {
            if self.spawn_one(pool, rng) {
                count += 1;
            }
        }

        count
    }

    /// Emits a burst of `burst_count` particles immediately.
    /// Returns the number of particles actually emitted.
    pub fn burst(&self, pool: &mut ParticlePool, rng: &mut Rng) -> u32 {
        let mut count = 0u32;
        for _ in 0..self.burst_count {
            if self.spawn_one(pool, rng) {
                count += 1;
            }
        }
        count
    }

    /// Spawns a single particle into the pool. Returns true if successful.
    fn spawn_one(&self, pool: &mut ParticlePool, rng: &mut Rng) -> bool {
        let offset = self.sample_shape(rng);
        let velocity = self.sample_velocity(rng);
        let lifetime = rng.next_range_f32(self.lifetime_range.0, self.lifetime_range.1);

        let particle = Particle {
            position: self.position + offset,
            velocity,
            age: 0.0,
            lifetime,
            color: self.color,
            size: self.size,
            rotation: 0.0,
        };

        pool.spawn(particle).is_some()
    }

    /// Samples a position offset from the emission shape.
    fn sample_shape(&self, rng: &mut Rng) -> Vec2 {
        match &self.shape {
            EmissionShape::Point => Vec2::ZERO,
            EmissionShape::Circle { radius } => {
                let v = rng.next_vec2_in_circle();
                v * *radius
            }
            EmissionShape::Rect { half_extents } => {
                let x = rng.next_range_f32(-half_extents.x, half_extents.x);
                let y = rng.next_range_f32(-half_extents.y, half_extents.y);
                Vec2::new(x, y)
            }
            EmissionShape::Cone { angle: _ } => {
                // For cone shape, position is at emitter; direction is handled
                // by sample_velocity spread.
                Vec2::ZERO
            }
        }
    }

    /// Samples an initial velocity based on direction, speed range, and spread.
    fn sample_velocity(&self, rng: &mut Rng) -> Vec2 {
        let speed = rng.next_range_f32(self.speed_range.0, self.speed_range.1);

        let effective_spread = match &self.shape {
            EmissionShape::Cone { angle } => *angle,
            _ => self.spread_angle,
        };

        let angle_offset = if effective_spread > 0.0 {
            rng.next_range_f32(-effective_spread, effective_spread)
        } else {
            0.0
        };

        let dir = self.direction.normalize().rotate(angle_offset);
        dir * speed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_rate_100_per_sec() {
        let mut emitter = ParticleEmitter {
            spawn_rate: 100.0,
            lifetime_range: (5.0, 5.0), // Long lifetime so none expire
            ..Default::default()
        };
        let mut pool = ParticlePool::new(200);
        let mut rng = Rng::seed(42);

        // Simulate 1 second in 100 steps of 10ms
        let mut total = 0u32;
        for _ in 0..100 {
            total += emitter.emit(0.01, &mut pool, &mut rng);
        }

        // Should be ~100 particles (±5)
        assert!(
            (total as i32 - 100).unsigned_abs() <= 5,
            "expected ~100 particles, got {total}"
        );
        assert!(
            (pool.alive_count() as i32 - 100).unsigned_abs() <= 5,
            "expected ~100 alive, got {}",
            pool.alive_count()
        );
    }

    #[test]
    fn burst_emits_exact_count() {
        let emitter = ParticleEmitter {
            burst_count: 50,
            lifetime_range: (5.0, 5.0),
            ..Default::default()
        };
        let mut pool = ParticlePool::new(100);
        let mut rng = Rng::seed(42);

        let count = emitter.burst(&mut pool, &mut rng);
        assert_eq!(count, 50);
        assert_eq!(pool.alive_count(), 50);
    }

    #[test]
    fn burst_capped_by_pool_capacity() {
        let emitter = ParticleEmitter {
            burst_count: 100,
            lifetime_range: (5.0, 5.0),
            ..Default::default()
        };
        let mut pool = ParticlePool::new(30);
        let mut rng = Rng::seed(42);

        let count = emitter.burst(&mut pool, &mut rng);
        assert_eq!(count, 30);
        assert_eq!(pool.alive_count(), 30);
    }

    #[test]
    fn circle_shape_within_radius() {
        let emitter = ParticleEmitter {
            shape: EmissionShape::Circle { radius: 10.0 },
            spawn_rate: 0.0,
            burst_count: 100,
            speed_range: (0.0, 0.0),
            lifetime_range: (5.0, 5.0),
            ..Default::default()
        };
        let mut pool = ParticlePool::new(200);
        let mut rng = Rng::seed(42);

        emitter.burst(&mut pool, &mut rng);

        for idx in pool.alive_indices() {
            let dist = pool.get(idx).position.length();
            assert!(
                dist <= 10.0 + 1e-3,
                "particle at distance {dist} exceeds radius 10"
            );
        }
    }

    #[test]
    fn rect_shape_within_half_extents() {
        let emitter = ParticleEmitter {
            shape: EmissionShape::Rect {
                half_extents: Vec2::new(5.0, 3.0),
            },
            spawn_rate: 0.0,
            burst_count: 100,
            speed_range: (0.0, 0.0),
            lifetime_range: (5.0, 5.0),
            ..Default::default()
        };
        let mut pool = ParticlePool::new(200);
        let mut rng = Rng::seed(42);

        emitter.burst(&mut pool, &mut rng);

        for idx in pool.alive_indices() {
            let p = pool.get(idx).position;
            assert!(
                p.x.abs() <= 5.0 + 1e-3 && p.y.abs() <= 3.0 + 1e-3,
                "particle at ({}, {}) exceeds rect bounds",
                p.x,
                p.y
            );
        }
    }

    #[test]
    fn accumulator_handles_fractional_spawns() {
        let mut emitter = ParticleEmitter {
            spawn_rate: 3.0, // 3 particles per second
            lifetime_range: (10.0, 10.0),
            ..Default::default()
        };
        let mut pool = ParticlePool::new(100);
        let mut rng = Rng::seed(42);

        // 10 steps of 0.1s each = 1.0s total -> expect 3 particles
        let mut total = 0u32;
        for _ in 0..10 {
            total += emitter.emit(0.1, &mut pool, &mut rng);
        }
        assert_eq!(total, 3, "expected 3 particles for 3/sec over 1s, got {total}");
    }
}
