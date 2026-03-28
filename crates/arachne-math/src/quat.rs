//! Quaternion type for game engine math.

use core::ops::Mul;

use crate::mat3::Mat3;
use crate::vec3::Vec3;

/// A quaternion with `f32` components.
///
/// Stored as `(x, y, z, w)` where `w` is the scalar part.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quat {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

impl Quat {
    /// The identity quaternion `(0, 0, 0, 1)`.
    pub const IDENTITY: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        w: 1.0,
    };
}

// ---------------------------------------------------------------------------
// Constructors & methods
// ---------------------------------------------------------------------------

impl Quat {
    /// Creates a new `Quat` from individual components.
    #[inline]
    pub fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }

    /// Creates a quaternion from an axis and an angle (in radians).
    ///
    /// The axis **must** be normalized before calling this function.
    #[inline]
    pub fn from_axis_angle(axis: Vec3, angle: f32) -> Self {
        let half = angle * 0.5;
        let (s, c) = half.sin_cos();
        Self {
            x: axis.x * s,
            y: axis.y * s,
            z: axis.z * s,
            w: c,
        }
    }

    /// Creates a quaternion from Euler angles using **YXZ** rotation order
    /// (yaw = Y, pitch = X, roll = Z).
    ///
    /// This is the standard game-engine convention.
    #[inline]
    pub fn from_euler(yaw: f32, pitch: f32, roll: f32) -> Self {
        let (sy, cy) = (yaw * 0.5).sin_cos();
        let (sp, cp) = (pitch * 0.5).sin_cos();
        let (sr, cr) = (roll * 0.5).sin_cos();

        // YXZ order: Q = Qy * Qx * Qz
        Self {
            x: cy * sp * cr + sy * cp * sr,
            y: sy * cp * cr - cy * sp * sr,
            z: cy * cp * sr - sy * sp * cr,
            w: cy * cp * cr + sy * sp * sr,
        }
    }

    /// Extracts Euler angles from the quaternion assuming **YXZ** rotation order.
    ///
    /// Returns `(yaw, pitch, roll)`.
    #[inline]
    pub fn to_euler(self) -> (f32, f32, f32) {
        // Build the relevant matrix elements from the quaternion.
        let x2 = self.x + self.x;
        let y2 = self.y + self.y;
        let z2 = self.z + self.z;
        let xx = self.x * x2;
        let xy = self.x * y2;
        let xz = self.x * z2;
        let yy = self.y * y2;
        let yz = self.y * z2;
        let zz = self.z * z2;
        let wx = self.w * x2;
        let wy = self.w * y2;
        let wz = self.w * z2;

        // Rotation matrix elements (column-major):
        // m[0][0] = 1 - yy - zz   m[1][0] = xy - wz       m[2][0] = xz + wy
        // m[0][1] = xy + wz        m[1][1] = 1 - xx - zz   m[2][1] = yz - wx
        // m[0][2] = xz - wy        m[1][2] = yz + wx        m[2][2] = 1 - xx - yy

        let m11 = 1.0 - xx - zz;
        let m20 = xz + wy;
        let m21 = yz - wx;
        let m22 = 1.0 - xx - yy;
        let m01 = xy + wz;

        let pitch = (-m21).asin();
        let yaw = m20.atan2(m22);
        let roll = m01.atan2(m11);

        (yaw, pitch, roll)
    }

    /// Converts this quaternion to a 3x3 rotation matrix.
    #[inline]
    pub fn to_mat3(self) -> Mat3 {
        let x2 = self.x + self.x;
        let y2 = self.y + self.y;
        let z2 = self.z + self.z;
        let xx = self.x * x2;
        let xy = self.x * y2;
        let xz = self.x * z2;
        let yy = self.y * y2;
        let yz = self.y * z2;
        let zz = self.z * z2;
        let wx = self.w * x2;
        let wy = self.w * y2;
        let wz = self.w * z2;

        Mat3::from_cols(
            [1.0 - yy - zz, xy + wz, xz - wy],
            [xy - wz, 1.0 - xx - zz, yz + wx],
            [xz + wy, yz - wx, 1.0 - xx - yy],
        )
    }

    /// Converts this quaternion to a 4x4 rotation matrix.
    ///
    /// Delegates to [`crate::mat4::Mat4::from_quat`].
    #[inline]
    pub fn to_mat4(self) -> crate::mat4::Mat4 {
        crate::mat4::Mat4::from_quat(self)
    }

    /// Spherical linear interpolation between `self` and `other`.
    ///
    /// When `t == 0.0` the result equals `self`, and when `t == 1.0` the
    /// result equals `other`. Takes the shortest path on the unit sphere.
    #[inline]
    pub fn slerp(self, other: Quat, t: f32) -> Quat {
        let mut dot = self.dot(other);

        // If the dot product is negative, negate one quaternion to take the
        // shorter path around the 4D sphere.
        let other = if dot < 0.0 {
            dot = -dot;
            Quat::new(-other.x, -other.y, -other.z, -other.w)
        } else {
            other
        };

        // If the quaternions are very close, fall back to normalized linear
        // interpolation to avoid division by a near-zero sine.
        if dot > 0.9995 {
            return self.nlerp(other, t);
        }

        let theta = dot.acos();
        let sin_theta = theta.sin();
        let s0 = ((1.0 - t) * theta).sin() / sin_theta;
        let s1 = (t * theta).sin() / sin_theta;

        Quat::new(
            self.x * s0 + other.x * s1,
            self.y * s0 + other.y * s1,
            self.z * s0 + other.z * s1,
            self.w * s0 + other.w * s1,
        )
    }

    /// Normalized linear interpolation between `self` and `other`.
    ///
    /// Faster than [`slerp`](Self::slerp) but does not maintain constant
    /// angular velocity.
    #[inline]
    pub fn nlerp(self, other: Quat, t: f32) -> Quat {
        let dot = self.dot(other);
        let other = if dot < 0.0 {
            Quat::new(-other.x, -other.y, -other.z, -other.w)
        } else {
            other
        };

        Quat::new(
            self.x + (other.x - self.x) * t,
            self.y + (other.y - self.y) * t,
            self.z + (other.z - self.z) * t,
            self.w + (other.w - self.w) * t,
        )
        .normalize()
    }

    /// Returns the conjugate of this quaternion `(-x, -y, -z, w)`.
    #[inline]
    pub fn conjugate(self) -> Quat {
        Quat::new(-self.x, -self.y, -self.z, self.w)
    }

    /// Returns the inverse of this quaternion.
    ///
    /// For unit quaternions this is equivalent to [`conjugate`](Self::conjugate).
    #[inline]
    pub fn inverse(self) -> Quat {
        let len_sq = self.length_squared();
        let conj = self.conjugate();
        Quat::new(
            conj.x / len_sq,
            conj.y / len_sq,
            conj.z / len_sq,
            conj.w / len_sq,
        )
    }

    /// Returns the normalized (unit-length) version of this quaternion.
    #[inline]
    pub fn normalize(self) -> Quat {
        let len = self.length();
        if len == 0.0 {
            return Self::IDENTITY;
        }
        let inv = 1.0 / len;
        Quat::new(self.x * inv, self.y * inv, self.z * inv, self.w * inv)
    }

    /// Returns the length (magnitude) of this quaternion.
    #[inline]
    pub fn length(self) -> f32 {
        self.length_squared().sqrt()
    }

    /// Returns the squared length of this quaternion.
    #[inline]
    pub fn length_squared(self) -> f32 {
        self.x * self.x + self.y * self.y + self.z * self.z + self.w * self.w
    }

    /// Returns the dot product of `self` and `other`.
    #[inline]
    pub fn dot(self, other: Quat) -> f32 {
        self.x * other.x + self.y * other.y + self.z * other.z + self.w * other.w
    }

    /// Computes the Hamilton product of `self` and `rhs`.
    #[inline]
    pub fn mul_quat(self, rhs: Quat) -> Quat {
        Quat::new(
            self.w * rhs.x + self.x * rhs.w + self.y * rhs.z - self.z * rhs.y,
            self.w * rhs.y - self.x * rhs.z + self.y * rhs.w + self.z * rhs.x,
            self.w * rhs.z + self.x * rhs.y - self.y * rhs.x + self.z * rhs.w,
            self.w * rhs.w - self.x * rhs.x - self.y * rhs.y - self.z * rhs.z,
        )
    }

    /// Rotates the vector `v` by this quaternion.
    ///
    /// Uses the formula: `result = v + 2w * (q_xyz x v) + 2 * (q_xyz x (q_xyz x v))`
    /// which simplifies to `v + w * t + q_xyz x t` where `t = 2 * (q_xyz x v)`.
    #[inline]
    pub fn mul_vec3(self, v: Vec3) -> Vec3 {
        let q = Vec3::new(self.x, self.y, self.z);
        let t = q.cross(v) * 2.0;
        v + t * self.w + q.cross(t)
    }
}

