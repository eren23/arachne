use core::ops::Mul;

use crate::quat::Quat;
use crate::vec3::Vec3;
use crate::vec4::Vec4;

/// A 4x4 column-major matrix of `f32` values.
///
/// `cols[i][j]` is column `i`, row `j`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat4 {
    pub cols: [[f32; 4]; 4],
}

impl Mat4 {
    pub const IDENTITY: Self = Self {
        cols: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    };

    pub const ZERO: Self = Self {
        cols: [
            [0.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 0.0],
        ],
    };

    /// Constructs a `Mat4` from four column arrays.
    #[inline]
    pub fn from_cols(c0: [f32; 4], c1: [f32; 4], c2: [f32; 4], c3: [f32; 4]) -> Self {
        Self {
            cols: [c0, c1, c2, c3],
        }
    }

    /// Matrix multiplication: `self * rhs`.
    ///
    /// Fully unrolled with scalar variables to avoid all array bounds checks
    /// in debug builds while enabling SIMD auto-vectorization in release.
    #[inline(always)]
    pub fn mul_mat4(self, rhs: Mat4) -> Mat4 {
        // Extract all 16 elements of self into scalars — zero array indexing
        // in the hot computation below.
        let a00 = self.cols[0][0]; let a01 = self.cols[0][1]; let a02 = self.cols[0][2]; let a03 = self.cols[0][3];
        let a10 = self.cols[1][0]; let a11 = self.cols[1][1]; let a12 = self.cols[1][2]; let a13 = self.cols[1][3];
        let a20 = self.cols[2][0]; let a21 = self.cols[2][1]; let a22 = self.cols[2][2]; let a23 = self.cols[2][3];
        let a30 = self.cols[3][0]; let a31 = self.cols[3][1]; let a32 = self.cols[3][2]; let a33 = self.cols[3][3];

        let b00 = rhs.cols[0][0]; let b01 = rhs.cols[0][1]; let b02 = rhs.cols[0][2]; let b03 = rhs.cols[0][3];
        let b10 = rhs.cols[1][0]; let b11 = rhs.cols[1][1]; let b12 = rhs.cols[1][2]; let b13 = rhs.cols[1][3];
        let b20 = rhs.cols[2][0]; let b21 = rhs.cols[2][1]; let b22 = rhs.cols[2][2]; let b23 = rhs.cols[2][3];
        let b30 = rhs.cols[3][0]; let b31 = rhs.cols[3][1]; let b32 = rhs.cols[3][2]; let b33 = rhs.cols[3][3];

        Mat4 { cols: [
            [
                a00 * b00 + a10 * b01 + a20 * b02 + a30 * b03,
                a01 * b00 + a11 * b01 + a21 * b02 + a31 * b03,
                a02 * b00 + a12 * b01 + a22 * b02 + a32 * b03,
                a03 * b00 + a13 * b01 + a23 * b02 + a33 * b03,
            ],
            [
                a00 * b10 + a10 * b11 + a20 * b12 + a30 * b13,
                a01 * b10 + a11 * b11 + a21 * b12 + a31 * b13,
                a02 * b10 + a12 * b11 + a22 * b12 + a32 * b13,
                a03 * b10 + a13 * b11 + a23 * b12 + a33 * b13,
            ],
            [
                a00 * b20 + a10 * b21 + a20 * b22 + a30 * b23,
                a01 * b20 + a11 * b21 + a21 * b22 + a31 * b23,
                a02 * b20 + a12 * b21 + a22 * b22 + a32 * b23,
                a03 * b20 + a13 * b21 + a23 * b22 + a33 * b23,
            ],
            [
                a00 * b30 + a10 * b31 + a20 * b32 + a30 * b33,
                a01 * b30 + a11 * b31 + a21 * b32 + a31 * b33,
                a02 * b30 + a12 * b31 + a22 * b32 + a32 * b33,
                a03 * b30 + a13 * b31 + a23 * b32 + a33 * b33,
            ],
        ]}
    }

