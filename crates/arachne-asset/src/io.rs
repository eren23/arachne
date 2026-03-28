use std::collections::HashMap;
use std::fmt;

#[derive(Debug)]
pub struct IoError(pub String);

impl fmt::Display for IoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for IoError {}

/// Trait for platform-specific asset IO.
pub trait AssetIo: Send + Sync + 'static {
    fn read(&self, path: &str) -> Result<Vec<u8>, IoError>;
}

// --- Native filesystem IO ---

#[cfg(not(target_arch = "wasm32"))]
pub struct NativeIo {
    base_path: std::path::PathBuf,
}

#[cfg(not(target_arch = "wasm32"))]
impl NativeIo {
    pub fn new(base_path: impl Into<std::path::PathBuf>) -> Self {
        NativeIo {
            base_path: base_path.into(),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl AssetIo for NativeIo {
    fn read(&self, path: &str) -> Result<Vec<u8>, IoError> {
        let full = self.base_path.join(path);
        std::fs::read(&full).map_err(|e| IoError(format!("{}: {}", full.display(), e)))
    }
}

// --- WASM fetch IO (stub) ---

#[cfg(target_arch = "wasm32")]
pub struct WasmIo {
    base_url: String,
}

#[cfg(target_arch = "wasm32")]
impl WasmIo {
    pub fn new(base_url: impl Into<String>) -> Self {
        WasmIo {
            base_url: base_url.into(),
        }
    }
}

#[cfg(target_arch = "wasm32")]
impl AssetIo for WasmIo {
    fn read(&self, path: &str) -> Result<Vec<u8>, IoError> {
        // Real implementation would use fetch API via wasm-bindgen.
        // This is a compile-target stub.
        Err(IoError(format!(
            "WasmIo::read is not available as synchronous; use async load path for {}{}",
            self.base_url, path
        )))
    }
}

// --- In-memory IO for testing and embedded assets ---

pub struct MemoryIo {
    files: HashMap<String, Vec<u8>>,
}

impl MemoryIo {
    pub fn new() -> Self {
        MemoryIo {
            files: HashMap::new(),
        }
    }

    pub fn add(&mut self, path: impl Into<String>, data: Vec<u8>) {
        self.files.insert(path.into(), data);
    }

    pub fn with(mut self, path: impl Into<String>, data: Vec<u8>) -> Self {
        self.add(path, data);
        self
    }
}

impl Default for MemoryIo {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetIo for MemoryIo {
    fn read(&self, path: &str) -> Result<Vec<u8>, IoError> {
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| IoError(format!("file not found: {}", path)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_io_read_existing() {
        let io = MemoryIo::new().with("hello.txt", b"Hello, World!".to_vec());
        let data = io.read("hello.txt").unwrap();
        assert_eq!(data, b"Hello, World!");
    }

    #[test]
    fn memory_io_read_missing() {
        let io = MemoryIo::new();
        assert!(io.read("missing.txt").is_err());
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_io_read_missing() {
        let io = NativeIo::new("/nonexistent");
        assert!(io.read("file.txt").is_err());
    }
}
