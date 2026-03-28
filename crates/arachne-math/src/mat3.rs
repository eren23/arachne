use core::ops::Mul;

use crate::vec2::Vec2;

/// A 3x3 column-major matrix of `f32` values.
///
/// Stored as 3 columns of 3 elements each. `cols[0]` is column 0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat3 {
    pub cols: [[f32; 3]; 3],
}

impl Mat3 {
    /// The 3x3 identity matrix.
    pub const IDENTITY: Self = Self {
        cols: [
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
        ],
    };

    /// The 3x3 zero matrix.
    pub const ZERO: Self = Self {
        cols: [
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
        ],
    };

    /// Constructs a `Mat3` from three column arrays.
    #[inline]
    pub fn from_cols(c0: [f32; 3], c1: [f32; 3], c2: [f32; 3]) -> Self {
        Self { cols: [c0, c1, c2] }
    }

    /// Matrix multiplication (`self * rhs`).
    ///
    /// Each element of the result is the dot product of a row of `self` with a
    /// column of `rhs`.
    #[inline]
    pub fn mul_mat3(self, rhs: Mat3) -> Mat3 {
        // self column-major: self.cols[col][row]
        // result.cols[j][i] = sum_k self.cols[k][i] * rhs.cols[j][k]
        let mut out = [[0.0f32; 3]; 3];
        for j in 0..3 {
            for i in 0..3 {
                out[j][i] = self.cols[0][i] * rhs.cols[j][0]
                    + self.cols[1][i] * rhs.cols[j][1]
                    + self.cols[2][i] * rhs.cols[j][2];
            }
        }
        Mat3 { cols: out }
    }

    /// Multiplies this matrix by a [`Vec2`], treating the vector as `(x, y, 1)`
    /// in homogeneous coordinates.
    ///
    /// Returns the first two components of the resulting 3-vector (affine
    /// transform -- the `w` component is not used for division).
    #[inline]
    pub fn mul_vec2(self, v: Vec2) -> Vec2 {
        let x = self.cols[0][0] * v.x + self.cols[1][0] * v.y + self.cols[2][0];
        let y = self.cols[0][1] * v.x + self.cols[1][1] * v.y + self.cols[2][1];
        Vec2 { x, y }
    }

    /// Computes the determinant of the matrix.
    #[inline]
    pub fn determinant(self) -> f32 {
        let [a, b, c] = self.cols[0];
        let [d, e, f] = self.cols[1];
        let [g, h, i] = self.cols[2];
        // Standard 3x3 determinant via cofactor expansion along the first column.
        a * (e * i - f * h) - d * (b * i - c * h) + g * (b * f - c * e)
    }

    /// Computes the inverse of the matrix, returning `None` if the determinant
    /// is approximately zero (absolute value < `1e-10`).
    #[inline]
    pub fn inverse(self) -> Option<Mat3> {
        let det = self.determinant();
        if det.abs() < 1e-10 {
            return None;
        }
        let inv_det = 1.0 / det;

        let [a, b, c] = self.cols[0];
        let [d, e, f] = self.cols[1];
        let [g, h, i] = self.cols[2];

        // Cofactor matrix (transposed, i.e. adjugate), then scaled by 1/det.
        // adjugate[col][row]
        let out = [
            [
                (e * i - f * h) * inv_det,
                (c * h - b * i) * inv_det,
                (b * f - c * e) * inv_det,
            ],
            [
                (f * g - d * i) * inv_det,
                (a * i - c * g) * inv_det,
                (c * d - a * f) * inv_det,
            ],
            [
                (d * h - e * g) * inv_det,
                (b * g - a * h) * inv_det,
                (a * e - b * d) * inv_det,
            ],
        ];

        Some(Mat3 { cols: out })
    }

    /// Returns the transpose of the matrix.
    #[inline]
    pub fn transpose(self) -> Mat3 {
        Mat3 {
            cols: [
                [self.cols[0][0], self.cols[1][0], self.cols[2][0]],
                [self.cols[0][1], self.cols[1][1], self.cols[2][1]],
                [self.cols[0][2], self.cols[1][2], self.cols[2][2]],
            ],
        }
    }

    /// Creates a 2D rotation matrix for the given angle (in radians).
    ///
    /// ```text
    /// [ cos  -sin  0 ]
    /// [ sin   cos  0 ]
    /// [  0     0   1 ]
    /// ```
    ///
    /// Stored column-major:
    /// - col 0: `[cos, sin, 0]`
    /// - col 1: `[-sin, cos, 0]`
    /// - col 2: `[0, 0, 1]`
    #[inline]
    pub fn from_rotation(angle: f32) -> Mat3 {
        let (sin, cos) = angle.sin_cos();
        Mat3 {
            cols: [
                [cos, sin, 0.0],
                [-sin, cos, 0.0],
                [0.0, 0.0, 1.0],
            ],
        }
    }

    /// Creates a 2D scale matrix from a [`Vec2`].
    ///
    /// ```text
    /// [ sx  0  0 ]
    /// [  0 sy  0 ]
    /// [  0  0  1 ]
    /// ```
    #[inline]
    pub fn from_scale(scale: Vec2) -> Mat3 {
        Mat3 {
            cols: [
                [scale.x, 0.0, 0.0],
                [0.0, scale.y, 0.0],
                [0.0, 0.0, 1.0],
            ],
        }
    }

