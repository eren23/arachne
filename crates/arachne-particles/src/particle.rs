//! Core particle data and pre-allocated pool.

use arachne_math::{Color, Vec2};

/// A single particle with position, velocity, age, lifetime, color, size, and rotation.
#[derive(Clone, Copy, Debug)]
pub struct Particle {
    pub position: Vec2,
    pub velocity: Vec2,
    pub age: f32,
    pub lifetime: f32,
    pub color: Color,
    pub size: f32,
    pub rotation: f32,
}

impl Default for Particle {
    fn default() -> Self {
        Self {
            position: Vec2::ZERO,
            velocity: Vec2::ZERO,
            age: 0.0,
            lifetime: 1.0,
            color: Color::WHITE,
            size: 1.0,
            rotation: 0.0,
        }
    }
}

impl Particle {
    /// Returns the normalized age `[0, 1]` clamped.
    #[inline]
    pub fn t(&self) -> f32 {
        if self.lifetime <= 0.0 {
            1.0
        } else {
            (self.age / self.lifetime).min(1.0)
        }
    }

    /// Returns true if the particle has expired.
    #[inline]
    pub fn is_dead(&self) -> bool {
        self.age >= self.lifetime
    }
}

/// GPU-compatible particle data for storage buffers.
///
/// Field order matches WGSL struct layout: `vec4` first (align 16), then `vec2`
/// (align 8), then `f32` (align 4). This avoids internal padding so the Rust
/// `repr(C)` layout is byte-identical to the WGSL layout.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuParticle {
    pub color: [f32; 4],
    pub position: [f32; 2],
    pub velocity: [f32; 2],
    pub age: f32,
    pub lifetime: f32,
    pub size: f32,
    pub rotation: f32,
}

impl Default for GpuParticle {
    fn default() -> Self {
        Self {
            color: [1.0, 1.0, 1.0, 1.0],
            position: [0.0; 2],
            velocity: [0.0; 2],
            age: 0.0,
            lifetime: 0.0,
            size: 1.0,
            rotation: 0.0,
        }
    }
}

impl From<&Particle> for GpuParticle {
    fn from(p: &Particle) -> Self {
        Self {
            color: p.color.to_array(),
            position: [p.position.x, p.position.y],
            velocity: [p.velocity.x, p.velocity.y],
            age: p.age,
            lifetime: p.lifetime,
            size: p.size,
            rotation: p.rotation,
        }
    }
}

impl From<&GpuParticle> for Particle {
    fn from(g: &GpuParticle) -> Self {
        Self {
            position: Vec2::new(g.position[0], g.position[1]),
            velocity: Vec2::new(g.velocity[0], g.velocity[1]),
            age: g.age,
            lifetime: g.lifetime,
            color: Color::from_array(g.color),
            size: g.size,
            rotation: g.rotation,
        }
    }
}

/// Pre-allocated particle pool with free-list recycling.
///
/// No runtime allocation during steady state: particles are recycled from a
/// free list into pre-allocated slots.
pub struct ParticlePool {
    /// All particle slots (capacity fixed at creation).
    particles: Vec<Particle>,
    /// Bitflag: true if the slot is alive.
    alive: Vec<bool>,
    /// Indices of free (dead) slots available for reuse.
    free_list: Vec<usize>,
    /// Number of currently alive particles.
    alive_count: usize,
}

impl ParticlePool {
    /// Creates a new pool with the given maximum capacity.
    pub fn new(capacity: usize) -> Self {
        let particles = vec![Particle::default(); capacity];
        let alive = vec![false; capacity];
        let free_list: Vec<usize> = (0..capacity).rev().collect();
        Self {
            particles,
            alive,
            free_list,
            alive_count: 0,
        }
    }

