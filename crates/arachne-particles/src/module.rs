//! Particle modules: per-particle per-frame behaviour modifiers.

use arachne_math::{Color, Vec2};

use crate::particle::Particle;

/// Trait for particle behaviour modules.
pub trait ParticleModule {
    /// Apply this module to a single particle for the given delta time.
    fn apply(&self, particle: &mut Particle, dt: f32);
}

// ---------------------------------------------------------------------------
// GravityModule
// ---------------------------------------------------------------------------

/// Applies a constant gravitational acceleration to particles.
pub struct GravityModule {
    pub acceleration: Vec2,
}

impl GravityModule {
    pub fn new(acceleration: Vec2) -> Self {
        Self { acceleration }
    }
}

impl ParticleModule for GravityModule {
    #[inline]
    fn apply(&self, particle: &mut Particle, dt: f32) {
        particle.velocity += self.acceleration * dt;
    }
}

// ---------------------------------------------------------------------------
// ColorOverLifeModule
// ---------------------------------------------------------------------------

/// Lerps particle color from `start_color` to `end_color` over its lifetime.
pub struct ColorOverLifeModule {
    pub start_color: Color,
    pub end_color: Color,
}

impl ColorOverLifeModule {
    pub fn new(start_color: Color, end_color: Color) -> Self {
        Self {
            start_color,
            end_color,
        }
    }
}

impl ParticleModule for ColorOverLifeModule {
    #[inline]
    fn apply(&self, particle: &mut Particle, _dt: f32) {
        let t = particle.t();
        particle.color = self.start_color.lerp(self.end_color, t);
    }
}

// ---------------------------------------------------------------------------
// SizeOverLifeModule
// ---------------------------------------------------------------------------

/// Controls particle size over lifetime. Supports linear interpolation or a
/// cubic bezier curve.
pub struct SizeOverLifeModule {
    pub start_size: f32,
    pub end_size: f32,
    /// Optional bezier control points for non-linear size curves.
    /// If `None`, linear interpolation is used.
    pub bezier: Option<(f32, f32)>,
}

impl SizeOverLifeModule {
    pub fn linear(start_size: f32, end_size: f32) -> Self {
        Self {
            start_size,
            end_size,
            bezier: None,
        }
    }

    pub fn bezier(start_size: f32, end_size: f32, cp1: f32, cp2: f32) -> Self {
        Self {
            start_size,
            end_size,
            bezier: Some((cp1, cp2)),
        }
    }
}

impl ParticleModule for SizeOverLifeModule {
    #[inline]
    fn apply(&self, particle: &mut Particle, _dt: f32) {
        let t = particle.t();
        let size = if let Some((cp1, cp2)) = self.bezier {
            // Cubic bezier: P0=start, P1=cp1, P2=cp2, P3=end
            let u = 1.0 - t;
            let u2 = u * u;
            let t2 = t * t;
            u2 * u * self.start_size
                + 3.0 * u2 * t * cp1
                + 3.0 * u * t2 * cp2
                + t2 * t * self.end_size
        } else {
            self.start_size + (self.end_size - self.start_size) * t
        };
        particle.size = size;
    }
}

// ---------------------------------------------------------------------------
// VelocityOverLifeModule
// ---------------------------------------------------------------------------

/// Multiplies particle speed by a curve value over lifetime.
pub struct VelocityOverLifeModule {
    /// Speed multiplier at t=0.
    pub start_multiplier: f32,
    /// Speed multiplier at t=1.
    pub end_multiplier: f32,
}

impl VelocityOverLifeModule {
    pub fn new(start_multiplier: f32, end_multiplier: f32) -> Self {
        Self {
            start_multiplier,
            end_multiplier,
        }
    }
}

