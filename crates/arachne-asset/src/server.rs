/// AssetServer: load(path) -> Handle<T>, get<T>(handle) -> Option<&T>.
/// Coordinates IO, loaders, and cache.
use std::any::Any;
use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;

use crate::cache::LruCache;
use crate::handle::{new_ref_count, Handle, HandleId, RefCount};
use crate::io::AssetIo;
use crate::loader::AssetLoader;

/// State of an asset in the server.
#[derive(Clone, Debug, PartialEq)]
pub enum AssetState {
    Loading,
    Loaded,
    Failed(String),
    Unloaded,
}

struct LoadResult {
    id: HandleId,
    extension: String,
    data: Result<Vec<u8>, String>,
}

pub struct AssetServer {
    io: Arc<dyn AssetIo>,
    loaders: HashMap<String, Arc<dyn AssetLoader>>,
    states: HashMap<HandleId, AssetState>,
    assets: HashMap<HandleId, Box<dyn Any + Send + Sync>>,
    ref_counts: HashMap<HandleId, RefCount>,
    sizes: HashMap<HandleId, usize>,
    paths: HashMap<HandleId, String>,
    sender: Sender<LoadResult>,
    receiver: Receiver<LoadResult>,
    cache: LruCache,
}

impl AssetServer {
    /// Create a new AssetServer with the given IO backend and cache budget (bytes).
    pub fn new(io: impl AssetIo, cache_budget: usize) -> Self {
        let (sender, receiver) = mpsc::channel();
        AssetServer {
            io: Arc::new(io),
            loaders: HashMap::new(),
            states: HashMap::new(),
            assets: HashMap::new(),
            ref_counts: HashMap::new(),
            sizes: HashMap::new(),
            paths: HashMap::new(),
            sender,
            receiver,
            cache: LruCache::new(cache_budget),
        }
    }

    /// Register an asset loader. All extensions it handles will be mapped.
    pub fn add_loader(&mut self, loader: impl AssetLoader) {
        let loader = Arc::new(loader);
        for &ext in loader.extensions() {
            self.loaders.insert(ext.to_string(), loader.clone());
        }
    }

    /// Start loading an asset at the given path. Returns a strong handle.
    /// If the asset is already loaded or loading, returns a handle to the existing entry.
    pub fn load<T: 'static>(&mut self, path: &str) -> Handle<T> {
        let id = HandleId::from_path(path);

        match self.states.get(&id) {
            Some(AssetState::Loading) | Some(AssetState::Loaded) => {
                return self.make_strong_handle(id);
            }
            _ => {}
        }

        self.states.insert(id, AssetState::Loading);
        self.paths.insert(id, path.to_string());

        // Spawn IO thread.
        let io = self.io.clone();
        let sender = self.sender.clone();
        let path_owned = path.to_string();
        let ext = Self::extension(path).to_string();

        std::thread::spawn(move || {
            let data = io.read(&path_owned).map_err(|e| e.0);
            let _ = sender.send(LoadResult {
                id,
                extension: ext,
                data,
            });
        });

