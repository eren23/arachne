/// Asset bundle: concatenated binary blobs with a manifest header.
/// Single-file distribution format suitable for WASM deployment.
///
/// Binary format:
/// ```text
/// [4 bytes] magic: b"ABND"
/// [4 bytes] version: 1u32 LE
/// [4 bytes] entry count: u32 LE
/// Per entry (manifest):
///   [2 bytes] type_tag length: u16 LE
///   [N bytes] type_tag: UTF-8
///   [2 bytes] path length: u16 LE
///   [N bytes] path: UTF-8
///   [8 bytes] data offset from start of data section: u64 LE
///   [8 bytes] data size: u64 LE
/// [data section: concatenated blobs]
/// ```

use std::collections::HashMap;

const MAGIC: &[u8; 4] = b"ABND";
const VERSION: u32 = 1;

/// A single entry in a bundle manifest.
#[derive(Clone, Debug)]
pub struct BundleEntry {
    pub type_tag: String,
    pub path: String,
    pub offset: u64,
    pub size: u64,
}

/// An asset bundle containing multiple assets packed into a single binary.
pub struct AssetBundle {
    pub entries: Vec<BundleEntry>,
    pub data: Vec<u8>,
}

impl AssetBundle {
    /// Create a new empty bundle.
    pub fn new() -> Self {
        AssetBundle {
            entries: Vec::new(),
            data: Vec::new(),
        }
    }

    /// Add an asset to the bundle.
    pub fn add(&mut self, path: impl Into<String>, type_tag: impl Into<String>, data: &[u8]) {
        let offset = self.data.len() as u64;
        let size = data.len() as u64;
        self.data.extend_from_slice(data);
        self.entries.push(BundleEntry {
            type_tag: type_tag.into(),
            path: path.into(),
            offset,
            size,
        });
    }

    /// Serialize the bundle to bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();

        // Magic.
        out.extend_from_slice(MAGIC);
        // Version.
        out.extend_from_slice(&VERSION.to_le_bytes());
        // Entry count.
        out.extend_from_slice(&(self.entries.len() as u32).to_le_bytes());

        // Manifest entries.
        for entry in &self.entries {
            // Type tag.
            let tag_bytes = entry.type_tag.as_bytes();
            out.extend_from_slice(&(tag_bytes.len() as u16).to_le_bytes());
            out.extend_from_slice(tag_bytes);
            // Path.
            let path_bytes = entry.path.as_bytes();
            out.extend_from_slice(&(path_bytes.len() as u16).to_le_bytes());
            out.extend_from_slice(path_bytes);
            // Offset and size.
            out.extend_from_slice(&entry.offset.to_le_bytes());
            out.extend_from_slice(&entry.size.to_le_bytes());
        }

        // Data section.
        out.extend_from_slice(&self.data);

        out
    }

    /// Deserialize a bundle from bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<AssetBundle, String> {
        let mut pos = 0;

        // Magic.
        if bytes.len() < 12 {
            return Err("bundle too short for header".into());
        }
        if &bytes[pos..pos + 4] != MAGIC {
            return Err("invalid bundle magic".into());
        }
        pos += 4;

        // Version.
        let version = u32::from_le_bytes(
            bytes[pos..pos + 4]
                .try_into()
                .map_err(|_| "invalid version bytes")?,
        );
        if version != VERSION {
            return Err(format!("unsupported bundle version: {}", version));
        }
        pos += 4;

        // Entry count.
        let entry_count = u32::from_le_bytes(
            bytes[pos..pos + 4]
                .try_into()
                .map_err(|_| "invalid entry count bytes")?,
        ) as usize;
        pos += 4;

        // Read manifest.
        let mut entries = Vec::with_capacity(entry_count);
        for i in 0..entry_count {
            // Type tag.
            if pos + 2 > bytes.len() {
                return Err(format!("truncated manifest at entry {}", i));
            }
            let tag_len = u16::from_le_bytes(
                bytes[pos..pos + 2]
                    .try_into()
                    .map_err(|_| "invalid tag len")?,
            ) as usize;
            pos += 2;
            if pos + tag_len > bytes.len() {
                return Err(format!("truncated type_tag at entry {}", i));
            }
            let type_tag = std::str::from_utf8(&bytes[pos..pos + tag_len])
                .map_err(|_| format!("invalid UTF-8 in type_tag at entry {}", i))?
                .to_string();
            pos += tag_len;

            // Path.
            if pos + 2 > bytes.len() {
                return Err(format!("truncated path len at entry {}", i));
            }
            let path_len = u16::from_le_bytes(
                bytes[pos..pos + 2]
                    .try_into()
                    .map_err(|_| "invalid path len")?,
            ) as usize;
            pos += 2;
            if pos + path_len > bytes.len() {
                return Err(format!("truncated path at entry {}", i));
            }
            let path = std::str::from_utf8(&bytes[pos..pos + path_len])
                .map_err(|_| format!("invalid UTF-8 in path at entry {}", i))?
                .to_string();
            pos += path_len;

            // Offset and size.
            if pos + 16 > bytes.len() {
                return Err(format!("truncated offset/size at entry {}", i));
            }
            let offset = u64::from_le_bytes(
                bytes[pos..pos + 8]
                    .try_into()
                    .map_err(|_| "invalid offset")?,
            );
            pos += 8;
            let size = u64::from_le_bytes(
                bytes[pos..pos + 8]
                    .try_into()
                    .map_err(|_| "invalid size")?,
            );
            pos += 8;

            entries.push(BundleEntry {
                type_tag,
                path,
                offset,
                size,
            });
        }

        // Data section is everything from `pos` onward.
        let data = bytes[pos..].to_vec();

        // Validate all entries point within data.
        for (i, entry) in entries.iter().enumerate() {
            let end = entry.offset + entry.size;
            if end as usize > data.len() {
                return Err(format!(
                    "entry {} '{}' data range {}..{} exceeds data section size {}",
                    i,
                    entry.path,
                    entry.offset,
                    end,
                    data.len()
                ));
            }
        }

        Ok(AssetBundle { entries, data })
    }

    /// Get the data for an entry by path.
    pub fn get(&self, path: &str) -> Option<&[u8]> {
        self.entries
            .iter()
            .find(|e| e.path == path)
            .map(|e| &self.data[e.offset as usize..(e.offset + e.size) as usize])
    }

    /// Get the data for an entry by index.
    pub fn get_by_index(&self, index: usize) -> Option<&[u8]> {
        self.entries.get(index).map(|e| {
            &self.data[e.offset as usize..(e.offset + e.size) as usize]
        })
    }

    /// Build a lookup map from path -> index for fast access.
    pub fn build_index(&self) -> HashMap<String, usize> {
        self.entries
            .iter()
            .enumerate()
            .map(|(i, e)| (e.path.clone(), i))
            .collect()
    }
}

