/// Asset loader trait and built-in loaders for images, meshes, and scenes.
use crate::image::Image;
use crate::mesh::Mesh;
use crate::scene::SceneDefinition;

/// A type-erased loaded asset with its memory footprint.
pub struct LoadedAsset {
    pub data: Box<dyn std::any::Any + Send + Sync>,
    pub size_bytes: usize,
}

/// Trait for loading assets from raw bytes.
pub trait AssetLoader: Send + Sync + 'static {
    /// File extensions this loader handles (without the dot).
    fn extensions(&self) -> &[&str];

    /// Load an asset from raw bytes.
    fn load(&self, bytes: &[u8]) -> Result<LoadedAsset, String>;
}

// ─── Built-in loaders ─────────────────────────────────────────────────────────

/// Loads PNG images into `Image`.
pub struct ImageLoader;

impl AssetLoader for ImageLoader {
    fn extensions(&self) -> &[&str] {
        &["png"]
    }

    fn load(&self, bytes: &[u8]) -> Result<LoadedAsset, String> {
        let img = Image::decode_png(bytes)?;
        let size = img.size_bytes();
        Ok(LoadedAsset {
            data: Box::new(img),
            size_bytes: size,
        })
    }
}

/// Loads Wavefront OBJ meshes into `Mesh`.
pub struct MeshLoader;

impl AssetLoader for MeshLoader {
    fn extensions(&self) -> &[&str] {
        &["obj"]
    }

    fn load(&self, bytes: &[u8]) -> Result<LoadedAsset, String> {
        let text = std::str::from_utf8(bytes).map_err(|e| format!("OBJ not valid UTF-8: {}", e))?;
        let mesh = Mesh::parse_obj(text)?;
        let size = mesh.size_bytes();
        Ok(LoadedAsset {
            data: Box::new(mesh),
            size_bytes: size,
        })
    }
}

/// Loads JSON scenes into `SceneDefinition`.
pub struct SceneLoader;

impl AssetLoader for SceneLoader {
    fn extensions(&self) -> &[&str] {
        &["json", "scene"]
    }

    fn load(&self, bytes: &[u8]) -> Result<LoadedAsset, String> {
        let text =
            std::str::from_utf8(bytes).map_err(|e| format!("scene not valid UTF-8: {}", e))?;
        let scene = SceneDefinition::from_json(text)?;
        let size = std::mem::size_of_val(&scene)
            + scene.entities.len() * std::mem::size_of::<crate::scene::EntityDescriptor>();
        Ok(LoadedAsset {
            data: Box::new(scene),
            size_bytes: size,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_loader_extensions() {
        let loader = ImageLoader;
        assert_eq!(loader.extensions(), &["png"]);
    }

    #[test]
    fn mesh_loader_extensions() {
        let loader = MeshLoader;
        assert_eq!(loader.extensions(), &["obj"]);
    }

    #[test]
    fn scene_loader_extensions() {
        let loader = SceneLoader;
        assert!(loader.extensions().contains(&"json"));
        assert!(loader.extensions().contains(&"scene"));
    }

    #[test]
    fn image_loader_loads_png() {
        let img = Image::solid(8, 8, [255, 0, 0, 255]);
        let png_bytes = img.encode_png().unwrap();
        let loader = ImageLoader;
        let loaded = loader.load(&png_bytes).unwrap();
        let result = loaded.data.downcast_ref::<Image>().unwrap();
        assert_eq!(result.width, 8);
        assert_eq!(result.height, 8);
        assert_eq!(result.pixel(0, 0), [255, 0, 0, 255]);
    }

    #[test]
    fn mesh_loader_loads_obj() {
        let obj = b"v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n";
        let loader = MeshLoader;
        let loaded = loader.load(obj).unwrap();
        let mesh = loaded.data.downcast_ref::<Mesh>().unwrap();
        assert_eq!(mesh.positions.len(), 3);
        assert_eq!(mesh.triangle_count(), 1);
    }

    #[test]
    fn scene_loader_loads_json() {
        let json = br#"[{"name":"test","components":{"transform":{"position":[0,0,0],"rotation":[0,0,0,1],"scale":[1,1,1]}}}]"#;
        let loader = SceneLoader;
        let loaded = loader.load(json).unwrap();
        let scene = loaded.data.downcast_ref::<SceneDefinition>().unwrap();
        assert_eq!(scene.entities.len(), 1);
        assert_eq!(scene.entities[0].name, "test");
    }
}