impl ParticleModule for VelocityOverLifeModule {
    #[inline]
    fn apply(&self, particle: &mut Particle, _dt: f32) {
        let t = particle.t();
        let multiplier = self.start_multiplier + (self.end_multiplier - self.start_multiplier) * t;
        let speed = particle.velocity.length();
        if speed > 1e-8 {
            let desired_speed = speed * multiplier;
            particle.velocity = particle.velocity * (desired_speed / speed);
        }
    }
}

// ---------------------------------------------------------------------------
// NoiseModule
// ---------------------------------------------------------------------------

/// Applies Perlin-like noise displacement to particles.
///
/// Uses a simplified gradient noise approach for performance.
pub struct NoiseModule {
    pub frequency: f32,
    pub amplitude: f32,
    seed: u32,
}

impl NoiseModule {
    pub fn new(frequency: f32, amplitude: f32) -> Self {
        Self {
            frequency,
            amplitude,
            seed: 0,
        }
    }

    pub fn with_seed(frequency: f32, amplitude: f32, seed: u32) -> Self {
        Self {
            frequency,
            amplitude,
            seed,
        }
    }

    /// Simple hash-based gradient noise in 2D.
    fn noise2d(&self, x: f32, y: f32) -> Vec2 {
        let ix = x.floor() as i32;
        let iy = y.floor() as i32;
        let fx = x - x.floor();
        let fy = y - y.floor();

        // Smoothstep
        let ux = fx * fx * (3.0 - 2.0 * fx);
        let uy = fy * fy * (3.0 - 2.0 * fy);

        let n00 = self.hash_gradient(ix, iy, fx, fy);
        let n10 = self.hash_gradient(ix + 1, iy, fx - 1.0, fy);
        let n01 = self.hash_gradient(ix, iy + 1, fx, fy - 1.0);
        let n11 = self.hash_gradient(ix + 1, iy + 1, fx - 1.0, fy - 1.0);

        let nx0 = n00 + ux * (n10 - n00);
        let nx1 = n01 + ux * (n11 - n01);
        let val_x = nx0 + uy * (nx1 - nx0);

        // Second channel: offset hash
        let m00 = self.hash_gradient(ix + 73, iy + 137, fx, fy);
        let m10 = self.hash_gradient(ix + 74, iy + 137, fx - 1.0, fy);
        let m01 = self.hash_gradient(ix + 73, iy + 138, fx, fy - 1.0);
        let m11 = self.hash_gradient(ix + 74, iy + 138, fx - 1.0, fy - 1.0);

        let mx0 = m00 + ux * (m10 - m00);
        let mx1 = m01 + ux * (m11 - m01);
        let val_y = mx0 + uy * (mx1 - mx0);

        Vec2::new(val_x, val_y)
    }

    #[inline]
    fn hash_gradient(&self, ix: i32, iy: i32, dx: f32, dy: f32) -> f32 {
        let h = self.hash(ix, iy);
        let (gx, gy) = match h & 3 {
            0 => (1.0_f32, 1.0_f32),
            1 => (-1.0, 1.0),
            2 => (1.0, -1.0),
            _ => (-1.0, -1.0),
        };
        gx * dx + gy * dy
    }

    #[inline]
    fn hash(&self, x: i32, y: i32) -> u32 {
        let mut h = (x as u32).wrapping_mul(374761393)
            .wrapping_add((y as u32).wrapping_mul(668265263))
            .wrapping_add(self.seed.wrapping_mul(1013904223));
        h = (h ^ (h >> 13)).wrapping_mul(1274126177);
        h ^ (h >> 16)
    }
}

impl ParticleModule for NoiseModule {
    #[inline]
    fn apply(&self, particle: &mut Particle, dt: f32) {
        let sample_pos = particle.position * self.frequency;
        let noise = self.noise2d(sample_pos.x, sample_pos.y);
        particle.velocity += noise * self.amplitude * dt;
    }
}

// ---------------------------------------------------------------------------
// RotationModule
// ---------------------------------------------------------------------------

