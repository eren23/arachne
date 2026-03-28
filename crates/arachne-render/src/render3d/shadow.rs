use arachne_math::{Mat4, Vec3};

// ---------------------------------------------------------------------------
// Shadow uniform (GPU-side, matches ShadowUniforms in both shaders)
// ---------------------------------------------------------------------------

/// Matches `ShadowUniforms` in mesh_pbr.wgsl and shadow.wgsl.
///
/// Layout:
///   light_view_proj: mat4x4<f32>  (64 bytes)
///   shadow_params:   vec4<f32>    (16 bytes) — x = map_size
///
/// Total: 80 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ShadowUniform {
    pub light_view_proj: [[f32; 4]; 4],
    pub shadow_params: [f32; 4],
}

impl Default for ShadowUniform {
    fn default() -> Self {
        Self {
            light_view_proj: Mat4::IDENTITY.cols,
            shadow_params: [2048.0, 0.0, 0.0, 0.0],
        }
    }
}

impl ShadowUniform {
    pub fn new(light_view_proj: &Mat4, map_size: f32) -> Self {
        Self {
            light_view_proj: light_view_proj.cols,
            shadow_params: [map_size, 0.0, 0.0, 0.0],
        }
    }
}

// ---------------------------------------------------------------------------
// ShadowMap: owns the depth texture and provides helpers
// ---------------------------------------------------------------------------

/// Default shadow map resolution.
pub const DEFAULT_SHADOW_MAP_SIZE: u32 = 2048;

pub struct ShadowMap {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    size: u32,
}