// ---------------------------------------------------------------------------
// Operator overloads
// ---------------------------------------------------------------------------

impl Mul<Quat> for Quat {
    type Output = Quat;

    #[inline]
    fn mul(self, rhs: Quat) -> Quat {
        self.mul_quat(rhs)
    }
}

impl Mul<Vec3> for Quat {
    type Output = Vec3;

    #[inline]
    fn mul(self, rhs: Vec3) -> Vec3 {
        self.mul_vec3(rhs)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use core::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI};

    const EPSILON: f32 = 1e-5;

    fn assert_quat_approx_eq(a: Quat, b: Quat) {
        assert!(
            (a.x - b.x).abs() < EPSILON
                && (a.y - b.y).abs() < EPSILON
                && (a.z - b.z).abs() < EPSILON
                && (a.w - b.w).abs() < EPSILON,
            "assertion failed: {:?} != {:?} (within epsilon {})",
            a,
            b,
            EPSILON,
        );
    }

    fn assert_vec3_approx_eq(a: Vec3, b: Vec3) {
        assert!(
            (a.x - b.x).abs() < EPSILON
                && (a.y - b.y).abs() < EPSILON
                && (a.z - b.z).abs() < EPSILON,
            "assertion failed: {:?} != {:?} (within epsilon {})",
            a,
            b,
            EPSILON,
        );
    }

