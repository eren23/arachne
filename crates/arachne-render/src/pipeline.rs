use std::collections::HashMap;
use std::hash::{Hash, Hasher};

/// A hashable key for cached render pipelines.
///
/// Composed of shader source hash, vertex layout hash, blend state, and depth state.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct PipelineKey {
    pub shader_hash: u64,
    pub vertex_layout_hash: u64,
    pub blend_enabled: bool,
    pub depth_enabled: bool,
    pub topology: PrimitiveTopologyKey,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum PrimitiveTopologyKey {
    TriangleList,
    TriangleStrip,
    LineList,
    LineStrip,
    PointList,
}

impl PrimitiveTopologyKey {
    pub fn to_wgpu(self) -> wgpu::PrimitiveTopology {
        match self {
            Self::TriangleList => wgpu::PrimitiveTopology::TriangleList,
            Self::TriangleStrip => wgpu::PrimitiveTopology::TriangleStrip,
            Self::LineList => wgpu::PrimitiveTopology::LineList,
            Self::LineStrip => wgpu::PrimitiveTopology::LineStrip,
            Self::PointList => wgpu::PrimitiveTopology::PointList,
        }
    }
}

/// Cache statistics.
#[derive(Clone, Debug, Default)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub entries: usize,
}

impl CacheStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            return 1.0;
        }
        self.hits as f64 / total as f64
    }
}

/// Caches render pipelines keyed by a hashable descriptor digest.
pub struct PipelineCache {
    cache: HashMap<PipelineKey, wgpu::RenderPipeline>,
    stats: CacheStats,
}

impl PipelineCache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
            stats: CacheStats::default(),
        }
    }

    /// Get a cached pipeline or create one via the provided closure.
    pub fn get_or_create(
        &mut self,
        key: &PipelineKey,
        create_fn: impl FnOnce() -> wgpu::RenderPipeline,
    ) -> &wgpu::RenderPipeline {
        if self.cache.contains_key(key) {
            self.stats.hits += 1;
        } else {
            self.stats.misses += 1;
            let pipeline = create_fn();
            self.cache.insert(key.clone(), pipeline);
        }
        self.stats.entries = self.cache.len();
        self.cache.get(key).unwrap()
    }

    pub fn contains(&self, key: &PipelineKey) -> bool {
        self.cache.contains_key(key)
    }

    pub fn stats(&self) -> &CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.cache.clear();
        self.stats.entries = 0;
    }
}

/// Hash a string to produce a shader source hash for pipeline keys.
pub fn hash_shader_source(source: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    source.hash(&mut hasher);
    hasher.finish()
}

/// Hash a vertex buffer layout description for pipeline keys.
pub fn hash_vertex_layout(attributes: &[wgpu::VertexAttribute], stride: u64) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    stride.hash(&mut hasher);
    for attr in attributes {
        attr.offset.hash(&mut hasher);
        attr.shader_location.hash(&mut hasher);
        // Hash the format as its debug repr
        std::mem::discriminant(&attr.format).hash(&mut hasher);
    }
    hasher.finish()
}

/// Create a shader module from embedded WGSL source.
pub fn create_shader_module(device: &wgpu::Device, label: &str, source: &str) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipeline_key_equality() {
        let k1 = PipelineKey {
            shader_hash: 123,
            vertex_layout_hash: 456,
            blend_enabled: true,
            depth_enabled: false,
            topology: PrimitiveTopologyKey::TriangleList,
        };
        let k2 = k1.clone();
        assert_eq!(k1, k2);
    }

    #[test]
    fn pipeline_key_inequality() {
        let k1 = PipelineKey {
            shader_hash: 123,
            vertex_layout_hash: 456,
            blend_enabled: true,
            depth_enabled: false,
            topology: PrimitiveTopologyKey::TriangleList,
        };
        let k2 = PipelineKey {
            shader_hash: 999,
            ..k1.clone()
        };
        assert_ne!(k1, k2);
    }

    fn create_camera_bgl(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("camera_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        })
    }

    #[test]
    fn cache_hit_and_miss() {
        let ctx = pollster::block_on(crate::context::RenderContext::new_headless()).unwrap();
        let device = ctx.device();
        let mut cache = PipelineCache::new();

        let shader_src = include_str!("shaders/shape.wgsl");
        let shader_hash = hash_shader_source(shader_src);

        let key = PipelineKey {
            shader_hash,
            vertex_layout_hash: 0,
            blend_enabled: false,
            depth_enabled: false,
            topology: PrimitiveTopologyKey::TriangleList,
        };

        let camera_bgl = create_camera_bgl(device);

        // Miss: first request
        let _pipeline = cache.get_or_create(&key, || {
            let module = create_shader_module(device, "shape", shader_src);
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("test"),
                bind_group_layouts: &[&camera_bgl],
                push_constant_ranges: &[],
            });
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("test"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs_main"),
                    buffers: &[crate::render2d::shape::ShapeVertex::LAYOUT],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba8UnormSrgb,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            })
        });
        assert_eq!(cache.stats().misses, 1);
        assert_eq!(cache.stats().hits, 0);

        // Hit: second request with same key
        let _pipeline = cache.get_or_create(&key, || {
            panic!("should not be called on cache hit");
        });
        assert_eq!(cache.stats().misses, 1);
        assert_eq!(cache.stats().hits, 1);
        assert!(cache.stats().hit_rate() >= 0.5);

        // Miss: different key
        let key2 = PipelineKey {
            shader_hash: shader_hash.wrapping_add(1),
            ..key.clone()
        };
        assert!(!cache.contains(&key2));
    }

    #[test]
    fn shader_hash_consistency() {
        let src = "fn main() {}";
        assert_eq!(hash_shader_source(src), hash_shader_source(src));
    }

    #[test]
    fn shader_hash_different_sources() {
        assert_ne!(hash_shader_source("fn a() {}"), hash_shader_source("fn b() {}"));
    }
}