impl ShadowMap {
    /// Create a shadow map with the given resolution.
    pub fn new(device: &wgpu::Device, size: u32) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shadow_map"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self { texture, size, view }
    }

    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    pub fn size(&self) -> u32 {
        self.size
    }

    /// Compute a light-space orthographic projection that encloses `scene_bounds`.
    ///
    /// `scene_min` and `scene_max` are the axis-aligned bounding box of the scene.
    /// `light_dir` is the normalized direction the light points (toward the scene).
    pub fn compute_light_projection(
        light_dir: Vec3,
        scene_min: Vec3,
        scene_max: Vec3,
    ) -> Mat4 {
        let center = (scene_min + scene_max) * 0.5;
        let radius = (scene_max - scene_min).length() * 0.5;

        let light_pos = center - light_dir * radius;

        // Choose an up vector that isn't parallel to the light direction
        let up = if light_dir.cross(Vec3::Y).length_squared() > 1e-4 {
            Vec3::Y
        } else {
            Vec3::Z
        };

        let light_view = Mat4::look_at(light_pos, center, up);
        let light_proj = Mat4::orthographic(
            -radius, radius,
            -radius, radius,
            0.0, radius * 2.0,
        );

        light_proj * light_view
    }

    /// Build a `ShadowUniform` for this shadow map with the given light projection.
    pub fn uniform(&self, light_view_proj: &Mat4) -> ShadowUniform {
        ShadowUniform::new(light_view_proj, self.size as f32)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::{UniformBuffer, VertexBuffer, IndexBuffer};
    use crate::render3d::mesh_render::{MeshVertex, MeshInstance, MeshRenderer};

    fn test_ctx() -> (wgpu::Device, wgpu::Queue) {
        pollster::block_on(async {
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::LowPower,
                    ..Default::default()
                })
                .await
                .expect("no GPU adapter");
            adapter
                .request_device(&wgpu::DeviceDescriptor::default(), None)
                .await
                .expect("device creation failed")
        })
    }

    #[test]
    fn shadow_uniform_size() {
        assert_eq!(std::mem::size_of::<ShadowUniform>(), 80);
    }

    #[test]
    fn shadow_map_creation() {
        let (device, _queue) = test_ctx();
        let shadow = ShadowMap::new(&device, 2048);
        assert_eq!(shadow.size(), 2048);
    }

    #[test]
    fn light_projection_encloses_scene() {
        let light_dir = Vec3::new(0.0, -1.0, -1.0).normalize();
        let scene_min = Vec3::new(-5.0, -5.0, -5.0);
        let scene_max = Vec3::new(5.0, 5.0, 5.0);

        let vp = ShadowMap::compute_light_projection(light_dir, scene_min, scene_max);

        // The center of the scene should project near the center of the shadow map
        let center = arachne_math::Vec4::new(0.0, 0.0, 0.0, 1.0);
        let clip = vp.mul_vec4(center);
        let ndc_x = clip.x / clip.w;
        let ndc_y = clip.y / clip.w;
        let ndc_z = clip.z / clip.w;

        assert!(ndc_x.abs() < 0.5, "center x should be near 0, got {ndc_x}");
        assert!(ndc_y.abs() < 0.5, "center y should be near 0, got {ndc_y}");
        assert!(ndc_z >= 0.0 && ndc_z <= 1.0, "center z should be in [0,1], got {ndc_z}");
    }

    #[test]
    fn shadow_map_render_and_readback() {
        let (device, queue) = test_ctx();
        let renderer = MeshRenderer::new(&device, &queue, wgpu::TextureFormat::Rgba8UnormSrgb);
        let shadow = ShadowMap::new(&device, 256);

        // Cube mesh
        let (vertices, indices) = MeshVertex::cube();
        let vb = VertexBuffer::new(&device, &vertices);
        let ib = IndexBuffer::new_u32(&device, &indices);
        let instance_buf = VertexBuffer::new(&device, &[MeshInstance::identity()]);

        // Light projection: looking at cube from upper-front
        let light_dir = Vec3::new(0.0, -1.0, -1.0).normalize();
        let light_vp = ShadowMap::compute_light_projection(
            light_dir,
            Vec3::new(-2.0, -2.0, -2.0),
            Vec3::new(2.0, 2.0, 2.0),
        );
        let shadow_uniform = shadow.uniform(&light_vp);
        let shadow_buf = UniformBuffer::new(&device, "shadow", &shadow_uniform);
        let shadow_bg = renderer.create_shadow_only_bind_group(&device, shadow_buf.buffer());

        // Render shadow pass
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("shadow_encoder"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow_pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: shadow.view(),
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });

            pass.set_pipeline(renderer.shadow_pipeline());
            pass.set_bind_group(0, &shadow_bg, &[]);
            pass.set_vertex_buffer(0, vb.slice());
            pass.set_vertex_buffer(1, instance_buf.slice());
            pass.set_index_buffer(ib.slice(), ib.format());
            pass.draw_indexed(0..ib.count(), 0, 0..1);
        }

        // Copy depth texture to staging buffer for readback
        let size = shadow.size();
        // Depth32Float rows must be aligned to 256 bytes for copy
        let bytes_per_row = size * 4;
        let aligned_bytes_per_row = (bytes_per_row + 255) & !255;
        let buffer_size = (aligned_bytes_per_row * size) as u64;
        let staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("shadow_readback"),
            size: buffer_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: shadow.texture(),
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::DepthOnly,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &staging,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(aligned_bytes_per_row),
                    rows_per_image: Some(size),
                },
            },
            wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
        );

        queue.submit(std::iter::once(encoder.finish()));

        // Map and read
        let slice = staging.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        device.poll(wgpu::Maintain::Wait);

        let data = slice.get_mapped_range();
        // Check that we have non-trivial depth values (not all 1.0)
        let mut has_non_trivial = false;
        for row in 0..size {
            let row_offset = (row * aligned_bytes_per_row) as usize;
            let row_data = &data[row_offset..row_offset + (size * 4) as usize];
            let depth_values: &[f32] = bytemuck::cast_slice(row_data);
            for &v in depth_values {
                if v > 0.0 && v < 1.0 {
                    has_non_trivial = true;
                    break;
                }
            }
            if has_non_trivial { break; }
        }

        drop(data);
        staging.unmap();

        assert!(has_non_trivial, "shadow map should have non-trivial depth values (between 0 and 1)");
    }
}
