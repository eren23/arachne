use arachne_math::{Mat4, Vec3, Vec4};

use crate::skeleton::Skeleton;

// ---------------------------------------------------------------------------
// SkinVertex
// ---------------------------------------------------------------------------

/// Per-vertex skinning data: up to 4 bone influences.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SkinVertex {
    pub bone_indices: [u8; 4],
    pub bone_weights: [f32; 4],
}

impl SkinVertex {
    /// Create a skin vertex influenced by a single bone at full weight.
    pub fn single(bone_index: u8) -> Self {
        Self {
            bone_indices: [bone_index, 0, 0, 0],
            bone_weights: [1.0, 0.0, 0.0, 0.0],
        }
    }

    /// Create a skin vertex influenced by two bones.
    pub fn dual(bone_a: u8, weight_a: f32, bone_b: u8, weight_b: f32) -> Self {
        Self {
            bone_indices: [bone_a, bone_b, 0, 0],
            bone_weights: [weight_a, weight_b, 0.0, 0.0],
        }
    }

    /// Normalize weights so they sum to 1.0.
    pub fn normalize_weights(&mut self) {
        let sum: f32 = self.bone_weights.iter().sum();
        if sum > 0.0 {
            let inv = 1.0 / sum;
            for w in &mut self.bone_weights {
                *w *= inv;
            }
        }
    }

    /// Returns the number of active bone influences (weight > 0).
    pub fn influence_count(&self) -> usize {
        self.bone_weights.iter().filter(|&&w| w > 0.0).count()
    }
}

// ---------------------------------------------------------------------------
// SkinningData
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct SkinningData {
    pub vertices: Vec<SkinVertex>,
}

impl SkinningData {
    #[inline]
    pub fn new(vertices: Vec<SkinVertex>) -> Self {
        Self { vertices }
    }

    pub fn validate_weights(&self) -> bool {
        for v in &self.vertices {
            let sum: f32 = v.bone_weights.iter().sum();
            if (sum - 1.0).abs() > 1e-6 {
                return false;
            }
        }
        true
    }

    /// Normalize all vertex weights in-place.
    pub fn normalize_all_weights(&mut self) {
        for v in &mut self.vertices {
            v.normalize_weights();
        }
    }

    /// Returns the maximum bone index referenced by any vertex.
    pub fn max_bone_index(&self) -> u8 {
        self.vertices
            .iter()
            .flat_map(|v| {
                v.bone_indices
                    .iter()
                    .zip(v.bone_weights.iter())
                    .filter(|(_, &w)| w > 0.0)
                    .map(|(&idx, _)| idx)
            })
            .max()
            .unwrap_or(0)
    }

    /// Validate that all bone indices are within the given bone count.
    pub fn validate_indices(&self, bone_count: usize) -> bool {
        for v in &self.vertices {
            for i in 0..4 {
                if v.bone_weights[i] > 0.0 && (v.bone_indices[i] as usize) >= bone_count {
                    return false;
                }
            }
        }
        true
    }
}

// ---------------------------------------------------------------------------
// Joint matrix computation
// ---------------------------------------------------------------------------

/// Compute final joint (skinning) matrices from skeleton bind pose and current
/// global pose transforms.
///
/// `joint_matrix[i] = pose_global[i] * inverse_bind[i]`
pub fn compute_joint_matrices(skeleton: &Skeleton, pose_globals: &[Mat4]) -> Vec<Mat4> {
    let count = skeleton.bone_count();
    let mut result = Vec::with_capacity(count);
    for i in 0..count {
        result.push(pose_globals[i].mul_mat4(skeleton.inverse_bind_matrices[i]));
    }
    result
}

/// Compute joint matrices using dual quaternion skinning representation.
/// Returns pairs of (real, dual) quaternion components stored as Vec4.
/// This is a simplified representation suitable for GPU upload.
pub fn compute_joint_dual_quaternions(
    skeleton: &Skeleton,
    pose_globals: &[Mat4],
) -> Vec<[Vec4; 2]> {
    let count = skeleton.bone_count();
    let mut result = Vec::with_capacity(count);
    for i in 0..count {
        let joint = pose_globals[i].mul_mat4(skeleton.inverse_bind_matrices[i]);
        // Extract rotation quaternion (simplified: assume no shear/non-uniform scale)
        let quat = mat4_to_quaternion(&joint);
        let translation = Vec3::new(joint.cols[3][0], joint.cols[3][1], joint.cols[3][2]);

        // Dual part: 0.5 * (t_x, t_y, t_z, 0) * q
        let dual = Vec4::new(
            0.5 * (translation.x * quat.w + translation.y * quat.z - translation.z * quat.y),
            0.5 * (-translation.x * quat.z + translation.y * quat.w + translation.z * quat.x),
            0.5 * (translation.x * quat.y - translation.y * quat.x + translation.z * quat.w),
            0.5 * (-translation.x * quat.x - translation.y * quat.y - translation.z * quat.z),
        );

        result.push([quat, dual]);
    }
    result
}

