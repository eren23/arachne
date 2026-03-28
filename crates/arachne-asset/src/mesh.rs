/// Minimal OBJ mesh parser. Handles positions, normals, texcoords, and triangulated faces.
/// No external dependencies.
use std::collections::HashMap;

pub struct Mesh {
    /// Deduplicated vertex positions (x, y, z).
    pub positions: Vec<[f32; 3]>,
    /// Per-vertex normals. Same length as `positions` if present, empty otherwise.
    pub normals: Vec<[f32; 3]>,
    /// Per-vertex texture coordinates. Same length as `positions` if present, empty otherwise.
    pub texcoords: Vec<[f32; 2]>,
    /// Triangle index buffer. Every 3 indices form a triangle.
    pub indices: Vec<u32>,
}

impl Mesh {
    /// Total triangle count.
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    /// Approximate memory usage in bytes.
    pub fn size_bytes(&self) -> usize {
        self.positions.len() * 12
            + self.normals.len() * 12
            + self.texcoords.len() * 8
            + self.indices.len() * 4
    }

    /// Parse a Wavefront OBJ string into a Mesh.
    ///
    /// Supports:
    /// - `v x y z` vertex positions
    /// - `vn x y z` vertex normals
    /// - `vt u v` texture coordinates
    /// - `f v/vt/vn ...` faces (triangulated if polygon)
    /// - `f v//vn ...` faces without texcoords
    /// - `f v ...` faces with positions only
    pub fn parse_obj(text: &str) -> Result<Mesh, String> {
        let mut raw_positions: Vec<[f32; 3]> = Vec::new();
        let mut raw_normals: Vec<[f32; 3]> = Vec::new();
        let mut raw_texcoords: Vec<[f32; 2]> = Vec::new();

        // (pos_idx, tc_idx, norm_idx) -> deduplicated vertex index
        let mut vertex_map: HashMap<(usize, Option<usize>, Option<usize>), u32> = HashMap::new();

        let mut positions: Vec<[f32; 3]> = Vec::new();
        let mut normals: Vec<[f32; 3]> = Vec::new();
        let mut texcoords: Vec<[f32; 2]> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();

        let has_normals;
        let has_texcoords;

        // First pass: collect raw data.
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let mut parts = line.split_whitespace();
            let prefix = match parts.next() {
                Some(p) => p,
                None => continue,
            };

            match prefix {
                "v" => {
                    let coords = parse_floats_3(&mut parts, line)?;
                    raw_positions.push(coords);
                }
                "vn" => {
                    let coords = parse_floats_3(&mut parts, line)?;
                    raw_normals.push(coords);
                }
                "vt" => {
                    let u = parse_f32(parts.next(), line)?;
                    let v = parse_f32(parts.next(), line)?;
                    raw_texcoords.push([u, v]);
                }
                "f" => {
                    // Collect face and handle later.
                }
                _ => {
                    // Ignore unknown prefixes (mtllib, usemtl, s, o, g, etc.)
                }
            }
        }

        has_normals = !raw_normals.is_empty();
        has_texcoords = !raw_texcoords.is_empty();

        // Second pass: process faces with raw data available for lookup.
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let mut parts = line.split_whitespace();
            let prefix = match parts.next() {
                Some(p) => p,
                None => continue,
            };

            if prefix != "f" {
                continue;
            }

            let face_verts: Vec<&str> = parts.collect();
            if face_verts.len() < 3 {
                return Err(format!("face with fewer than 3 vertices: {}", line));
            }

            // Parse each face vertex.
            let mut face_indices: Vec<u32> = Vec::with_capacity(face_verts.len());

            for vert_str in &face_verts {
                let (pi, ti, ni) = parse_face_vertex(vert_str, line)?;

                // Convert to 0-based.
                let pi0 = obj_index(pi, raw_positions.len())?;
                let ti0 = ti.map(|t| obj_index(t, raw_texcoords.len())).transpose()?;
                let ni0 = ni.map(|n| obj_index(n, raw_normals.len())).transpose()?;

                let key = (pi0, ti0, ni0);

                let vert_idx = if let Some(&existing) = vertex_map.get(&key) {
                    existing
                } else {
                    let idx = positions.len() as u32;
                    positions.push(raw_positions[pi0]);
                    if has_normals {
                        normals.push(ni0.map(|i| raw_normals[i]).unwrap_or([0.0, 0.0, 0.0]));
                    }
                    if has_texcoords {
                        texcoords.push(ti0.map(|i| raw_texcoords[i]).unwrap_or([0.0, 0.0]));
                    }
                    vertex_map.insert(key, idx);
                    idx
                };

                face_indices.push(vert_idx);
            }

            // Triangulate polygon (fan triangulation).
            for i in 1..face_indices.len() - 1 {
                indices.push(face_indices[0]);
                indices.push(face_indices[i]);
                indices.push(face_indices[i + 1]);
            }
        }

        Ok(Mesh {
            positions,
            normals,
            texcoords,
            indices,
        })
    }
}