    /// Creates a 2D translation matrix from a [`Vec2`] in homogeneous
    /// coordinates.
    ///
    /// ```text
    /// [ 1  0  tx ]
    /// [ 0  1  ty ]
    /// [ 0  0   1 ]
    /// ```
    #[inline]
    pub fn from_translation(translation: Vec2) -> Mat3 {
        Mat3 {
            cols: [
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [translation.x, translation.y, 1.0],
            ],
        }
    }
}

impl Mul<Mat3> for Mat3 {
    type Output = Mat3;

    #[inline]
    fn mul(self, rhs: Mat3) -> Mat3 {
        self.mul_mat3(rhs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: checks whether two matrices are approximately equal (per-element).
    fn mat3_approx_eq(a: &Mat3, b: &Mat3, eps: f32) -> bool {
        for c in 0..3 {
            for r in 0..3 {
                if (a.cols[c][r] - b.cols[c][r]).abs() > eps {
                    return false;
                }
            }
        }
        true
    }

    /// Helper: checks whether two `f32` values are approximately equal.
    fn approx_eq_f32(a: f32, b: f32, eps: f32) -> bool {
        (a - b).abs() <= eps
    }

    #[test]
    fn identity_times_identity_is_identity() {
        let result = Mat3::IDENTITY * Mat3::IDENTITY;
        assert_eq!(result, Mat3::IDENTITY);
    }

    #[test]
    fn determinant_of_identity_is_one() {
        assert!(approx_eq_f32(Mat3::IDENTITY.determinant(), 1.0, 1e-6));
    }

    #[test]
    fn inverse_of_identity_is_identity() {
        let inv = Mat3::IDENTITY.inverse().expect("identity should be invertible");
        assert!(mat3_approx_eq(&inv, &Mat3::IDENTITY, 1e-6));
    }

    #[test]
    fn inverse_times_original_is_identity() {
        let m = Mat3::from_cols(
            [2.0, 0.0, 1.0],
            [3.0, 1.0, 0.0],
            [1.0, 2.0, 3.0],
        );
        let inv = m.inverse().expect("matrix should be invertible");
        let product = inv * m;
        assert!(
            mat3_approx_eq(&product, &Mat3::IDENTITY, 1e-5),
            "inv * m should be ~identity, got {:?}",
            product
        );
    }

    #[test]
    fn from_rotation_90_degrees() {
        let angle = core::f32::consts::FRAC_PI_2; // 90 degrees
        let rot = Mat3::from_rotation(angle);
        let v = Vec2 { x: 1.0, y: 0.0 };
        let result = rot.mul_vec2(v);
        assert!(
            approx_eq_f32(result.x, 0.0, 1e-5) && approx_eq_f32(result.y, 1.0, 1e-5),
            "rotating (1,0) by 90 degrees should give ~(0,1), got ({}, {})",
            result.x,
            result.y
        );
    }

    #[test]
    fn multiply_associativity() {
        let a = Mat3::from_cols(
            [1.0, 2.0, 0.0],
            [0.0, 1.0, 3.0],
            [2.0, 0.0, 1.0],
        );
        let b = Mat3::from_cols(
            [0.0, 1.0, 2.0],
            [1.0, 0.0, 1.0],
            [1.0, 2.0, 0.0],
        );
        let c = Mat3::from_cols(
            [1.0, 0.0, 1.0],
            [2.0, 1.0, 0.0],
            [0.0, 1.0, 2.0],
        );

        let ab_c = (a * b) * c;
        let a_bc = a * (b * c);
        assert!(
            mat3_approx_eq(&ab_c, &a_bc, 1e-5),
            "(A*B)*C should equal A*(B*C)\nlhs: {:?}\nrhs: {:?}",
            ab_c,
            a_bc
        );
    }

    #[test]
    fn transpose_of_transpose_is_original() {
        let m = Mat3::from_cols(
            [1.0, 2.0, 3.0],
            [4.0, 5.0, 6.0],
            [7.0, 8.0, 9.0],
        );
        let tt = m.transpose().transpose();
        assert_eq!(tt, m);
    }

    #[test]
    fn determinant_of_known_matrix() {
        // Matrix (column-major):
        //   col0 = [1, 4, 7]  ->  row-major row0 = [1, 2, 3]
        //   col1 = [2, 5, 8]  ->  row-major row1 = [4, 5, 6]
        //   col2 = [3, 6, 0]  ->  row-major row2 = [7, 8, 0]
        //
        // det = 1*(5*0 - 6*8) - 2*(4*0 - 6*7) + 3*(4*8 - 5*7)
        //     = 1*(-48) - 2*(-42) + 3*(-3)
        //     = -48 + 84 - 9
        //     = 27
        //
        // But our determinant expands along the first *column* (not row).
        // With a=[1,4,7], d=[2,5,8], g=[3,6,0]:
        //   det = a*(e*i - f*h) - d*(b*i - c*h) + g*(b*f - c*e)
        //       = 1*(5*0 - 8*6) - 2*(4*0 - 7*6) + 3*(4*8 - 7*5)
        //       = 1*(-48) - 2*(-42) + 3*(-3)
        //       = -48 + 84 - 9
        //       = 27
        let m = Mat3::from_cols(
            [1.0, 4.0, 7.0],
            [2.0, 5.0, 8.0],
            [3.0, 6.0, 0.0],
        );
        assert!(
            approx_eq_f32(m.determinant(), 27.0, 1e-5),
            "determinant should be 27, got {}",
            m.determinant()
        );
    }
}