    /// Returns the maximum capacity of the pool.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.particles.len()
    }

    /// Returns the number of alive particles.
    #[inline]
    pub fn alive_count(&self) -> usize {
        self.alive_count
    }

    /// Spawns a particle using a free slot. Returns the slot index, or `None`
    /// if the pool is full.
    pub fn spawn(&mut self, particle: Particle) -> Option<usize> {
        let idx = self.free_list.pop()?;
        self.particles[idx] = particle;
        self.alive[idx] = true;
        self.alive_count += 1;
        Some(idx)
    }

    /// Kills the particle at the given index, returning it to the free list.
    pub fn kill(&mut self, index: usize) {
        if self.alive[index] {
            self.alive[index] = false;
            self.free_list.push(index);
            self.alive_count -= 1;
        }
    }

    /// Returns a reference to the particle at the given index.
    #[inline]
    pub fn get(&self, index: usize) -> &Particle {
        &self.particles[index]
    }

    /// Returns a mutable reference to the particle at the given index.
    #[inline]
    pub fn get_mut(&mut self, index: usize) -> &mut Particle {
        &mut self.particles[index]
    }

    /// Returns true if the slot at `index` is alive.
    #[inline]
    pub fn is_alive(&self, index: usize) -> bool {
        self.alive[index]
    }

    /// Returns the raw particles slice.
    #[inline]
    pub fn particles(&self) -> &[Particle] {
        &self.particles
    }

    /// Returns the raw particles mutable slice.
    #[inline]
    pub fn particles_mut(&mut self) -> &mut [Particle] {
        &mut self.particles
    }

    /// Returns the alive flags slice.
    #[inline]
    pub fn alive_flags(&self) -> &[bool] {
        &self.alive
    }

    /// Iterates over alive particle indices.
    pub fn alive_indices(&self) -> impl Iterator<Item = usize> + '_ {
        self.alive
            .iter()
            .enumerate()
            .filter_map(|(i, &a)| if a { Some(i) } else { None })
    }

    /// Kill all expired particles (age >= lifetime) and return them to the free list.
    pub fn reap_dead(&mut self) {
        for i in 0..self.particles.len() {
            if self.alive[i] && self.particles[i].is_dead() {
                self.alive[i] = false;
                self.free_list.push(i);
                self.alive_count -= 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pool_spawn_and_kill() {
        let mut pool = ParticlePool::new(100);
        assert_eq!(pool.alive_count(), 0);
        assert_eq!(pool.capacity(), 100);

        // Spawn 50
        for _ in 0..50 {
            pool.spawn(Particle::default()).unwrap();
        }
        assert_eq!(pool.alive_count(), 50);

        // Kill first 25
        let indices: Vec<usize> = pool.alive_indices().take(25).collect();
        for idx in indices {
            pool.kill(idx);
        }
        assert_eq!(pool.alive_count(), 25);
    }

    #[test]
    fn pool_reuse_slots() {
        let mut pool = ParticlePool::new(10);

        // Fill pool
        let mut indices = Vec::new();
        for _ in 0..10 {
            indices.push(pool.spawn(Particle::default()).unwrap());
        }
        assert_eq!(pool.alive_count(), 10);

        // Pool is full
        assert!(pool.spawn(Particle::default()).is_none());

        // Kill 5
        for &idx in &indices[0..5] {
            pool.kill(idx);
        }
        assert_eq!(pool.alive_count(), 5);

        // Spawn 5 more -> reuses killed slots
        for _ in 0..5 {
            pool.spawn(Particle::default()).unwrap();
        }
        assert_eq!(pool.alive_count(), 10);
    }

    #[test]
    fn pool_spawn_1000_kill_500_spawn_300() {
        let mut pool = ParticlePool::new(2000);

        // Spawn 1000
        let mut indices = Vec::new();
        for _ in 0..1000 {
            indices.push(pool.spawn(Particle::default()).unwrap());
        }
        assert_eq!(pool.alive_count(), 1000);

        // Kill 500
        for &idx in &indices[0..500] {
            pool.kill(idx);
        }
        assert_eq!(pool.alive_count(), 500);

        // Spawn 300 -> reuses killed slots
        for _ in 0..300 {
            pool.spawn(Particle::default()).unwrap();
        }
        assert_eq!(pool.alive_count(), 800);
    }

    #[test]
    fn particle_t_and_is_dead() {
        let mut p = Particle::default();
        p.lifetime = 2.0;
        p.age = 1.0;
        assert!((p.t() - 0.5).abs() < 1e-6);
        assert!(!p.is_dead());

        p.age = 2.0;
        assert!((p.t() - 1.0).abs() < 1e-6);
        assert!(p.is_dead());
    }

    #[test]
    fn reap_dead_removes_expired() {
        let mut pool = ParticlePool::new(100);

        // Spawn 10 particles with lifetime=1.0
        for _ in 0..10 {
            let mut p = Particle::default();
            p.lifetime = 1.0;
            p.age = 0.0;
            pool.spawn(p);
        }
        assert_eq!(pool.alive_count(), 10);

        // Age 5 of them past lifetime
        let indices: Vec<usize> = pool.alive_indices().take(5).collect();
        for &idx in &indices {
            pool.get_mut(idx).age = 2.0;
        }

        pool.reap_dead();
        assert_eq!(pool.alive_count(), 5);
    }
}