/// Applies constant angular velocity to particles.
pub struct RotationModule {
    /// Angular velocity in radians per second.
    pub angular_velocity: f32,
}

impl RotationModule {
    pub fn new(angular_velocity: f32) -> Self {
        Self { angular_velocity }
    }
}

impl ParticleModule for RotationModule {
    #[inline]
    fn apply(&self, particle: &mut Particle, dt: f32) {
        particle.rotation += self.angular_velocity * dt;
    }
}

// ---------------------------------------------------------------------------
// DynModuleList: type-erased module collection
// ---------------------------------------------------------------------------

/// A collection of boxed particle modules.
pub struct ModuleList {
    modules: Vec<Box<dyn ParticleModule>>,
}

impl ModuleList {
    pub fn new() -> Self {
        Self {
            modules: Vec::new(),
        }
    }

    pub fn add<M: ParticleModule + 'static>(&mut self, module: M) {
        self.modules.push(Box::new(module));
    }

    #[inline]
    pub fn apply_all(&self, particle: &mut Particle, dt: f32) {
        for module in &self.modules {
            module.apply(particle, dt);
        }
    }

    pub fn len(&self) -> usize {
        self.modules.len()
    }

    pub fn is_empty(&self) -> bool {
        self.modules.is_empty()
    }
}

impl Default for ModuleList {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arachne_math::Color;

    #[test]
    fn gravity_accelerates_particle() {
        let module = GravityModule::new(Vec2::new(0.0, -9.81));
        let mut p = Particle::default();
        p.velocity = Vec2::ZERO;

        module.apply(&mut p, 1.0);
        assert!((p.velocity.y - (-9.81)).abs() < 1e-5);
    }

    #[test]
    fn gravity_particle_falls_half_g_t_squared() {
        let g = Vec2::new(0.0, -9.81);
        let module = GravityModule::new(g);
        let mut p = Particle::default();
        p.position = Vec2::ZERO;
        p.velocity = Vec2::ZERO;
        p.lifetime = 10.0;

        let dt = 0.001;
        let steps = 1000; // 1 second
        for _ in 0..steps {
            module.apply(&mut p, dt);
            p.position += p.velocity * dt;
        }

        // Expected: y = 0.5 * (-9.81) * 1^2 = -4.905
        let expected_y = 0.5 * g.y * 1.0;
        assert!(
            (p.position.y - expected_y).abs() < 1e-2,
            "expected y ~ {expected_y}, got {}",
            p.position.y
        );
    }

    #[test]
    fn color_over_life_at_start() {
        let module = ColorOverLifeModule::new(Color::RED, Color::BLUE);
        let mut p = Particle::default();
        p.age = 0.0;
        p.lifetime = 1.0;

        module.apply(&mut p, 0.0);
        assert!((p.color.r - 1.0).abs() < 1e-5);
        assert!(p.color.b.abs() < 1e-5);
    }

    #[test]
    fn color_over_life_at_end() {
        let module = ColorOverLifeModule::new(Color::RED, Color::BLUE);
        let mut p = Particle::default();
        p.age = 1.0;
        p.lifetime = 1.0;

        module.apply(&mut p, 0.0);
        assert!(p.color.r.abs() < 1e-5);
        assert!((p.color.b - 1.0).abs() < 1e-5);
    }

    #[test]
    fn color_over_life_midpoint() {
        let module = ColorOverLifeModule::new(Color::RED, Color::BLUE);
        let mut p = Particle::default();
        p.age = 0.5;
        p.lifetime = 1.0;

        module.apply(&mut p, 0.0);
        // At midpoint: r=0.5, b=0.5
        assert!(
            (p.color.r - 0.5).abs() < 1e-5,
            "r at midpoint: {}",
            p.color.r
        );
        assert!(
            (p.color.b - 0.5).abs() < 1e-5,
            "b at midpoint: {}",
            p.color.b
        );
    }