    /// Multiplies this matrix by a `Vec4`, returning a `Vec4`.
    #[inline]
    pub fn mul_vec4(self, v: Vec4) -> Vec4 {
        Vec4 {
            x: self.cols[0][0] * v.x
                + self.cols[1][0] * v.y
                + self.cols[2][0] * v.z
                + self.cols[3][0] * v.w,
            y: self.cols[0][1] * v.x
                + self.cols[1][1] * v.y
                + self.cols[2][1] * v.z
                + self.cols[3][1] * v.w,
            z: self.cols[0][2] * v.x
                + self.cols[1][2] * v.y
                + self.cols[2][2] * v.z
                + self.cols[3][2] * v.w,
            w: self.cols[0][3] * v.x
                + self.cols[1][3] * v.y
                + self.cols[2][3] * v.z
                + self.cols[3][3] * v.w,
        }
    }

    /// Computes the determinant of this matrix.
    pub fn determinant(self) -> f32 {
        let m = &self.cols;

        // Convenience aliases: m[col][row] => mCR
        let m00 = m[0][0];
        let m01 = m[0][1];
        let m02 = m[0][2];
        let m03 = m[0][3];
        let m10 = m[1][0];
        let m11 = m[1][1];
        let m12 = m[1][2];
        let m13 = m[1][3];
        let m20 = m[2][0];
        let m21 = m[2][1];
        let m22 = m[2][2];
        let m23 = m[2][3];
        let m30 = m[3][0];
        let m31 = m[3][1];
        let m32 = m[3][2];
        let m33 = m[3][3];

        // 2x2 sub-determinants (same decomposition used by inverse).
        let s0 = m00 * m11 - m10 * m01;
        let s1 = m00 * m21 - m20 * m01;
        let s2 = m00 * m31 - m30 * m01;
        let s3 = m10 * m21 - m20 * m11;
        let s4 = m10 * m31 - m30 * m11;
        let s5 = m20 * m31 - m30 * m21;

        let c5 = m22 * m33 - m32 * m23;
        let c4 = m12 * m33 - m32 * m13;
        let c3 = m12 * m23 - m22 * m13;
        let c2 = m02 * m33 - m32 * m03;
        let c1 = m02 * m23 - m22 * m03;
        let c0 = m02 * m13 - m12 * m03;

        s0 * c5 - s1 * c4 + s2 * c3 + s3 * c2 - s4 * c1 + s5 * c0
    }

    /// Returns the inverse of this matrix, or `None` if the matrix is singular.
    pub fn inverse(self) -> Option<Mat4> {
        let m = &self.cols;

        let m00 = m[0][0];
        let m01 = m[0][1];
        let m02 = m[0][2];
        let m03 = m[0][3];
        let m10 = m[1][0];
        let m11 = m[1][1];
        let m12 = m[1][2];
        let m13 = m[1][3];
        let m20 = m[2][0];
        let m21 = m[2][1];
        let m22 = m[2][2];
        let m23 = m[2][3];
        let m30 = m[3][0];
        let m31 = m[3][1];
        let m32 = m[3][2];
        let m33 = m[3][3];

        // 2x2 sub-determinants
        let s0 = m00 * m11 - m10 * m01;
        let s1 = m00 * m21 - m20 * m01;
        let s2 = m00 * m31 - m30 * m01;
        let s3 = m10 * m21 - m20 * m11;
        let s4 = m10 * m31 - m30 * m11;
        let s5 = m20 * m31 - m30 * m21;

        let c5 = m22 * m33 - m32 * m23;
        let c4 = m12 * m33 - m32 * m13;
        let c3 = m12 * m23 - m22 * m13;
        let c2 = m02 * m33 - m32 * m03;
        let c1 = m02 * m23 - m22 * m03;
        let c0 = m02 * m13 - m12 * m03;

        let det = s0 * c5 - s1 * c4 + s2 * c3 + s3 * c2 - s4 * c1 + s5 * c0;

        if det.abs() < 1e-12 {
            return None;
        }

        let inv_det = 1.0 / det;

        // Adjugate matrix (transposed cofactor matrix), divided by determinant.
        // Result cols[i][j] = cofactor(j, i) / det
        let out = [
            [
                (m11 * c5 - m21 * c4 + m31 * c3) * inv_det,
                (-m01 * c5 + m21 * c2 - m31 * c1) * inv_det,
                (m01 * c4 - m11 * c2 + m31 * c0) * inv_det,
                (-m01 * c3 + m11 * c1 - m21 * c0) * inv_det,
            ],
            [
                (-m10 * c5 + m20 * c4 - m30 * c3) * inv_det,
                (m00 * c5 - m20 * c2 + m30 * c1) * inv_det,
                (-m00 * c4 + m10 * c2 - m30 * c0) * inv_det,
                (m00 * c3 - m10 * c1 + m20 * c0) * inv_det,
            ],
            [
                (m13 * s5 - m23 * s4 + m33 * s3) * inv_det,
                (-m03 * s5 + m23 * s2 - m33 * s1) * inv_det,
                (m03 * s4 - m13 * s2 + m33 * s0) * inv_det,
                (-m03 * s3 + m13 * s1 - m23 * s0) * inv_det,
            ],
            [
                (-m12 * s5 + m22 * s4 - m32 * s3) * inv_det,
                (m02 * s5 - m22 * s2 + m32 * s1) * inv_det,
                (-m02 * s4 + m12 * s2 - m32 * s0) * inv_det,
                (m02 * s3 - m12 * s1 + m22 * s0) * inv_det,
            ],
        ];

        Some(Mat4 { cols: out })
    }

