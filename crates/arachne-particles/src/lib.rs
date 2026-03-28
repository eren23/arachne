pub mod particle;
pub mod emitter;
pub mod module;
pub mod sim_cpu;
pub mod sim_gpu;
pub mod render;

pub use particle::{Particle, ParticlePool, GpuParticle};
pub use emitter::{ParticleEmitter, EmissionShape};
pub use module::{
    ParticleModule, ModuleList,
    GravityModule, ColorOverLifeModule, SizeOverLifeModule,
    VelocityOverLifeModule, NoiseModule, RotationModule,
};
pub use sim_cpu::CpuSimulator;
pub use sim_gpu::{GpuSimulator, SimParams};
pub use render::{ParticleRenderer, ParticleInstance, QuadVertex, BlendMode};