    fn assert_f32_approx(a: f32, b: f32) {
        assert!(
            (a - b).abs() < EPSILON,
            "assertion failed: {} != {} (within epsilon {})",
            a,
            b,
            EPSILON,
        );
    }

    // -- Identity ----------------------------------------------------------

    #[test]
    fn identity_rotate_vector() {
        let v = Vec3::new(1.0, 2.0, 3.0);
        let result = Quat::IDENTITY.mul_vec3(v);
        assert_vec3_approx_eq(result, v);
    }

    // -- from_axis_angle ---------------------------------------------------

    #[test]
    fn from_axis_angle_90_around_y() {
        let q = Quat::from_axis_angle(Vec3::Y, FRAC_PI_2);
        let v = Vec3::new(1.0, 0.0, 0.0);
        let result = q.mul_vec3(v);
        assert_vec3_approx_eq(result, Vec3::new(0.0, 0.0, -1.0));
    }

    // -- from_euler / to_euler roundtrip -----------------------------------

    #[test]
    fn euler_roundtrip() {
        let yaw = 0.3;
        let pitch = 0.5;
        let roll = 0.1;
        let q = Quat::from_euler(yaw, pitch, roll);
        let (ey, ep, er) = q.to_euler();
        assert_f32_approx(ey, yaw);
        assert_f32_approx(ep, pitch);
        assert_f32_approx(er, roll);
    }

    // -- slerp -------------------------------------------------------------

    #[test]
    fn slerp_at_zero_returns_self() {
        let a = Quat::from_axis_angle(Vec3::Y, 0.0);
        let b = Quat::from_axis_angle(Vec3::Y, FRAC_PI_2);
        let result = a.slerp(b, 0.0);
        assert_quat_approx_eq(result, a);
    }

    #[test]
    fn slerp_at_one_returns_other() {
        let a = Quat::from_axis_angle(Vec3::Y, 0.0);
        let b = Quat::from_axis_angle(Vec3::Y, FRAC_PI_2);
        let result = a.slerp(b, 1.0);
        assert_quat_approx_eq(result, b);
    }

    #[test]
    fn slerp_at_half_is_midpoint() {
        let a = Quat::from_axis_angle(Vec3::Y, 0.0);
        let b = Quat::from_axis_angle(Vec3::Y, FRAC_PI_2);
        let mid = a.slerp(b, 0.5);
        let expected = Quat::from_axis_angle(Vec3::Y, FRAC_PI_4);
        assert_quat_approx_eq(mid, expected);
    }

    // -- normalize ---------------------------------------------------------

    #[test]
    fn normalize_already_unit() {
        let q = Quat::from_axis_angle(Vec3::Y, 1.0);
        let n = q.normalize();
        assert_quat_approx_eq(n, q);
    }

    #[test]
    fn normalize_non_unit() {
        let q = Quat::new(0.0, 2.0, 0.0, 2.0);
        let n = q.normalize();
        assert_f32_approx(n.length(), 1.0);
    }

    #[test]
    fn normalize_idempotent() {
        let q = Quat::new(1.0, 2.0, 3.0, 4.0);
        let n1 = q.normalize();
        let n2 = n1.normalize();
        assert_quat_approx_eq(n1, n2);
    }

    // -- conjugate ---------------------------------------------------------

    #[test]
    fn conjugate_of_identity() {
        let c = Quat::IDENTITY.conjugate();
        assert_quat_approx_eq(c, Quat::IDENTITY);
    }

    // -- mul_quat ----------------------------------------------------------

    #[test]
    fn mul_quat_two_90_degree_rotations() {
        // Two 90-degree rotations around Y should equal a 180-degree rotation.
        let q90 = Quat::from_axis_angle(Vec3::Y, FRAC_PI_2);
        let q180 = q90.mul_quat(q90);
        let expected = Quat::from_axis_angle(Vec3::Y, PI);
        assert_quat_approx_eq(q180, expected);
    }

    // -- mul_vec3 ----------------------------------------------------------

    #[test]
    fn mul_vec3_rotate_unit_vector() {
        // 90-degree rotation around Z: X -> Y
        let q = Quat::from_axis_angle(Vec3::Z, FRAC_PI_2);
        let result = q.mul_vec3(Vec3::X);
        assert_vec3_approx_eq(result, Vec3::Y);
    }

    // -- Mul operator overloads --------------------------------------------

    #[test]
    fn mul_operator_quat_quat() {
        let a = Quat::from_axis_angle(Vec3::Y, FRAC_PI_4);
        let b = Quat::from_axis_angle(Vec3::Y, FRAC_PI_4);
        let product = a * b;
        let expected = a.mul_quat(b);
        assert_quat_approx_eq(product, expected);
    }

    #[test]
    fn mul_operator_quat_vec3() {
        let q = Quat::from_axis_angle(Vec3::Y, FRAC_PI_2);
        let v = Vec3::new(1.0, 0.0, 0.0);
        let result = q * v;
        assert_vec3_approx_eq(result, Vec3::new(0.0, 0.0, -1.0));
    }
}
