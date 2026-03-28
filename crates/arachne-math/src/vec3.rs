//! Three-component vector type for game engine math.

use core::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

/// A 3D vector with `f32` components.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

impl Vec3 {
    /// The zero vector `(0, 0, 0)`.
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    /// The one vector `(1, 1, 1)`.
    pub const ONE: Self = Self {
        x: 1.0,
        y: 1.0,
        z: 1.0,
    };

    /// The unit X axis `(1, 0, 0)`.
    pub const X: Self = Self {
        x: 1.0,
        y: 0.0,
        z: 0.0,
    };

    /// The unit Y axis `(0, 1, 0)`.
    pub const Y: Self = Self {
        x: 0.0,
        y: 1.0,
        z: 0.0,
    };

    /// The unit Z axis `(0, 0, 1)`.
    pub const Z: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 1.0,
    };
}

// ---------------------------------------------------------------------------
// Constructors & methods
// ---------------------------------------------------------------------------

impl Vec3 {
    /// Creates a new `Vec3` from individual components.
    #[inline]
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    /// Creates a `Vec3` where all components are equal to `v`.
    #[inline]
    pub fn splat(v: f32) -> Self {
        Self { x: v, y: v, z: v }
    }

    /// Returns the dot product of `self` and `rhs`.
    #[inline]
    pub fn dot(self, rhs: Self) -> f32 {
        self.x * rhs.x + self.y * rhs.y + self.z * rhs.z
    }

    /// Returns the cross product of `self` and `rhs`.
    #[inline]
    pub fn cross(self, rhs: Self) -> Self {
        Self {
            x: self.y * rhs.z - self.z * rhs.y,
            y: self.z * rhs.x - self.x * rhs.z,
            z: self.x * rhs.y - self.y * rhs.x,
        }
    }

    /// Returns the squared length (magnitude) of the vector.
    #[inline]
    pub fn length_squared(self) -> f32 {
        self.dot(self)
    }

    /// Returns the length (magnitude) of the vector.
    #[inline]
    pub fn length(self) -> f32 {
        self.length_squared().sqrt()
    }

    /// Returns the normalized (unit-length) version of this vector.
    ///
    /// If the vector is zero-length, [`Vec3::ZERO`] is returned instead.
    #[inline]
    pub fn normalize(self) -> Self {
        let len = self.length();
        if len == 0.0 {
            Self::ZERO
        } else {
            self * (1.0 / len)
        }
    }

    /// Linearly interpolates between `self` and `rhs` by the factor `t`.
    ///
    /// When `t == 0.0` the result equals `self`, and when `t == 1.0` the
    /// result equals `rhs`.
    #[inline]
    pub fn lerp(self, rhs: Self, t: f32) -> Self {
        Self {
            x: self.x + (rhs.x - self.x) * t,
            y: self.y + (rhs.y - self.y) * t,
            z: self.z + (rhs.z - self.z) * t,
        }
    }

    /// Returns the Euclidean distance between `self` and `other`.
    #[inline]
    pub fn distance(self, other: Self) -> f32 {
        (self - other).length()
    }

    /// Returns a vector with the component-wise minimum of `self` and `rhs`.
    #[inline]
    pub fn min(self, rhs: Self) -> Self {
        Self {
            x: self.x.min(rhs.x),
            y: self.y.min(rhs.y),
            z: self.z.min(rhs.z),
        }
    }

    /// Returns a vector with the component-wise maximum of `self` and `rhs`.
    #[inline]
    pub fn max(self, rhs: Self) -> Self {
        Self {
            x: self.x.max(rhs.x),
            y: self.y.max(rhs.y),
            z: self.z.max(rhs.z),
        }
    }

    /// Returns a vector with the absolute value of each component.
    #[inline]
    pub fn abs(self) -> Self {
        Self {
            x: self.x.abs(),
            y: self.y.abs(),
            z: self.z.abs(),
        }
    }

    /// Returns a vector with each component rounded toward negative infinity.
    #[inline]
    pub fn floor(self) -> Self {
        Self {
            x: self.x.floor(),
            y: self.y.floor(),
            z: self.z.floor(),
        }
    }