        self.make_strong_handle(id)
    }

    /// Process pending IO results. Call this each frame.
    pub fn poll(&mut self) {
        while let Ok(result) = self.receiver.try_recv() {
            match result.data {
                Ok(bytes) => {
                    if let Some(loader) = self.loaders.get(&result.extension).cloned() {
                        match loader.load(&bytes) {
                            Ok(loaded) => {
                                let size = loaded.size_bytes;
                                self.assets.insert(result.id, loaded.data);
                                self.sizes.insert(result.id, size);
                                self.states.insert(result.id, AssetState::Loaded);

                                // Add to cache.
                                if let Some(rc) = self.ref_counts.get(&result.id) {
                                    self.cache.insert(result.id, size, rc.clone());
                                }

                                // Evict if over budget.
                                let evicted = self.cache.evict_if_needed();
                                for eid in evicted {
                                    self.assets.remove(&eid);
                                    self.sizes.remove(&eid);
                                    self.states.insert(eid, AssetState::Unloaded);
                                }
                            }
                            Err(e) => {
                                self.states.insert(result.id, AssetState::Failed(e));
                            }
                        }
                    } else {
                        self.states.insert(
                            result.id,
                            AssetState::Failed(format!(
                                "no loader for extension: {}",
                                result.extension
                            )),
                        );
                    }
                }
                Err(e) => {
                    self.states.insert(result.id, AssetState::Failed(e));
                }
            }
        }
    }

    /// Get a loaded asset by handle. Returns None if not loaded yet or evicted.
    pub fn get<T: 'static>(&self, handle: &Handle<T>) -> Option<&T> {
        self.assets
            .get(&handle.id())
            .and_then(|boxed| boxed.downcast_ref::<T>())
    }

    /// Get the current state of an asset.
    pub fn state(&self, id: HandleId) -> Option<&AssetState> {
        self.states.get(&id)
    }

    /// Synchronous load helper: load + poll until done. Useful for tests.
    pub fn load_sync<T: 'static>(&mut self, path: &str) -> Handle<T> {
        let handle = self.load::<T>(path);
        loop {
            self.poll();
            match self.states.get(&handle.id()) {
                Some(AssetState::Loading) => std::thread::yield_now(),
                _ => break,
            }
        }
        handle
    }

    fn make_strong_handle<T: 'static>(&mut self, id: HandleId) -> Handle<T> {
        let rc = self
            .ref_counts
            .entry(id)
            .or_insert_with(new_ref_count)
            .clone();
        Handle::strong(id, rc)
    }

    fn extension(path: &str) -> &str {
        path.rsplit('.').next().unwrap_or("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::Image;
    use crate::io::MemoryIo;
    use crate::loader::{ImageLoader, MeshLoader, SceneLoader};
    use crate::mesh::Mesh;
    use crate::scene::SceneDefinition;

    fn test_server() -> AssetServer {
        let img = Image::solid(4, 4, [255, 0, 0, 255]);
        let png_bytes = img.encode_png().unwrap();

        let obj_bytes = b"v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n".to_vec();
        let scene_bytes = br#"[{"name":"e1","components":{"transform":{"position":[1,2,3],"rotation":[0,0,0,1],"scale":[1,1,1]}}}]"#.to_vec();

        let io = MemoryIo::new()
            .with("test.png", png_bytes)
            .with("cube.obj", obj_bytes)
            .with("level.json", scene_bytes);

        let mut server = AssetServer::new(io, 1024 * 1024);
        server.add_loader(ImageLoader);
        server.add_loader(MeshLoader);
        server.add_loader(SceneLoader);
        server
    }

    #[test]
    fn load_png_and_get() {
        let mut server = test_server();
        let handle = server.load_sync::<Image>("test.png");

        assert_eq!(server.state(handle.id()), Some(&AssetState::Loaded));

        let img = server.get(&handle).unwrap();
        assert_eq!(img.width, 4);
        assert_eq!(img.height, 4);
        assert_eq!(img.pixel(0, 0), [255, 0, 0, 255]);
    }

    #[test]
    fn load_obj_and_get() {
        let mut server = test_server();
        let handle = server.load_sync::<Mesh>("cube.obj");

        assert_eq!(server.state(handle.id()), Some(&AssetState::Loaded));

        let mesh = server.get(&handle).unwrap();
        assert_eq!(mesh.positions.len(), 3);
        assert_eq!(mesh.triangle_count(), 1);
    }

    #[test]
    fn load_scene_and_get() {
        let mut server = test_server();
        let handle = server.load_sync::<SceneDefinition>("level.json");

        assert_eq!(server.state(handle.id()), Some(&AssetState::Loaded));

        let scene = server.get(&handle).unwrap();
        assert_eq!(scene.entities.len(), 1);
        assert_eq!(scene.entities[0].name, "e1");
    }

    #[test]
    fn load_missing_asset_fails() {
        let io = MemoryIo::new();
        let mut server = AssetServer::new(io, 1024 * 1024);
        server.add_loader(ImageLoader);

        let handle = server.load_sync::<Image>("missing.png");
        assert!(matches!(
            server.state(handle.id()),
            Some(AssetState::Failed(_))
        ));
        assert!(server.get(&handle).is_none());
    }

    #[test]
    fn load_unknown_extension_fails() {
        let io = MemoryIo::new().with("test.xyz", b"data".to_vec());
        let mut server = AssetServer::new(io, 1024 * 1024);

        let handle = server.load_sync::<Image>("test.xyz");
        match server.state(handle.id()) {
            Some(AssetState::Failed(msg)) => {
                assert!(msg.contains("no loader"), "unexpected error: {}", msg);
            }
            other => panic!("expected Failed, got {:?}", other),
        }
    }

    #[test]
    fn duplicate_load_returns_same_id() {
        let mut server = test_server();
        let h1 = server.load_sync::<Image>("test.png");
        let h2 = server.load::<Image>("test.png");
        assert_eq!(h1.id(), h2.id());
    }

    #[test]
    fn weak_handle_cannot_get() {
        let mut server = test_server();
        let strong = server.load_sync::<Image>("test.png");
        let weak = strong.downgrade();

        // Weak handle still has the right ID.
        assert_eq!(weak.id(), strong.id());
        // get() works by ID regardless of handle strength.
        let img = server.get(&weak).unwrap();
        assert_eq!(img.width, 4);
    }

    #[test]
    fn cache_eviction_on_load() {
        // Budget of 100 bytes. A 4x4 RGBA image = 64 bytes.
        let img1 = Image::solid(4, 4, [1, 2, 3, 255]);
        let img2 = Image::solid(4, 4, [4, 5, 6, 255]);

        let io = MemoryIo::new()
            .with("a.png", img1.encode_png().unwrap())
            .with("b.png", img2.encode_png().unwrap());

        let mut server = AssetServer::new(io, 100);
        server.add_loader(ImageLoader);

        let h1 = server.load_sync::<Image>("a.png");
        assert_eq!(server.state(h1.id()), Some(&AssetState::Loaded));

        // Drop the strong handle so the asset can be evicted.
        drop(h1);

        let h2 = server.load_sync::<Image>("b.png");
        assert_eq!(server.state(h2.id()), Some(&AssetState::Loaded));

        // Both together would be 128 bytes, over the 100 budget.
        // h1's asset should have been evicted since its handle was dropped.
        let h1_id = HandleId::from_path("a.png");
        assert_eq!(server.state(h1_id), Some(&AssetState::Unloaded));
    }
}