    /// Returns the transpose of this matrix.
    #[inline]
    pub fn transpose(self) -> Mat4 {
        let m = &self.cols;
        Mat4 {
            cols: [
                [m[0][0], m[1][0], m[2][0], m[3][0]],
                [m[0][1], m[1][1], m[2][1], m[3][1]],
                [m[0][2], m[1][2], m[2][2], m[3][2]],
                [m[0][3], m[1][3], m[2][3], m[3][3]],
            ],
        }
    }

    /// Creates a translation matrix from a `Vec3`.
    #[inline]
    pub fn from_translation(t: Vec3) -> Mat4 {
        Mat4 {
            cols: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [t.x, t.y, t.z, 1.0],
            ],
        }
    }

    /// Creates a scale matrix from a `Vec3`.
    #[inline]
    pub fn from_scale(s: Vec3) -> Mat4 {
        Mat4 {
            cols: [
                [s.x, 0.0, 0.0, 0.0],
                [0.0, s.y, 0.0, 0.0],
                [0.0, 0.0, s.z, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    /// Creates a rotation matrix around the X axis.
    #[inline]
    pub fn from_rotation_x(angle: f32) -> Mat4 {
        let (s, c) = (angle.sin(), angle.cos());
        Mat4 {
            cols: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, c, s, 0.0],
                [0.0, -s, c, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    /// Creates a rotation matrix around the Y axis.
    #[inline]
    pub fn from_rotation_y(angle: f32) -> Mat4 {
        let (s, c) = (angle.sin(), angle.cos());
        Mat4 {
            cols: [
                [c, 0.0, -s, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [s, 0.0, c, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    /// Creates a rotation matrix around the Z axis.
    #[inline]
    pub fn from_rotation_z(angle: f32) -> Mat4 {
        let (s, c) = (angle.sin(), angle.cos());
        Mat4 {
            cols: [
                [c, s, 0.0, 0.0],
                [-s, c, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    /// Creates a rotation matrix from a unit quaternion.
    pub fn from_quat(q: Quat) -> Mat4 {
        let x2 = q.x + q.x;
        let y2 = q.y + q.y;
        let z2 = q.z + q.z;
        let xx = q.x * x2;
        let xy = q.x * y2;
        let xz = q.x * z2;
        let yy = q.y * y2;
        let yz = q.y * z2;
        let zz = q.z * z2;
        let wx = q.w * x2;
        let wy = q.w * y2;
        let wz = q.w * z2;

        Mat4 {
            cols: [
                [1.0 - (yy + zz), xy + wz, xz - wy, 0.0],
                [xy - wz, 1.0 - (xx + zz), yz + wx, 0.0],
                [xz + wy, yz - wx, 1.0 - (xx + yy), 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    /// Builds a right-handed look-at view matrix (OpenGL convention).
    ///
    /// `eye` is the camera position, `target` is the point being looked at,
    /// and `up` is the world-space up direction.
    pub fn look_at(eye: Vec3, target: Vec3, up: Vec3) -> Mat4 {
        let f = (target - eye).normalize();
        let s = f.cross(up).normalize();
        let u = s.cross(f);

        Mat4 {
            cols: [
                [s.x, u.x, -f.x, 0.0],
                [s.y, u.y, -f.y, 0.0],
                [s.z, u.z, -f.z, 0.0],
                [-s.dot(eye), -u.dot(eye), f.dot(eye), 1.0],
            ],
        }
    }

    /// Builds a right-handed perspective projection matrix with depth mapped
    /// to `[0, 1]` (Vulkan / WebGPU convention).
    ///
    /// * `fov_y_radians` -- vertical field-of-view in radians.
    /// * `aspect` -- width / height.
    /// * `near` / `far` -- near and far clip planes (positive distances).
    pub fn perspective(fov_y_radians: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
        let f = 1.0 / (fov_y_radians * 0.5).tan();
        let range_inv = 1.0 / (near - far);

        Mat4 {
            cols: [
                [f / aspect, 0.0, 0.0, 0.0],
                [0.0, f, 0.0, 0.0],
                [0.0, 0.0, far * range_inv, -1.0],
                [0.0, 0.0, near * far * range_inv, 0.0],
            ],
        }
    }

    /// Builds a right-handed orthographic projection matrix.
    pub fn orthographic(
        left: f32,
        right: f32,
        bottom: f32,
        top: f32,
        near: f32,
        far: f32,
    ) -> Mat4 {
        let rml = right - left;
        let tmb = top - bottom;
        let fmn = far - near;

        Mat4 {
            cols: [
                [2.0 / rml, 0.0, 0.0, 0.0],
                [0.0, 2.0 / tmb, 0.0, 0.0],
                [0.0, 0.0, -1.0 / fmn, 0.0],
                [
                    -(right + left) / rml,
                    -(top + bottom) / tmb,
                    -near / fmn,
                    1.0,
                ],
            ],
        }
    }
}

impl Mul<Mat4> for Mat4 {
    type Output = Mat4;

    #[inline]
    fn mul(self, rhs: Mat4) -> Mat4 {
        self.mul_mat4(rhs)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: check that two matrices are approximately equal.
    fn approx_eq_mat4(a: &Mat4, b: &Mat4, eps: f32) -> bool {
        for c in 0..4 {
            for r in 0..4 {
                if (a.cols[c][r] - b.cols[c][r]).abs() > eps {
                    return false;
                }
            }
        }
        true
    }

    /// Helper: check that two Vec4s are approximately equal.
    fn approx_eq_vec4(a: &Vec4, b: &Vec4, eps: f32) -> bool {
        (a.x - b.x).abs() <= eps
            && (a.y - b.y).abs() <= eps
            && (a.z - b.z).abs() <= eps
            && (a.w - b.w).abs() <= eps
    }

    #[test]
    fn identity_times_identity() {
        let id = Mat4::IDENTITY;
        assert_eq!(id * id, id);
    }

    #[test]
    fn determinant_of_identity() {
        assert!((Mat4::IDENTITY.determinant() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn inverse_times_original_is_identity() {
        let m = Mat4::from_cols(
            [2.0, 0.0, 0.0, 0.0],
            [0.0, 3.0, 0.0, 0.0],
            [0.0, 0.0, 4.0, 0.0],
            [1.0, 2.0, 3.0, 1.0],
        );
        let inv = m.inverse().expect("matrix should be invertible");
        let result = inv * m;
        assert!(
            approx_eq_mat4(&result, &Mat4::IDENTITY, 1e-5),
            "inv * m should be identity, got {:?}",
            result,
        );
    }

    #[test]
    fn from_translation_point() {
        let t = Mat4::from_translation(Vec3 {
            x: 3.0,
            y: 4.0,
            z: 5.0,
        });
        let p = Vec4 {
            x: 1.0,
            y: 2.0,
            z: 3.0,
            w: 1.0,
        };
        let result = t.mul_vec4(p);
        let expected = Vec4 {
            x: 4.0,
            y: 6.0,
            z: 8.0,
            w: 1.0,
        };
        assert!(
            approx_eq_vec4(&result, &expected, 1e-6),
            "expected {:?}, got {:?}",
            expected,
            result,
        );
    }

    #[test]
    fn from_rotation_z_90_deg() {
        let angle = core::f32::consts::FRAC_PI_2; // 90 degrees
        let rot = Mat4::from_rotation_z(angle);
        let v = Vec4 {
            x: 1.0,
            y: 0.0,
            z: 0.0,
            w: 1.0,
        };
        let result = rot.mul_vec4(v);
        let expected = Vec4 {
            x: 0.0,
            y: 1.0,
            z: 0.0,
            w: 1.0,
        };
        assert!(
            approx_eq_vec4(&result, &expected, 1e-6),
            "expected {:?}, got {:?}",
            expected,
            result,
        );
    }

    #[test]
    fn perspective_near_plane_mapping() {
        let fov = core::f32::consts::FRAC_PI_2; // 90 degrees
        let proj = Mat4::perspective(fov, 1.0, 0.1, 100.0);

        // A point at the center of the near plane.
        let p = Vec4 {
            x: 0.0,
            y: 0.0,
            z: -0.1,
            w: 1.0,
        };
        let clip = proj.mul_vec4(p);
        // After perspective divide, z should be 0 (near plane maps to 0).
        let ndc_z = clip.z / clip.w;
        assert!(
            ndc_z.abs() < 1e-5,
            "near plane should map to z=0, got ndc_z={}",
            ndc_z,
        );
    }

    #[test]
    fn orthographic_bounds_mapping() {
        let ortho = Mat4::orthographic(-1.0, 1.0, -1.0, 1.0, 0.0, 10.0);

        // Near-plane centre should map to (0, 0, 0).
        let near_center = Vec4 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 1.0,
        };
        let result = ortho.mul_vec4(near_center);
        assert!(
            approx_eq_vec4(
                &result,
                &Vec4 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    w: 1.0,
                },
                1e-6
            ),
            "near center mapped incorrectly: {:?}",
            result,
        );

        // Right-top-far corner should map to (1, 1, -1) in clip space.
        let corner = Vec4 {
            x: 1.0,
            y: 1.0,
            z: -10.0,
            w: 1.0,
        };
        let result2 = ortho.mul_vec4(corner);
        assert!(
            approx_eq_vec4(
                &result2,
                &Vec4 {
                    x: 1.0,
                    y: 1.0,
                    z: 1.0,
                    w: 1.0,
                },
                1e-5
            ),
            "far corner mapped incorrectly: {:?}",
            result2,
        );
    }

    #[test]
    fn multiply_associativity() {
        let a = Mat4::from_rotation_x(0.3);
        let b = Mat4::from_rotation_y(0.7);
        let c = Mat4::from_translation(Vec3 {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        });

        let ab_c = (a * b) * c;
        let a_bc = a * (b * c);
        assert!(
            approx_eq_mat4(&ab_c, &a_bc, 1e-5),
            "(A*B)*C != A*(B*C):\n  left  = {:?}\n  right = {:?}",
            ab_c,
            a_bc,
        );
    }

    #[test]
    fn transpose_of_transpose_is_original() {
        let m = Mat4::from_cols(
            [1.0, 2.0, 3.0, 4.0],
            [5.0, 6.0, 7.0, 8.0],
            [9.0, 10.0, 11.0, 12.0],
            [13.0, 14.0, 15.0, 16.0],
        );
        assert_eq!(m.transpose().transpose(), m);
    }

    #[test]
    fn look_at_basic() {
        let eye = Vec3 {
            x: 0.0,
            y: 0.0,
            z: 5.0,
        };
        let target = Vec3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        let up = Vec3 {
            x: 0.0,
            y: 1.0,
            z: 0.0,
        };
        let view = Mat4::look_at(eye, target, up);

        // The origin in world space should be at (0, 0, -5) in view space.
        let origin = Vec4 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 1.0,
        };
        let result = view.mul_vec4(origin);
        assert!(
            approx_eq_vec4(
                &result,
                &Vec4 {
                    x: 0.0,
                    y: 0.0,
                    z: -5.0,
                    w: 1.0,
                },
                1e-5
            ),
            "look_at: origin should map to (0,0,-5,1), got {:?}",
            result,
        );
    }
}