    /// Returns a vector with each component rounded toward positive infinity.
    #[inline]
    pub fn ceil(self) -> Self {
        Self {
            x: self.x.ceil(),
            y: self.y.ceil(),
            z: self.z.ceil(),
        }
    }

    /// Returns a vector with each component rounded to the nearest integer.
    #[inline]
    pub fn round(self) -> Self {
        Self {
            x: self.x.round(),
            y: self.y.round(),
            z: self.z.round(),
        }
    }
}

// ---------------------------------------------------------------------------
// Operator overloads
// ---------------------------------------------------------------------------

impl Add for Vec3 {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
            z: self.z + rhs.z,
        }
    }
}

impl Sub for Vec3 {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
            z: self.z - rhs.z,
        }
    }
}

impl Mul<f32> for Vec3 {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: f32) -> Self {
        Self {
            x: self.x * rhs,
            y: self.y * rhs,
            z: self.z * rhs,
        }
    }
}

impl Mul<Vec3> for Vec3 {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: Vec3) -> Self {
        Self {
            x: self.x * rhs.x,
            y: self.y * rhs.y,
            z: self.z * rhs.z,
        }
    }
}

impl Div<f32> for Vec3 {
    type Output = Self;

    #[inline]
    fn div(self, rhs: f32) -> Self {
        Self {
            x: self.x / rhs,
            y: self.y / rhs,
            z: self.z / rhs,
        }
    }
}

impl Div<Vec3> for Vec3 {
    type Output = Self;

    #[inline]
    fn div(self, rhs: Vec3) -> Self {
        Self {
            x: self.x / rhs.x,
            y: self.y / rhs.y,
            z: self.z / rhs.z,
        }
    }
}

impl Neg for Vec3 {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self {
        Self {
            x: -self.x,
            y: -self.y,
            z: -self.z,
        }
    }
}

impl AddAssign for Vec3 {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
        self.z += rhs.z;
    }
}

impl SubAssign for Vec3 {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.x -= rhs.x;
        self.y -= rhs.y;
        self.z -= rhs.z;
    }
}

impl MulAssign<f32> for Vec3 {
    #[inline]
    fn mul_assign(&mut self, rhs: f32) {
        self.x *= rhs;
        self.y *= rhs;
        self.z *= rhs;
    }
}