/// Parse a face vertex like "1/2/3", "1//3", or "1".
fn parse_face_vertex(s: &str, _line: &str) -> Result<(i64, Option<i64>, Option<i64>), String> {
    let parts: Vec<&str> = s.split('/').collect();
    match parts.len() {
        1 => {
            let v: i64 = parts[0]
                .parse()
                .map_err(|_| format!("invalid vertex index: {}", s))?;
            Ok((v, None, None))
        }
        2 => {
            let v: i64 = parts[0]
                .parse()
                .map_err(|_| format!("invalid vertex index: {}", s))?;
            let vt: i64 = parts[1]
                .parse()
                .map_err(|_| format!("invalid texcoord index: {}", s))?;
            Ok((v, Some(vt), None))
        }
        3 => {
            let v: i64 = parts[0]
                .parse()
                .map_err(|_| format!("invalid vertex index: {}", s))?;
            let vt = if parts[1].is_empty() {
                None
            } else {
                Some(
                    parts[1]
                        .parse()
                        .map_err(|_| format!("invalid texcoord index: {}", s))?,
                )
            };
            let vn: i64 = parts[2]
                .parse()
                .map_err(|_| format!("invalid normal index: {}", s))?;
            Ok((v, vt, Some(vn)))
        }
        _ => Err(format!("invalid face vertex: {}", s)),
    }
}

/// Convert a 1-based OBJ index (possibly negative) to 0-based.
fn obj_index(idx: i64, count: usize) -> Result<usize, String> {
    let i = if idx > 0 {
        (idx - 1) as usize
    } else if idx < 0 {
        let abs = (-idx) as usize;
        if abs > count {
            return Err(format!("negative index {} out of range (count={})", idx, count));
        }
        count - abs
    } else {
        return Err("OBJ index 0 is invalid".into());
    };
    if i >= count {
        return Err(format!(
            "index {} out of range (count={})",
            idx, count
        ));
    }
    Ok(i)
}

fn parse_f32(val: Option<&str>, line: &str) -> Result<f32, String> {
    val.ok_or_else(|| format!("missing float in: {}", line))?
        .parse()
        .map_err(|_| format!("invalid float in: {}", line))
}

fn parse_floats_3(
    parts: &mut std::str::SplitWhitespace<'_>,
    line: &str,
) -> Result<[f32; 3], String> {
    let x = parse_f32(parts.next(), line)?;
    let y = parse_f32(parts.next(), line)?;
    let z = parse_f32(parts.next(), line)?;
    Ok([x, y, z])
}

#[cfg(test)]
mod tests {
    use super::*;

    const CUBE_OBJ: &str = r#"
# Unit cube
v -1.0 -1.0 -1.0
v  1.0 -1.0 -1.0
v  1.0  1.0 -1.0
v -1.0  1.0 -1.0
v -1.0 -1.0  1.0
v  1.0 -1.0  1.0
v  1.0  1.0  1.0
v -1.0  1.0  1.0

f 1 2 3
f 1 3 4
f 5 7 6
f 5 8 7
f 1 5 6
f 1 6 2
f 2 6 7
f 2 7 3
f 3 7 8
f 3 8 4
f 4 8 5
f 4 5 1
"#;

    #[test]
    fn parse_cube_obj() {
        let mesh = Mesh::parse_obj(CUBE_OBJ).unwrap();
        assert_eq!(mesh.positions.len(), 8, "cube should have 8 unique positions");
        assert_eq!(
            mesh.triangle_count(),
            12,
            "cube should have 12 triangles"
        );
        assert_eq!(mesh.indices.len(), 36);
        assert!(mesh.normals.is_empty());
        assert!(mesh.texcoords.is_empty());
    }

    #[test]
    fn parse_cube_positions_values() {
        let mesh = Mesh::parse_obj(CUBE_OBJ).unwrap();
        // All positions should be +/-1.0 on each axis.
        for pos in &mesh.positions {
            for &c in pos {
                assert!(
                    (c - 1.0).abs() < 0.001 || (c + 1.0).abs() < 0.001,
                    "unexpected coordinate: {}",
                    c
                );
            }
        }
    }

    const CUBE_WITH_NORMALS: &str = r#"
v 0 0 0
v 1 0 0
v 1 1 0
v 0 1 0
vn 0 0 -1
vn 0 0 1
f 1//1 2//1 3//1
f 1//1 3//1 4//1
"#;

    #[test]
    fn parse_obj_with_normals() {
        let mesh = Mesh::parse_obj(CUBE_WITH_NORMALS).unwrap();
        assert_eq!(mesh.positions.len(), 4);
        assert_eq!(mesh.normals.len(), 4);
        assert_eq!(mesh.triangle_count(), 2);
        // All normals should be (0, 0, -1).
        for n in &mesh.normals {
            assert!((n[2] + 1.0).abs() < 0.001);
        }
    }

    const QUAD_WITH_TC: &str = r#"
v 0 0 0
v 1 0 0
v 1 1 0
v 0 1 0
vt 0 0
vt 1 0
vt 1 1
vt 0 1
f 1/1 2/2 3/3
f 1/1 3/3 4/4
"#;

    #[test]
    fn parse_obj_with_texcoords() {
        let mesh = Mesh::parse_obj(QUAD_WITH_TC).unwrap();
        assert_eq!(mesh.positions.len(), 4);
        assert_eq!(mesh.texcoords.len(), 4);
        assert_eq!(mesh.triangle_count(), 2);
    }

    #[test]
    fn parse_polygon_face_triangulation() {
        // A single quad face should be split into 2 triangles.
        let obj = r#"
v 0 0 0
v 1 0 0
v 1 1 0
v 0 1 0
f 1 2 3 4
"#;
        let mesh = Mesh::parse_obj(obj).unwrap();
        assert_eq!(mesh.triangle_count(), 2);
        assert_eq!(mesh.indices.len(), 6);
    }

    #[test]
    fn negative_indices() {
        let obj = r#"
v 0 0 0
v 1 0 0
v 1 1 0
f -3 -2 -1
"#;
        let mesh = Mesh::parse_obj(obj).unwrap();
        assert_eq!(mesh.positions.len(), 3);
        assert_eq!(mesh.triangle_count(), 1);
    }
}