    #[test]
    fn size_over_life_linear() {
        let module = SizeOverLifeModule::linear(10.0, 0.0);
        let mut p = Particle::default();
        p.lifetime = 1.0;

        p.age = 0.0;
        module.apply(&mut p, 0.0);
        assert!((p.size - 10.0).abs() < 1e-5);

        p.age = 0.5;
        module.apply(&mut p, 0.0);
        assert!((p.size - 5.0).abs() < 1e-5);

        p.age = 1.0;
        module.apply(&mut p, 0.0);
        assert!(p.size.abs() < 1e-5);
    }

    #[test]
    fn size_over_life_bezier() {
        let module = SizeOverLifeModule::bezier(0.0, 10.0, 0.0, 10.0);
        let mut p = Particle::default();
        p.lifetime = 1.0;

        p.age = 0.0;
        module.apply(&mut p, 0.0);
        assert!(p.size.abs() < 1e-5);

        p.age = 1.0;
        module.apply(&mut p, 0.0);
        assert!((p.size - 10.0).abs() < 1e-5);
    }

    #[test]
    fn noise_module_increases_variance() {
        let module = NoiseModule::new(1.0, 50.0);

        // Create many particles at different positions
        let n = 100;
        let mut particles: Vec<Particle> = (0..n)
            .map(|i| {
                let mut p = Particle::default();
                p.position = Vec2::new(i as f32 * 0.1, 0.0);
                p.velocity = Vec2::ZERO;
                p.lifetime = 10.0;
                p
            })
            .collect();

        // Measure initial variance of positions
        let mean_before: Vec2 = particles.iter().map(|p| p.position).fold(Vec2::ZERO, |a, b| a + b) / n as f32;
        let var_before: f32 = particles
            .iter()
            .map(|p| (p.position - mean_before).length_squared())
            .sum::<f32>()
            / n as f32;

        // Apply noise for many steps
        for _ in 0..100 {
            for p in particles.iter_mut() {
                module.apply(p, 0.016);
                p.position += p.velocity * 0.016;
            }
        }

        let mean_after: Vec2 = particles.iter().map(|p| p.position).fold(Vec2::ZERO, |a, b| a + b) / n as f32;
        let var_after: f32 = particles
            .iter()
            .map(|p| (p.position - mean_after).length_squared())
            .sum::<f32>()
            / n as f32;

        assert!(
            var_after > var_before,
            "variance did not increase: before={var_before}, after={var_after}"
        );
    }

    #[test]
    fn rotation_module_rotates() {
        let module = RotationModule::new(core::f32::consts::PI); // 180 deg/sec
        let mut p = Particle::default();
        p.rotation = 0.0;

        module.apply(&mut p, 1.0);
        assert!((p.rotation - core::f32::consts::PI).abs() < 1e-5);
    }

    #[test]
    fn velocity_over_life_slows_down() {
        let module = VelocityOverLifeModule::new(1.0, 0.0);
        let mut p = Particle::default();
        p.velocity = Vec2::new(10.0, 0.0);
        p.lifetime = 1.0;
        p.age = 0.5;

        module.apply(&mut p, 0.0);
        // At midpoint, multiplier = 0.5, so speed = 10 * 0.5 = 5.0
        assert!(
            (p.velocity.length() - 5.0).abs() < 1e-3,
            "speed at midpoint: {}",
            p.velocity.length()
        );
    }

    #[test]
    fn module_list_applies_all() {
        let mut list = ModuleList::new();
        list.add(GravityModule::new(Vec2::new(0.0, -10.0)));
        list.add(RotationModule::new(1.0));

        let mut p = Particle::default();
        p.velocity = Vec2::ZERO;
        p.rotation = 0.0;

        list.apply_all(&mut p, 1.0);
        assert!((p.velocity.y - (-10.0)).abs() < 1e-5);
        assert!((p.rotation - 1.0).abs() < 1e-5);
    }
}
