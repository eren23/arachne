pub mod handle;
pub mod io;
pub mod image;
pub mod mesh;
pub mod scene;
pub mod cache;
pub mod loader;
pub mod server;
pub mod bundle;

pub use handle::{Handle, HandleId};
pub use io::{AssetIo, IoError, MemoryIo};
pub use image::{Image, UvRect, pack_atlas};
pub use mesh::Mesh;
pub use scene::{SceneDefinition, EntityDescriptor, ComponentData, JsonValue};
pub use cache::LruCache;
pub use loader::{AssetLoader, LoadedAsset, ImageLoader, MeshLoader, SceneLoader};
pub use server::{AssetServer, AssetState};
pub use bundle::AssetBundle;

#[cfg(not(target_arch = "wasm32"))]
pub use io::NativeIo;