impl Default for AssetBundle {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_roundtrip_single() {
        let mut bundle = AssetBundle::new();
        bundle.add("test.txt", "text", b"Hello, bundle!");

        let bytes = bundle.to_bytes();
        let loaded = AssetBundle::from_bytes(&bytes).unwrap();

        assert_eq!(loaded.entries.len(), 1);
        assert_eq!(loaded.entries[0].path, "test.txt");
        assert_eq!(loaded.entries[0].type_tag, "text");
        assert_eq!(loaded.get("test.txt").unwrap(), b"Hello, bundle!");
    }

    #[test]
    fn bundle_roundtrip_multiple() {
        let mut bundle = AssetBundle::new();
        bundle.add("image.png", "image", &[0x89, 0x50, 0x4E, 0x47]); // PNG magic
        bundle.add("mesh.obj", "mesh", b"v 0 0 0\n");
        bundle.add(
            "scene.json",
            "scene",
            b"[{\"name\":\"test\",\"components\":{}}]",
        );

        let bytes = bundle.to_bytes();
        let loaded = AssetBundle::from_bytes(&bytes).unwrap();

        assert_eq!(loaded.entries.len(), 3);

        // Verify all entries present and data intact.
        assert_eq!(
            loaded.get("image.png").unwrap(),
            &[0x89, 0x50, 0x4E, 0x47]
        );
        assert_eq!(loaded.get("mesh.obj").unwrap(), b"v 0 0 0\n");
        assert_eq!(
            loaded.get("scene.json").unwrap(),
            b"[{\"name\":\"test\",\"components\":{}}]"
        );

        // Verify type tags.
        assert_eq!(loaded.entries[0].type_tag, "image");
        assert_eq!(loaded.entries[1].type_tag, "mesh");
        assert_eq!(loaded.entries[2].type_tag, "scene");
    }

    #[test]
    fn bundle_get_missing() {
        let bundle = AssetBundle::new();
        assert!(bundle.get("nonexistent").is_none());
    }

    #[test]
    fn bundle_get_by_index() {
        let mut bundle = AssetBundle::new();
        bundle.add("a.txt", "text", b"AAA");
        bundle.add("b.txt", "text", b"BBB");

        assert_eq!(bundle.get_by_index(0).unwrap(), b"AAA");
        assert_eq!(bundle.get_by_index(1).unwrap(), b"BBB");
        assert!(bundle.get_by_index(2).is_none());
    }

    #[test]
    fn bundle_build_index() {
        let mut bundle = AssetBundle::new();
        bundle.add("x.png", "image", b"X");
        bundle.add("y.obj", "mesh", b"Y");
        bundle.add("z.json", "scene", b"Z");

        let index = bundle.build_index();
        assert_eq!(index["x.png"], 0);
        assert_eq!(index["y.obj"], 1);
        assert_eq!(index["z.json"], 2);
    }

    #[test]
    fn bundle_invalid_magic() {
        let bytes = b"XXXX\x01\x00\x00\x00\x00\x00\x00\x00";
        assert!(AssetBundle::from_bytes(bytes).is_err());
    }

    #[test]
    fn bundle_empty() {
        let bundle = AssetBundle::new();
        let bytes = bundle.to_bytes();
        let loaded = AssetBundle::from_bytes(&bytes).unwrap();
        assert_eq!(loaded.entries.len(), 0);
        assert!(loaded.data.is_empty());
    }

    #[test]
    fn bundle_large_data() {
        let mut bundle = AssetBundle::new();
        let big_data = vec![0xAB; 100_000];
        bundle.add("big.bin", "binary", &big_data);

        let bytes = bundle.to_bytes();
        let loaded = AssetBundle::from_bytes(&bytes).unwrap();
        assert_eq!(loaded.get("big.bin").unwrap().len(), 100_000);
        assert!(loaded.get("big.bin").unwrap().iter().all(|&b| b == 0xAB));
    }
}
