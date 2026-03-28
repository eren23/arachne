//! Particle rendering: billboard quads with instanced rendering.

use crate::particle::ParticlePool;

/// Blend mode for particle rendering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlendMode {
    /// Additive blending (fire, glow effects).
    Additive,
    /// Standard alpha blending (smoke, dust).
    Alpha,
}

/// Per-particle instance data sent to the GPU for billboard rendering.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ParticleInstance {
    pub position: [f32; 2],
    pub size: f32,
    pub rotation: f32,
    pub color: [f32; 4],
}

impl ParticleInstance {
    /// Vertex buffer layout for instanced particle rendering.
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<ParticleInstance>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &[
            // position
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 0,
                shader_location: 2,
            },
            // size
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32,
                offset: 8,
                shader_location: 3,
            },
            // rotation
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32,
                offset: 12,
                shader_location: 4,
            },
            // color
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 16,
                shader_location: 5,
            },
        ],
    };
}

/// Billboard quad vertex (unit quad).
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct QuadVertex {
    pub position: [f32; 2],
    pub uv: [f32; 2],
}

impl QuadVertex {
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<QuadVertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 8,
                shader_location: 1,
            },
        ],
    };

    /// Unit quad vertices: 4 corners centered at origin, size 1x1.
    pub const QUAD_VERTICES: [QuadVertex; 4] = [
        QuadVertex {
            position: [-0.5, -0.5],
            uv: [0.0, 1.0],
        },
        QuadVertex {
            position: [0.5, -0.5],
            uv: [1.0, 1.0],
        },
        QuadVertex {
            position: [0.5, 0.5],
            uv: [1.0, 0.0],
        },
        QuadVertex {
            position: [-0.5, 0.5],
            uv: [0.0, 0.0],
        },
    ];

    /// Indices for two triangles forming the quad.
    pub const QUAD_INDICES: [u16; 6] = [0, 1, 2, 0, 2, 3];
}

/// Particle renderer: generates billboard quads for alive particles.
pub struct ParticleRenderer {
    /// Instance data buffer for this frame.
    instances: Vec<ParticleInstance>,
    /// Vertex buffer for the unit quad.
    quad_vbo: wgpu::Buffer,
    /// Index buffer for the unit quad.
    quad_ibo: wgpu::Buffer,
    /// Instance buffer (resized as needed).
    instance_buffer: Option<wgpu::Buffer>,
    instance_buffer_capacity: usize,
    /// Blend mode.
    pub blend_mode: BlendMode,
}