/// Extract a quaternion (as Vec4: x, y, z, w) from a rotation matrix.
fn mat4_to_quaternion(m: &Mat4) -> Vec4 {
    let m00 = m.cols[0][0];
    let m11 = m.cols[1][1];
    let m22 = m.cols[2][2];
    let trace = m00 + m11 + m22;

    if trace > 0.0 {
        let s = (trace + 1.0).sqrt() * 2.0;
        let inv_s = 1.0 / s;
        Vec4::new(
            (m.cols[1][2] - m.cols[2][1]) * inv_s,
            (m.cols[2][0] - m.cols[0][2]) * inv_s,
            (m.cols[0][1] - m.cols[1][0]) * inv_s,
            0.25 * s,
        )
    } else if m00 > m11 && m00 > m22 {
        let s = (1.0 + m00 - m11 - m22).sqrt() * 2.0;
        let inv_s = 1.0 / s;
        Vec4::new(
            0.25 * s,
            (m.cols[1][0] + m.cols[0][1]) * inv_s,
            (m.cols[2][0] + m.cols[0][2]) * inv_s,
            (m.cols[1][2] - m.cols[2][1]) * inv_s,
        )
    } else if m11 > m22 {
        let s = (1.0 + m11 - m00 - m22).sqrt() * 2.0;
        let inv_s = 1.0 / s;
        Vec4::new(
            (m.cols[1][0] + m.cols[0][1]) * inv_s,
            0.25 * s,
            (m.cols[2][1] + m.cols[1][2]) * inv_s,
            (m.cols[2][0] - m.cols[0][2]) * inv_s,
        )
    } else {
        let s = (1.0 + m22 - m00 - m11).sqrt() * 2.0;
        let inv_s = 1.0 / s;
        Vec4::new(
            (m.cols[2][0] + m.cols[0][2]) * inv_s,
            (m.cols[2][1] + m.cols[1][2]) * inv_s,
            0.25 * s,
            (m.cols[0][1] - m.cols[1][0]) * inv_s,
        )
    }
}

// ---------------------------------------------------------------------------
// CPU skinning
// ---------------------------------------------------------------------------

/// Skin vertex positions on the CPU using linear blend skinning (LBS).
pub fn cpu_skin_positions(
    positions: &[Vec3],
    skin: &SkinningData,
    joint_matrices: &[Mat4],
) -> Vec<Vec3> {
    assert_eq!(positions.len(), skin.vertices.len());

    let mut output = Vec::with_capacity(positions.len());

    for (v_idx, sv) in skin.vertices.iter().enumerate() {
        let pos = positions[v_idx];
        let v4 = Vec4 {
            x: pos.x,
            y: pos.y,
            z: pos.z,
            w: 1.0,
        };

        let mut result = Vec4 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 0.0,
        };
        for j in 0..4 {
            let weight = sv.bone_weights[j];
            if weight <= 0.0 {
                continue;
            }
            let bone = sv.bone_indices[j] as usize;
            let transformed = joint_matrices[bone].mul_vec4(v4);
            result.x += transformed.x * weight;
            result.y += transformed.y * weight;
            result.z += transformed.z * weight;
            result.w += transformed.w * weight;
        }

        output.push(Vec3::new(result.x, result.y, result.z));
    }

    output
}

/// Skin vertex normals on the CPU. Normals are transformed by the inverse
/// transpose (which for rigid transforms is the upper-3x3 of the joint matrix).
pub fn cpu_skin_normals(
    normals: &[Vec3],
    skin: &SkinningData,
    joint_matrices: &[Mat4],
) -> Vec<Vec3> {
    assert_eq!(normals.len(), skin.vertices.len());

    let mut output = Vec::with_capacity(normals.len());

    for (v_idx, sv) in skin.vertices.iter().enumerate() {
        let n = normals[v_idx];
        let v4 = Vec4 {
            x: n.x,
            y: n.y,
            z: n.z,
            w: 0.0,
        }; // w=0 for direction

        let mut result = Vec4 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 0.0,
        };
        for j in 0..4 {
            let weight = sv.bone_weights[j];
            if weight <= 0.0 {
                continue;
            }
            let bone = sv.bone_indices[j] as usize;
            let transformed = joint_matrices[bone].mul_vec4(v4);
            result.x += transformed.x * weight;
            result.y += transformed.y * weight;
            result.z += transformed.z * weight;
        }

        // Re-normalize the result
        let len = (result.x * result.x + result.y * result.y + result.z * result.z).sqrt();
        if len > 1e-8 {
            let inv = 1.0 / len;
            output.push(Vec3::new(result.x * inv, result.y * inv, result.z * inv));
        } else {
            output.push(Vec3::new(0.0, 1.0, 0.0)); // fallback up
        }
    }

    output
}

