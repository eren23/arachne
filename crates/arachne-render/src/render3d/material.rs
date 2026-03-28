use arachne_math::Color;
use crate::texture::TextureHandle;

// ---------------------------------------------------------------------------
// PBR material (CPU-side)
// ---------------------------------------------------------------------------

/// Albedo source: either a flat color or a texture.
#[derive(Clone, Debug)]
pub enum Albedo {
    Color(Color),
    Texture(TextureHandle),
}

impl Default for Albedo {
    fn default() -> Self {
        Albedo::Color(Color::WHITE)
    }
}

/// A physically-based rendering material.
#[derive(Clone, Debug)]
pub struct PbrMaterial {
    pub albedo: Albedo,
    pub metallic: f32,
    pub roughness: f32,
    pub normal_map: Option<TextureHandle>,
    pub emissive: Option<Color>,
}

impl Default for PbrMaterial {
    fn default() -> Self {
        Self {
            albedo: Albedo::default(),
            metallic: 0.0,
            roughness: 0.5,
            normal_map: None,
            emissive: None,
        }
    }
}

impl PbrMaterial {
    pub fn new(albedo: Albedo, metallic: f32, roughness: f32) -> Self {
        Self {
            albedo,
            metallic,
            roughness,
            normal_map: None,
            emissive: None,
        }
    }

    /// Convert to a GPU-compatible uniform struct.
    pub fn to_uniform(&self) -> MaterialUniform {
        let (albedo_color, has_albedo_tex) = match &self.albedo {
            Albedo::Color(c) => ([c.r, c.g, c.b, c.a], 0u32),
            Albedo::Texture(_) => ([1.0, 1.0, 1.0, 1.0], 1u32),
        };
        let has_normal = if self.normal_map.is_some() { 1u32 } else { 0u32 };
        let emissive = self.emissive.unwrap_or(Color::TRANSPARENT);

        MaterialUniform {
            albedo: albedo_color,
            metallic: self.metallic,
            roughness: self.roughness,
            has_albedo_tex,
            has_normal_map: has_normal,
            emissive: [emissive.r, emissive.g, emissive.b, 0.0],
        }
    }
}

// ---------------------------------------------------------------------------
// GPU uniform (std140 compatible)
// ---------------------------------------------------------------------------

/// Matches `MaterialUniforms` in mesh_pbr.wgsl.
///
/// Layout (std140):
///   offset  0: albedo      vec4<f32>  (16 bytes)
///   offset 16: metallic    f32        ( 4 bytes)
///   offset 20: roughness   f32        ( 4 bytes)
///   offset 24: has_albedo  u32        ( 4 bytes)
///   offset 28: has_normal  u32        ( 4 bytes)
///   offset 32: emissive    vec4<f32>  (16 bytes)
///   total: 48 bytes
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MaterialUniform {
    pub albedo: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub has_albedo_tex: u32,
    pub has_normal_map: u32,
    pub emissive: [f32; 4],
}

impl Default for MaterialUniform {
    fn default() -> Self {
        Self {
            albedo: [1.0, 1.0, 1.0, 1.0],
            metallic: 0.0,
            roughness: 0.5,
            has_albedo_tex: 0,
            has_normal_map: 0,
            emissive: [0.0; 4],
        }
    }
}

// ---------------------------------------------------------------------------
// Handle & storage
// ---------------------------------------------------------------------------

/// Opaque handle to a material in `MaterialStorage`.
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
pub struct MaterialHandle(pub u32);

struct MaterialEntry {
    material: PbrMaterial,
    uniform: MaterialUniform,
}

/// Stores PBR materials and their cached GPU uniforms.
pub struct MaterialStorage {
    entries: Vec<MaterialEntry>,
}

impl MaterialStorage {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Add a material and return its handle.
    pub fn add(&mut self, material: PbrMaterial) -> MaterialHandle {
        let uniform = material.to_uniform();
        let handle = MaterialHandle(self.entries.len() as u32);
        self.entries.push(MaterialEntry { material, uniform });
        handle
    }

    /// Get the CPU-side material.
    pub fn get(&self, handle: MaterialHandle) -> &PbrMaterial {
        &self.entries[handle.0 as usize].material
    }

    /// Get the cached GPU uniform.
    pub fn get_uniform(&self, handle: MaterialHandle) -> &MaterialUniform {
        &self.entries[handle.0 as usize].uniform
    }

    /// Update a material and recompute its uniform.
    pub fn update(&mut self, handle: MaterialHandle, material: PbrMaterial) {
        let entry = &mut self.entries[handle.0 as usize];
        entry.uniform = material.to_uniform();
        entry.material = material;
    }