impl ParticleRenderer {
    /// Creates a new particle renderer.
    pub fn new(device: &wgpu::Device) -> Self {
        let quad_vbo = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("particle_quad_vbo"),
            size: std::mem::size_of_val(&QuadVertex::QUAD_VERTICES) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: true,
        });
        {
            let mut mapping = quad_vbo.slice(..).get_mapped_range_mut();
            mapping.copy_from_slice(bytemuck::cast_slice(&QuadVertex::QUAD_VERTICES));
        }
        quad_vbo.unmap();

        let quad_ibo = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("particle_quad_ibo"),
            size: std::mem::size_of_val(&QuadVertex::QUAD_INDICES) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: true,
        });
        {
            let mut mapping = quad_ibo.slice(..).get_mapped_range_mut();
            mapping.copy_from_slice(bytemuck::cast_slice(&QuadVertex::QUAD_INDICES));
        }
        quad_ibo.unmap();

        Self {
            instances: Vec::new(),
            quad_vbo,
            quad_ibo,
            instance_buffer: None,
            instance_buffer_capacity: 0,
            blend_mode: BlendMode::Alpha,
        }
    }

    /// Prepares instance data from the pool, using sorted indices from the CPU
    /// simulator for correct draw order.
    pub fn prepare(
        &mut self,
        pool: &ParticlePool,
        sorted_indices: &[usize],
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> u32 {
        self.instances.clear();

        for &idx in sorted_indices {
            if pool.is_alive(idx) {
                let p = pool.get(idx);
                self.instances.push(ParticleInstance {
                    position: [p.position.x, p.position.y],
                    size: p.size,
                    rotation: p.rotation,
                    color: p.color.to_array(),
                });
            }
        }

        let count = self.instances.len();
        if count == 0 {
            return 0;
        }

        // Resize instance buffer if needed
        if count > self.instance_buffer_capacity {
            let new_cap = count.max(256).next_power_of_two();
            let buf = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("particle_instance_buffer"),
                size: (new_cap * std::mem::size_of::<ParticleInstance>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.instance_buffer = Some(buf);
            self.instance_buffer_capacity = new_cap;
        }

        if let Some(ref buf) = self.instance_buffer {
            queue.write_buffer(buf, 0, bytemuck::cast_slice(&self.instances));
        }

        count as u32
    }

    /// Returns the number of instances prepared this frame.
    pub fn instance_count(&self) -> usize {
        self.instances.len()
    }

    /// Returns the quad vertex buffer.
    pub fn quad_vbo(&self) -> &wgpu::Buffer {
        &self.quad_vbo
    }

    /// Returns the quad index buffer.
    pub fn quad_ibo(&self) -> &wgpu::Buffer {
        &self.quad_ibo
    }

    /// Returns the instance buffer (if prepared).
    pub fn instance_buffer(&self) -> Option<&wgpu::Buffer> {
        self.instance_buffer.as_ref()
    }

    /// Returns the wgpu blend state for the current blend mode.
    pub fn blend_state(&self) -> wgpu::BlendState {
        match self.blend_mode {
            BlendMode::Additive => wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
            },
            BlendMode::Alpha => wgpu::BlendState::ALPHA_BLENDING,
        }
    }
}

/// Returns the wgpu blend state for a given blend mode.
pub fn blend_state_for(mode: BlendMode) -> wgpu::BlendState {
    match mode {
        BlendMode::Additive => wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::SrcAlpha,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
        },
        BlendMode::Alpha => wgpu::BlendState::ALPHA_BLENDING,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::ModuleList;
    use crate::particle::Particle;
    use crate::sim_cpu::CpuSimulator;
    use arachne_math::Vec2;

    fn test_ctx() -> (wgpu::Device, wgpu::Queue) {
        pollster::block_on(async {
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::LowPower,
                    ..Default::default()
                })
                .await
                .expect("no GPU adapter found");
            adapter
                .request_device(&wgpu::DeviceDescriptor::default(), None)
                .await
                .expect("device creation failed")
        })
    }

    #[test]
    fn instance_count_matches_alive() {
        let (device, queue) = test_ctx();
        let mut pool = ParticlePool::new(100);

        // Spawn 42 particles
        for i in 0..42 {
            let mut p = Particle::default();
            p.position = Vec2::new(i as f32, 0.0);
            p.lifetime = 10.0;
            pool.spawn(p);
        }

        let modules = ModuleList::new();
        let mut cpu_sim = CpuSimulator::new();
        cpu_sim.step(&mut pool, &modules, 0.0);

        let mut renderer = ParticleRenderer::new(&device);
        let count = renderer.prepare(&pool, cpu_sim.sorted_indices(), &device, &queue);

        assert_eq!(count, 42, "instance count should match alive count");
        assert_eq!(renderer.instance_count(), 42);
    }

    #[test]
    fn blend_mode_additive() {
        let blend = blend_state_for(BlendMode::Additive);
        assert_eq!(blend.color.dst_factor, wgpu::BlendFactor::One);
    }

    #[test]
    fn blend_mode_alpha() {
        let blend = blend_state_for(BlendMode::Alpha);
        assert_eq!(blend, wgpu::BlendState::ALPHA_BLENDING);
    }

    #[test]
    fn quad_vertex_layout_is_correct() {
        assert_eq!(
            QuadVertex::LAYOUT.array_stride,
            std::mem::size_of::<QuadVertex>() as u64
        );
    }

    #[test]
    fn instance_layout_is_correct() {
        assert_eq!(
            ParticleInstance::LAYOUT.array_stride,
            std::mem::size_of::<ParticleInstance>() as u64
        );
    }
}