/// Skin both positions and normals in a single pass (more cache-friendly).
pub fn cpu_skin_mesh(
    positions: &[Vec3],
    normals: &[Vec3],
    skin: &SkinningData,
    joint_matrices: &[Mat4],
) -> (Vec<Vec3>, Vec<Vec3>) {
    assert_eq!(positions.len(), skin.vertices.len());
    assert_eq!(normals.len(), skin.vertices.len());

    let n = positions.len();
    let mut out_pos = Vec::with_capacity(n);
    let mut out_norm = Vec::with_capacity(n);

    for (v_idx, sv) in skin.vertices.iter().enumerate() {
        let pos = positions[v_idx];
        let nor = normals[v_idx];
        let p4 = Vec4 { x: pos.x, y: pos.y, z: pos.z, w: 1.0 };
        let n4 = Vec4 { x: nor.x, y: nor.y, z: nor.z, w: 0.0 };

        let mut rp = Vec4 { x: 0.0, y: 0.0, z: 0.0, w: 0.0 };
        let mut rn = Vec4 { x: 0.0, y: 0.0, z: 0.0, w: 0.0 };

        for j in 0..4 {
            let weight = sv.bone_weights[j];
            if weight <= 0.0 { continue; }
            let bone = sv.bone_indices[j] as usize;
            let mat = &joint_matrices[bone];

            let tp = mat.mul_vec4(p4);
            rp.x += tp.x * weight;
            rp.y += tp.y * weight;
            rp.z += tp.z * weight;

            let tn = mat.mul_vec4(n4);
            rn.x += tn.x * weight;
            rn.y += tn.y * weight;
            rn.z += tn.z * weight;
        }

        out_pos.push(Vec3::new(rp.x, rp.y, rp.z));

        let len = (rn.x * rn.x + rn.y * rn.y + rn.z * rn.z).sqrt();
        if len > 1e-8 {
            let inv = 1.0 / len;
            out_norm.push(Vec3::new(rn.x * inv, rn.y * inv, rn.z * inv));
        } else {
            out_norm.push(Vec3::new(0.0, 1.0, 0.0));
        }
    }

    (out_pos, out_norm)
}

// ---------------------------------------------------------------------------
// GPU skinning data packing
// ---------------------------------------------------------------------------