    pub fn count(&self) -> usize {
        self.entries.len()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_material_uniform() {
        let mat = PbrMaterial::default();
        let u = mat.to_uniform();

        assert_eq!(u.albedo, [1.0, 1.0, 1.0, 1.0]);
        assert_eq!(u.metallic, 0.0);
        assert_eq!(u.roughness, 0.5);
        assert_eq!(u.has_albedo_tex, 0);
        assert_eq!(u.has_normal_map, 0);
        assert_eq!(u.emissive, [0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn material_with_color_albedo() {
        let mat = PbrMaterial {
            albedo: Albedo::Color(Color::RED),
            metallic: 0.8,
            roughness: 0.2,
            normal_map: None,
            emissive: Some(Color::rgb(0.5, 0.0, 0.0)),
        };
        let u = mat.to_uniform();

        assert_eq!(u.albedo, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(u.metallic, 0.8);
        assert_eq!(u.roughness, 0.2);
        assert_eq!(u.has_albedo_tex, 0);
        assert_eq!(u.has_normal_map, 0);
        assert_eq!(u.emissive[0], 0.5);
    }

    #[test]
    fn material_with_texture_albedo() {
        let mat = PbrMaterial {
            albedo: Albedo::Texture(TextureHandle(42)),
            metallic: 1.0,
            roughness: 0.0,
            normal_map: Some(TextureHandle(7)),
            emissive: None,
        };
        let u = mat.to_uniform();

        // Texture albedo: color is white (tint), flag is 1
        assert_eq!(u.albedo, [1.0, 1.0, 1.0, 1.0]);
        assert_eq!(u.has_albedo_tex, 1);
        assert_eq!(u.has_normal_map, 1);
        assert_eq!(u.metallic, 1.0);
        assert_eq!(u.roughness, 0.0);
    }

    #[test]
    fn material_uniform_size_and_layout() {
        assert_eq!(std::mem::size_of::<MaterialUniform>(), 48);
        assert_eq!(std::mem::align_of::<MaterialUniform>(), 4);

        // Verify offsets via bytemuck
        let u = MaterialUniform::default();
        let bytes = bytemuck::bytes_of(&u);
        assert_eq!(bytes.len(), 48);

        // albedo at offset 0
        let albedo_bytes = &bytes[0..16];
        let albedo: &[f32] = bytemuck::cast_slice(albedo_bytes);
        assert_eq!(albedo, &[1.0, 1.0, 1.0, 1.0]);

        // metallic at offset 16
        let metallic: f32 = *bytemuck::from_bytes(&bytes[16..20]);
        assert_eq!(metallic, 0.0);

        // roughness at offset 20
        let roughness: f32 = *bytemuck::from_bytes(&bytes[20..24]);
        assert_eq!(roughness, 0.5);
    }

    #[test]
    fn material_storage_add_and_get() {
        let mut storage = MaterialStorage::new();
        let mat = PbrMaterial::new(Albedo::Color(Color::BLUE), 0.5, 0.3);
        let handle = storage.add(mat);

        assert_eq!(handle, MaterialHandle(0));
        assert_eq!(storage.count(), 1);
        assert_eq!(storage.get_uniform(handle).metallic, 0.5);
    }

    #[test]
    fn material_storage_update() {
        let mut storage = MaterialStorage::new();
        let handle = storage.add(PbrMaterial::default());

        let updated = PbrMaterial {
            metallic: 0.9,
            roughness: 0.1,
            ..PbrMaterial::default()
        };
        storage.update(handle, updated);

        assert_eq!(storage.get_uniform(handle).metallic, 0.9);
        assert_eq!(storage.get_uniform(handle).roughness, 0.1);
    }

    #[test]
    fn material_uniform_buffer_data_matches() {
        let mat = PbrMaterial {
            albedo: Albedo::Color(Color::new(0.8, 0.2, 0.1, 1.0)),
            metallic: 0.7,
            roughness: 0.3,
            normal_map: Some(TextureHandle(0)),
            emissive: Some(Color::rgb(1.0, 0.5, 0.0)),
        };
        let u = mat.to_uniform();
        let bytes = bytemuck::bytes_of(&u);

        // Reconstruct from raw bytes and verify
        let reconstructed: MaterialUniform = *bytemuck::from_bytes(bytes);
        assert_eq!(reconstructed.albedo[0], 0.8);
        assert_eq!(reconstructed.albedo[1], 0.2);
        assert_eq!(reconstructed.albedo[2], 0.1);
        assert_eq!(reconstructed.albedo[3], 1.0);
        assert_eq!(reconstructed.metallic, 0.7);
        assert_eq!(reconstructed.roughness, 0.3);
        assert_eq!(reconstructed.has_albedo_tex, 0); // Color, not texture
        assert_eq!(reconstructed.has_normal_map, 1);
        assert_eq!(reconstructed.emissive[0], 1.0);
        assert_eq!(reconstructed.emissive[1], 0.5);
        assert_eq!(reconstructed.emissive[2], 0.0);
    }
}
