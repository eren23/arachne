//! Position + Rotation + Scale transform type for 3D game engine math.

use crate::mat4::Mat4;
use crate::quat::Quat;
use crate::vec3::Vec3;

/// A 3D transform composed of position, rotation, and scale.
///
/// The transform is applied in TRS order: **T**ranslation * **R**otation * **S**cale.
/// When transforming a point `p`, the result is `rotation * (scale * p) + position`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

impl Transform {
    /// The identity transform: zero position, identity rotation, unit scale.
    pub const IDENTITY: Self = Self {
        position: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    };
}

// ---------------------------------------------------------------------------
// Constructors & methods
// ---------------------------------------------------------------------------

impl Transform {
    /// Creates a new `Transform` from the given position, rotation, and scale.
    #[inline]
    pub fn new(position: Vec3, rotation: Quat, scale: Vec3) -> Self {
        Self {
            position,
            rotation,
            scale,
        }
    }

    /// Creates a transform with the given position, identity rotation, and unit
    /// scale.
    #[inline]
    pub fn from_position(position: Vec3) -> Self {
        Self {
            position,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }

    /// Creates a transform with zero position, the given rotation, and unit
    /// scale.
    #[inline]
    pub fn from_rotation(rotation: Quat) -> Self {
        Self {
            position: Vec3::ZERO,
            rotation,
            scale: Vec3::ONE,
        }
    }

    /// Creates a transform with zero position, identity rotation, and the given
    /// scale.
    #[inline]
    pub fn from_scale(scale: Vec3) -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale,
        }
    }

    /// Computes the 4x4 local-to-world matrix for this transform.
    ///
    /// The matrix is built as `Translation * Rotation * Scale`.
    #[inline]
    pub fn local_to_world(self) -> Mat4 {
        let t = Mat4::from_translation(self.position);
        let r = Mat4::from_quat(self.rotation);
        let s = Mat4::from_scale(self.scale);
        t * r * s
    }

    /// Composes two transforms such that `parent.compose(child)` produces a
    /// transform equivalent to first applying `child`, then `parent`.
    ///
    /// This is the standard parent-child hierarchy operation: the child's
    /// transform is expressed in the parent's local space.
    #[inline]
    pub fn compose(self, child: Transform) -> Transform {
        let scaled_child_pos = Vec3::new(
            child.position.x * self.scale.x,
            child.position.y * self.scale.y,
            child.position.z * self.scale.z,
        );
        let position = self.rotation.mul_vec3(scaled_child_pos) + self.position;
        let rotation = self.rotation.mul_quat(child.rotation);
        let scale = Vec3::new(
            self.scale.x * child.scale.x,
            self.scale.y * child.scale.y,
            self.scale.z * child.scale.z,
        );
        Transform {
            position,
            rotation,
            scale,
        }
    }

    /// Computes the inverse of this transform.
    ///
    /// For a TRS transform that maps a point as `R * (S * p) + T`, the inverse
    /// undoes that mapping. This is exact when scale is uniform but serves as a
    /// practical approximation for non-uniform scale, which is standard in game
    /// engines.
    #[inline]
    pub fn inverse(self) -> Transform {
        let inv_rotation = self.rotation.conjugate();
        let inv_scale = Vec3::new(
            1.0 / self.scale.x,
            1.0 / self.scale.y,
            1.0 / self.scale.z,
        );
        // The forward transform is: p' = R * (S * p) + T
        // Solving for p:  p = S^-1 * (R^-1 * (p' - T))
        //
        // Expressing this as a new TRS transform (new_R * (new_S * p') + new_T):
        //   new_R = R^-1
        //   new_S = S^-1
        //   new_T = -(S^-1 * (R^-1 * T))
        //
        // However since new_R * (new_S * p') applies scale then rotation, and we
        // need S^-1 * R^-1 * p', the TRS form only factors cleanly for uniform
        // scale. We compute the position that makes compose(self, inverse) =
        // IDENTITY:
        let neg_pos = Vec3::new(-self.position.x, -self.position.y, -self.position.z);
        let rotated = inv_rotation.mul_vec3(neg_pos);
        let inv_position = Vec3::new(
            rotated.x * inv_scale.x,
            rotated.y * inv_scale.y,
            rotated.z * inv_scale.z,
        );
        Transform::new(inv_position, inv_rotation, inv_scale)
    }

    /// Transforms a point by this transform (applies scale, rotation, and
    /// translation).
    ///
    /// Equivalent to `rotation * (scale * point) + position`.
    #[inline]
    pub fn transform_point(self, point: Vec3) -> Vec3 {
        let scaled = Vec3::new(
            point.x * self.scale.x,
            point.y * self.scale.y,
            point.z * self.scale.z,
        );
        self.rotation.mul_vec3(scaled) + self.position
    }

    /// Transforms a direction vector by this transform (applies scale and
    /// rotation only -- no translation).
    ///
    /// Equivalent to `rotation * (scale * vector)`.
    #[inline]
    pub fn transform_vector(self, vector: Vec3) -> Vec3 {
        let scaled = Vec3::new(
            vector.x * self.scale.x,
            vector.y * self.scale.y,
            vector.z * self.scale.z,
        );
        self.rotation.mul_vec3(scaled)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-5;

    fn approx_eq_f32(a: f32, b: f32) -> bool {
        (a - b).abs() < EPSILON
    }

    fn assert_vec3_approx(a: Vec3, b: Vec3) {
        assert!(
            approx_eq_f32(a.x, b.x) && approx_eq_f32(a.y, b.y) && approx_eq_f32(a.z, b.z),
            "Vec3 mismatch: {:?} != {:?} (epsilon {})",
            a,
            b,
            EPSILON,
        );
    }

    fn assert_mat4_approx(a: &Mat4, b: &Mat4) {
        for c in 0..4 {
            for r in 0..4 {
                assert!(
                    approx_eq_f32(a.cols[c][r], b.cols[c][r]),
                    "Mat4 mismatch at col={} row={}: {} != {} (epsilon {})\nlhs: {:?}\nrhs: {:?}",
                    c,
                    r,
                    a.cols[c][r],
                    b.cols[c][r],
                    EPSILON,
                    a,
                    b,
                );
            }
        }
    }

    // -- IDENTITY -----------------------------------------------------------

    #[test]
    fn identity_local_to_world_is_identity_matrix() {
        let m = Transform::IDENTITY.local_to_world();
        assert_mat4_approx(&m, &Mat4::IDENTITY);
    }

    // -- from_position ------------------------------------------------------

    #[test]
    fn from_position_has_translation_in_last_column() {
        let t = Transform::from_position(Vec3::new(3.0, 4.0, 5.0));
        let m = t.local_to_world();
        // Translation lives in column 3 of a column-major TRS matrix.
        assert!(approx_eq_f32(m.cols[3][0], 3.0));
        assert!(approx_eq_f32(m.cols[3][1], 4.0));
        assert!(approx_eq_f32(m.cols[3][2], 5.0));
        assert!(approx_eq_f32(m.cols[3][3], 1.0));
        // Upper-left 3x3 should be identity (no rotation, unit scale).
        assert!(approx_eq_f32(m.cols[0][0], 1.0));
        assert!(approx_eq_f32(m.cols[1][1], 1.0));
        assert!(approx_eq_f32(m.cols[2][2], 1.0));
    }

    // -- compose ------------------------------------------------------------

    #[test]
    fn compose_two_translations() {
        let parent = Transform::from_position(Vec3::new(1.0, 2.0, 3.0));
        let child = Transform::from_position(Vec3::new(10.0, 20.0, 30.0));
        let combined = parent.compose(child);

        assert_vec3_approx(combined.position, Vec3::new(11.0, 22.0, 33.0));
        assert_eq!(combined.rotation, Quat::IDENTITY);
        assert_vec3_approx(combined.scale, Vec3::ONE);
    }

    #[test]
    fn compose_with_scale() {
        let parent = Transform::new(
            Vec3::ZERO,
            Quat::IDENTITY,
            Vec3::new(2.0, 2.0, 2.0),
        );
        let child = Transform::from_position(Vec3::new(1.0, 1.0, 1.0));
        let combined = parent.compose(child);

        // Child position is scaled by parent scale.
        assert_vec3_approx(combined.position, Vec3::new(2.0, 2.0, 2.0));
        assert_vec3_approx(combined.scale, Vec3::new(2.0, 2.0, 2.0));
    }

    // -- inverse ------------------------------------------------------------

    #[test]
    fn compose_with_inverse_gives_identity() {
        let t = Transform::new(
            Vec3::new(3.0, -1.0, 7.0),
            Quat::from_axis_angle(Vec3::new(0.0, 1.0, 0.0), core::f32::consts::FRAC_PI_4),
            Vec3::new(2.0, 2.0, 2.0),
        );
        let inv = t.inverse();
        let result = t.compose(inv);

        let m = result.local_to_world();
        assert_mat4_approx(&m, &Mat4::IDENTITY);
    }

    #[test]
    fn inverse_of_identity_is_identity() {
        let inv = Transform::IDENTITY.inverse();
        assert_vec3_approx(inv.position, Vec3::ZERO);
        assert_eq!(inv.rotation, Quat::IDENTITY);
        assert_vec3_approx(inv.scale, Vec3::ONE);
    }

    // -- transform_point ----------------------------------------------------

    #[test]
    fn transform_point_identity() {
        let p = Vec3::new(1.0, 2.0, 3.0);
        let result = Transform::IDENTITY.transform_point(p);
        assert_vec3_approx(result, p);
    }

    #[test]
    fn transform_point_translation() {
        let t = Transform::from_position(Vec3::new(10.0, 20.0, 30.0));
        let result = t.transform_point(Vec3::new(1.0, 2.0, 3.0));
        assert_vec3_approx(result, Vec3::new(11.0, 22.0, 33.0));
    }

    #[test]
    fn transform_point_scale() {
        let t = Transform::from_scale(Vec3::new(2.0, 3.0, 4.0));
        let result = t.transform_point(Vec3::new(1.0, 1.0, 1.0));
        assert_vec3_approx(result, Vec3::new(2.0, 3.0, 4.0));
    }

    // -- transform_vector ---------------------------------------------------

    #[test]
    fn transform_vector_ignores_translation() {
        let t = Transform::from_position(Vec3::new(100.0, 200.0, 300.0));
        let v = Vec3::new(1.0, 0.0, 0.0);
        let result = t.transform_vector(v);
        // Translation should NOT affect direction vectors.
        assert_vec3_approx(result, Vec3::new(1.0, 0.0, 0.0));
    }

    #[test]
    fn transform_vector_applies_scale() {
        let t = Transform::from_scale(Vec3::new(2.0, 3.0, 4.0));
        let v = Vec3::new(1.0, 1.0, 1.0);
        let result = t.transform_vector(v);
        assert_vec3_approx(result, Vec3::new(2.0, 3.0, 4.0));
    }

    #[test]
    fn transform_vector_applies_rotation() {
        // 90-degree rotation around Y axis: X -> Z, Z -> -X
        let t = Transform::from_rotation(Quat::from_axis_angle(
            Vec3::new(0.0, 1.0, 0.0),
            core::f32::consts::FRAC_PI_2,
        ));
        let v = Vec3::new(1.0, 0.0, 0.0);
        let result = t.transform_vector(v);
        assert_vec3_approx(result, Vec3::new(0.0, 0.0, -1.0));
    }
}