/// Pack joint matrices into a flat f32 array suitable for GPU uniform/storage
/// buffer upload. Each matrix is stored as 16 consecutive f32 values
/// (column-major).
pub fn pack_joint_matrices_f32(joint_matrices: &[Mat4]) -> Vec<f32> {
    let mut data = Vec::with_capacity(joint_matrices.len() * 16);
    for mat in joint_matrices {
        for col in 0..4 {
            for row in 0..4 {
                data.push(mat.cols[col][row]);
            }
        }
    }
    data
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use arachne_math::Transform;
    use crate::skeleton::Skeleton;
    use core::hint::black_box;

    const EPSILON: f32 = 1e-4;

    fn make_simple_skeleton() -> Skeleton {
        let names = vec!["root".into(), "bone1".into()];
        let parents = vec![None, Some(0)];
        let bind_pose = vec![
            Transform::IDENTITY,
            Transform::from_position(Vec3::new(1.0, 0.0, 0.0)),
        ];
        Skeleton::new(names, parents, bind_pose)
    }

    #[test]
    fn single_bone_full_weight_translation() {
        let skeleton = make_simple_skeleton();

        // Pose: move root by (5,0,0) from bind
        let pose = vec![
            Transform::from_position(Vec3::new(5.0, 0.0, 0.0)),
            Transform::from_position(Vec3::new(1.0, 0.0, 0.0)),
        ];

        let pose_globals =
            Skeleton::compute_global_transforms(&pose, &skeleton.parent_indices);
        let joint_mats = compute_joint_matrices(&skeleton, &pose_globals);

        let positions = vec![Vec3::ZERO];
        let skin = SkinningData::new(vec![SkinVertex {
            bone_indices: [0, 0, 0, 0],
            bone_weights: [1.0, 0.0, 0.0, 0.0],
        }]);

        let result = cpu_skin_positions(&positions, &skin, &joint_mats);
        assert!(
            (result[0].x - 5.0).abs() < EPSILON,
            "expected x=5, got {}",
            result[0].x
        );
        assert!(
            (result[0].y - 0.0).abs() < EPSILON,
            "expected y=0, got {}",
            result[0].y
        );
        assert!(
            (result[0].z - 0.0).abs() < EPSILON,
            "expected z=0, got {}",
            result[0].z
        );
    }

    #[test]
    fn blend_weights_50_50() {
        let names = vec!["a".into(), "b".into()];
        let parents = vec![None, None];
        let bind_pose = vec![Transform::IDENTITY, Transform::IDENTITY];
        let skeleton = Skeleton::new(names, parents.clone(), bind_pose);

        let pose = vec![
            Transform::from_position(Vec3::new(10.0, 0.0, 0.0)),
            Transform::from_position(Vec3::new(0.0, 10.0, 0.0)),
        ];

        let pose_globals =
            Skeleton::compute_global_transforms(&pose, &parents);
        let joint_mats = compute_joint_matrices(&skeleton, &pose_globals);

        let positions = vec![Vec3::ZERO];
        let skin = SkinningData::new(vec![SkinVertex {
            bone_indices: [0, 1, 0, 0],
            bone_weights: [0.5, 0.5, 0.0, 0.0],
        }]);

        let result = cpu_skin_positions(&positions, &skin, &joint_mats);
        assert!(
            (result[0].x - 5.0).abs() < EPSILON,
            "50/50 blend x expected 5, got {}",
            result[0].x
        );
        assert!(
            (result[0].y - 5.0).abs() < EPSILON,
            "50/50 blend y expected 5, got {}",
            result[0].y
        );
    }

    #[test]
    fn validate_weights_sum_to_one() {
        let skin = SkinningData::new(vec![
            SkinVertex {
                bone_indices: [0, 1, 2, 3],
                bone_weights: [0.25, 0.25, 0.25, 0.25],
            },
            SkinVertex {
                bone_indices: [0, 0, 0, 0],
                bone_weights: [1.0, 0.0, 0.0, 0.0],
            },
        ]);
        assert!(skin.validate_weights());
    }

    #[test]
    fn validate_weights_bad_sum() {
        let skin = SkinningData::new(vec![SkinVertex {
            bone_indices: [0, 1, 0, 0],
            bone_weights: [0.5, 0.3, 0.0, 0.0],
        }]);
        assert!(!skin.validate_weights());
    }

    // -- New tests for extended functionality ----------------------------

    #[test]
    fn skin_vertex_single() {
        let sv = SkinVertex::single(3);
        assert_eq!(sv.bone_indices[0], 3);
        assert_eq!(sv.bone_weights[0], 1.0);
        assert_eq!(sv.influence_count(), 1);
    }

    #[test]
    fn skin_vertex_dual() {
        let sv = SkinVertex::dual(0, 0.6, 1, 0.4);
        assert_eq!(sv.influence_count(), 2);
    }

    #[test]
    fn normalize_weights() {
        let mut sv = SkinVertex {
            bone_indices: [0, 1, 0, 0],
            bone_weights: [0.5, 0.3, 0.0, 0.0],
        };
        sv.normalize_weights();
        let sum: f32 = sv.bone_weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-6);
    }

    #[test]
    fn normalize_all_weights() {
        let mut skin = SkinningData::new(vec![
            SkinVertex {
                bone_indices: [0, 1, 0, 0],
                bone_weights: [0.5, 0.3, 0.0, 0.0],
            },
        ]);
        assert!(!skin.validate_weights());
        skin.normalize_all_weights();
        assert!(skin.validate_weights());
    }

    #[test]
    fn max_bone_index() {
        let skin = SkinningData::new(vec![
            SkinVertex {
                bone_indices: [0, 5, 0, 0],
                bone_weights: [0.5, 0.5, 0.0, 0.0],
            },
            SkinVertex::single(3),
        ]);
        assert_eq!(skin.max_bone_index(), 5);
    }

    #[test]
    fn validate_indices() {
        let skin = SkinningData::new(vec![SkinVertex::single(2)]);
        assert!(skin.validate_indices(3));
        assert!(!skin.validate_indices(2));
    }

    #[test]
    fn cpu_skin_normals_preserves_unit_length() {
        let names = vec!["root".into()];
        let parents = vec![None];
        let bind_pose = vec![Transform::IDENTITY];
        let skeleton = Skeleton::new(names, parents.clone(), bind_pose);

        let pose = vec![Transform::from_position(Vec3::new(5.0, 0.0, 0.0))];
        let pose_globals = Skeleton::compute_global_transforms(&pose, &parents);
        let joint_mats = compute_joint_matrices(&skeleton, &pose_globals);

        let normals = vec![Vec3::new(0.0, 1.0, 0.0)];
        let skin = SkinningData::new(vec![SkinVertex::single(0)]);

        let result = cpu_skin_normals(&normals, &skin, &joint_mats);
        let len = (result[0].x * result[0].x + result[0].y * result[0].y + result[0].z * result[0].z).sqrt();
        assert!((len - 1.0).abs() < EPSILON, "normal length should be 1.0, got {}", len);
    }

    #[test]
    fn cpu_skin_mesh_combined() {
        let names = vec!["root".into()];
        let parents = vec![None];
        let bind_pose = vec![Transform::IDENTITY];
        let skeleton = Skeleton::new(names, parents.clone(), bind_pose);

        let pose = vec![Transform::from_position(Vec3::new(3.0, 0.0, 0.0))];
        let pose_globals = Skeleton::compute_global_transforms(&pose, &parents);
        let joint_mats = compute_joint_matrices(&skeleton, &pose_globals);

        let positions = vec![Vec3::ZERO];
        let normals = vec![Vec3::new(0.0, 1.0, 0.0)];
        let skin = SkinningData::new(vec![SkinVertex::single(0)]);

        let (out_pos, out_norm) = cpu_skin_mesh(&positions, &normals, &skin, &joint_mats);
        assert!((out_pos[0].x - 3.0).abs() < EPSILON);
        assert!((out_norm[0].y - 1.0).abs() < EPSILON);
    }

    #[test]
    fn pack_joint_matrices() {
        let mat = Mat4::IDENTITY;
        let data = pack_joint_matrices_f32(&[mat]);
        assert_eq!(data.len(), 16);
        assert_eq!(data[0], 1.0); // col 0, row 0
        assert_eq!(data[5], 1.0); // col 1, row 1
        assert_eq!(data[10], 1.0); // col 2, row 2
        assert_eq!(data[15], 1.0); // col 3, row 3
    }

    #[test]
    fn bench_100_bone_joint_matrices() {
        let count = 100;
        let names: Vec<String> = (0..count).map(|i| format!("bone_{i}")).collect();
        let parents: Vec<Option<usize>> = (0..count)
            .map(|i| if i == 0 { None } else { Some(i - 1) })
            .collect();
        let bind_pose: Vec<Transform> = (0..count)
            .map(|_| Transform::from_position(Vec3::new(0.1, 0.0, 0.0)))
            .collect();

        let skeleton = Skeleton::new(names, parents.clone(), bind_pose.clone());

        let pose_globals =
            Skeleton::compute_global_transforms(&bind_pose, &parents);

        let iterations = 1000u32;
        let start = std::time::Instant::now();
        for _ in 0..iterations {
            let jm = black_box(compute_joint_matrices(
                black_box(&skeleton),
                black_box(&pose_globals),
            ));
            black_box(jm);
        }
        let elapsed = start.elapsed();
        let per_iter_ms = elapsed.as_secs_f64() * 1000.0 / iterations as f64;
        eprintln!(
            "100-bone joint matrices: {:.4}ms per iteration ({} iterations in {:.3}ms)",
            per_iter_ms,
            iterations,
            elapsed.as_secs_f64() * 1000.0
        );
        assert!(
            per_iter_ms < 0.5,
            "100-bone joint matrices took {per_iter_ms:.4}ms, should be < 0.5ms"
        );
    }

    #[test]
    fn dual_quaternion_identity() {
        let names = vec!["root".into()];
        let parents = vec![None];
        let bind_pose = vec![Transform::IDENTITY];
        let skeleton = Skeleton::new(names, parents.clone(), bind_pose);

        let pose = vec![Transform::IDENTITY];
        let pose_globals = Skeleton::compute_global_transforms(&pose, &parents);
        let dqs = compute_joint_dual_quaternions(&skeleton, &pose_globals);

        assert_eq!(dqs.len(), 1);
        // For identity, real part should be (0,0,0,1) and dual part (0,0,0,0)
        let [real, dual] = dqs[0];
        assert!((real.w - 1.0).abs() < EPSILON || (real.w + 1.0).abs() < EPSILON,
            "identity quaternion w should be +/-1, got {}", real.w);
        assert!((dual.x).abs() < EPSILON);
        assert!((dual.y).abs() < EPSILON);
        assert!((dual.z).abs() < EPSILON);
    }
}