impl DivAssign<f32> for Vec3 {
    #[inline]
    fn div_assign(&mut self, rhs: f32) {
        self.x /= rhs;
        self.y /= rhs;
        self.z /= rhs;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-6;

    /// Helper: asserts two `Vec3` values are approximately equal.
    fn assert_approx_eq(a: Vec3, b: Vec3) {
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

    // -- Arithmetic --------------------------------------------------------

    #[test]
    fn test_add() {
        let a = Vec3::new(1.0, 2.0, 3.0);
        let b = Vec3::new(4.0, 5.0, 6.0);
        assert_eq!(a + b, Vec3::new(5.0, 7.0, 9.0));
    }

    #[test]
    fn test_sub() {
        let a = Vec3::new(4.0, 5.0, 6.0);
        let b = Vec3::new(1.0, 2.0, 3.0);
        assert_eq!(a - b, Vec3::new(3.0, 3.0, 3.0));
    }

    #[test]
    fn test_mul_scalar() {
        let a = Vec3::new(1.0, 2.0, 3.0);
        assert_eq!(a * 2.0, Vec3::new(2.0, 4.0, 6.0));
    }

    #[test]
    fn test_mul_component_wise() {
        let a = Vec3::new(2.0, 3.0, 4.0);
        let b = Vec3::new(5.0, 6.0, 7.0);
        assert_eq!(a * b, Vec3::new(10.0, 18.0, 28.0));
    }

    #[test]
    fn test_div_scalar() {
        let a = Vec3::new(2.0, 4.0, 6.0);
        assert_eq!(a / 2.0, Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn test_div_component_wise() {
        let a = Vec3::new(10.0, 18.0, 28.0);
        let b = Vec3::new(5.0, 6.0, 7.0);
        assert_eq!(a / b, Vec3::new(2.0, 3.0, 4.0));
    }

    #[test]
    fn test_neg() {
        let a = Vec3::new(1.0, -2.0, 3.0);
        assert_eq!(-a, Vec3::new(-1.0, 2.0, -3.0));
    }

    #[test]
    fn test_add_assign() {
        let mut a = Vec3::new(1.0, 2.0, 3.0);
        a += Vec3::new(4.0, 5.0, 6.0);
        assert_eq!(a, Vec3::new(5.0, 7.0, 9.0));
    }

    #[test]
    fn test_sub_assign() {
        let mut a = Vec3::new(5.0, 7.0, 9.0);
        a -= Vec3::new(4.0, 5.0, 6.0);
        assert_eq!(a, Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn test_mul_assign() {
        let mut a = Vec3::new(1.0, 2.0, 3.0);
        a *= 3.0;
        assert_eq!(a, Vec3::new(3.0, 6.0, 9.0));
    }

    #[test]
    fn test_div_assign() {
        let mut a = Vec3::new(3.0, 6.0, 9.0);
        a /= 3.0;
        assert_eq!(a, Vec3::new(1.0, 2.0, 3.0));
    }

    // -- Dot product -------------------------------------------------------

    #[test]
    fn test_dot() {
        let a = Vec3::new(1.0, 2.0, 3.0);
        let b = Vec3::new(4.0, 5.0, 6.0);
        // 1*4 + 2*5 + 3*6 = 4 + 10 + 18 = 32
        assert_f32_approx(a.dot(b), 32.0);
    }

    #[test]
    fn test_dot_orthogonal() {
        assert_f32_approx(Vec3::X.dot(Vec3::Y), 0.0);
        assert_f32_approx(Vec3::Y.dot(Vec3::Z), 0.0);
        assert_f32_approx(Vec3::Z.dot(Vec3::X), 0.0);
    }

    // -- Cross product -----------------------------------------------------

    #[test]
    fn test_cross_xy_z() {
        assert_approx_eq(Vec3::X.cross(Vec3::Y), Vec3::Z);
    }

    #[test]
    fn test_cross_yz_x() {
        assert_approx_eq(Vec3::Y.cross(Vec3::Z), Vec3::X);
    }

    #[test]
    fn test_cross_zx_y() {
        assert_approx_eq(Vec3::Z.cross(Vec3::X), Vec3::Y);
    }

    #[test]
    fn test_cross_anticommutative() {
        let a = Vec3::new(1.0, 2.0, 3.0);
        let b = Vec3::new(4.0, 5.0, 6.0);
        assert_approx_eq(a.cross(b), -b.cross(a));
    }

    // -- Normalization -----------------------------------------------------

    #[test]
    fn test_normalize() {
        let v = Vec3::new(3.0, 0.0, 4.0);
        let n = v.normalize();
        assert_f32_approx(n.length(), 1.0);
        assert_approx_eq(n, Vec3::new(0.6, 0.0, 0.8));
    }

    #[test]
    fn test_normalize_zero_vector() {
        let n = Vec3::ZERO.normalize();
        assert_eq!(n, Vec3::ZERO);
    }

    #[test]
    fn test_normalize_unit_vectors() {
        assert_approx_eq(Vec3::X.normalize(), Vec3::X);
        assert_approx_eq(Vec3::Y.normalize(), Vec3::Y);
        assert_approx_eq(Vec3::Z.normalize(), Vec3::Z);
    }

    // -- Lerp --------------------------------------------------------------

    #[test]
    fn test_lerp_at_zero() {
        let a = Vec3::new(1.0, 2.0, 3.0);
        let b = Vec3::new(5.0, 6.0, 7.0);
        assert_approx_eq(a.lerp(b, 0.0), a);
    }

    #[test]
    fn test_lerp_at_one() {
        let a = Vec3::new(1.0, 2.0, 3.0);
        let b = Vec3::new(5.0, 6.0, 7.0);
        assert_approx_eq(a.lerp(b, 1.0), b);
    }

    #[test]
    fn test_lerp_at_half() {
        let a = Vec3::new(0.0, 0.0, 0.0);
        let b = Vec3::new(10.0, 20.0, 30.0);
        assert_approx_eq(a.lerp(b, 0.5), Vec3::new(5.0, 10.0, 15.0));
    }

    // -- Distance ----------------------------------------------------------

    #[test]
    fn test_distance() {
        let a = Vec3::new(1.0, 2.0, 3.0);
        let b = Vec3::new(4.0, 6.0, 3.0);
        // sqrt((3)^2 + (4)^2 + 0^2) = 5
        assert_f32_approx(a.distance(b), 5.0);
    }

    #[test]
    fn test_distance_to_self() {
        let a = Vec3::new(7.0, 8.0, 9.0);
        assert_f32_approx(a.distance(a), 0.0);
    }

    // -- Edge cases --------------------------------------------------------

    #[test]
    fn test_very_large_vectors() {
        let big = 1.0e18_f32;
        let a = Vec3::splat(big);
        let b = Vec3::splat(big);
        let sum = a + b;
        assert_eq!(sum, Vec3::splat(2.0e18));
    }

    #[test]
    fn test_very_large_length() {
        let big = 1.0e18_f32;
        let v = Vec3::new(big, 0.0, 0.0);
        assert_f32_approx(v.length(), big);
    }

    #[test]
    fn test_very_large_normalize() {
        let big = 1.0e18_f32;
        let v = Vec3::new(big, 0.0, 0.0);
        let n = v.normalize();
        assert_approx_eq(n, Vec3::X);
    }

    // -- Misc methods ------------------------------------------------------

    #[test]
    fn test_length_squared() {
        let v = Vec3::new(1.0, 2.0, 3.0);
        assert_f32_approx(v.length_squared(), 14.0);
    }

    #[test]
    fn test_length() {
        let v = Vec3::new(3.0, 4.0, 0.0);
        assert_f32_approx(v.length(), 5.0);
    }

    #[test]
    fn test_min() {
        let a = Vec3::new(1.0, 5.0, 3.0);
        let b = Vec3::new(4.0, 2.0, 6.0);
        assert_eq!(a.min(b), Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn test_max() {
        let a = Vec3::new(1.0, 5.0, 3.0);
        let b = Vec3::new(4.0, 2.0, 6.0);
        assert_eq!(a.max(b), Vec3::new(4.0, 5.0, 6.0));
    }

    #[test]
    fn test_abs() {
        let v = Vec3::new(-1.0, 2.0, -3.0);
        assert_eq!(v.abs(), Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn test_floor() {
        let v = Vec3::new(1.7, -2.3, 0.5);
        assert_eq!(v.floor(), Vec3::new(1.0, -3.0, 0.0));
    }

    #[test]
    fn test_ceil() {
        let v = Vec3::new(1.2, -2.7, 0.5);
        assert_eq!(v.ceil(), Vec3::new(2.0, -2.0, 1.0));
    }

    #[test]
    fn test_round() {
        let v = Vec3::new(1.4, 2.5, -3.6);
        assert_eq!(v.round(), Vec3::new(1.0, 3.0, -4.0));
    }

    #[test]
    fn test_splat() {
        assert_eq!(Vec3::splat(5.0), Vec3::new(5.0, 5.0, 5.0));
    }

    #[test]
    fn test_constants() {
        assert_eq!(Vec3::ZERO, Vec3::new(0.0, 0.0, 0.0));
        assert_eq!(Vec3::ONE, Vec3::new(1.0, 1.0, 1.0));
        assert_eq!(Vec3::X, Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(Vec3::Y, Vec3::new(0.0, 1.0, 0.0));
        assert_eq!(Vec3::Z, Vec3::new(0.0, 0.0, 1.0));
    }

    #[test]
    fn test_default_is_zero() {
        assert_eq!(Vec3::default(), Vec3::ZERO);
    }
}
