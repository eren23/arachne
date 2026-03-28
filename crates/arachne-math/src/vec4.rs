use crate::vec3::Vec3;
use core::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

/// A 4-component vector of `f32` values, used for homogeneous coordinates and color.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Vec4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Vec4 {
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        w: 0.0,
    };

    pub const ONE: Self = Self {
        x: 1.0,
        y: 1.0,
        z: 1.0,
        w: 1.0,
    };

    pub const W: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        w: 1.0,
    };

    #[inline]
    pub fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }

    #[inline]
    pub fn splat(v: f32) -> Self {
        Self {
            x: v,
            y: v,
            z: v,
            w: v,
        }
    }

    #[inline]
    pub fn dot(self, rhs: Self) -> f32 {
        self.x * rhs.x + self.y * rhs.y + self.z * rhs.z + self.w * rhs.w
    }

    #[inline]
    pub fn length_squared(self) -> f32 {
        self.dot(self)
    }

    #[inline]
    pub fn length(self) -> f32 {
        self.length_squared().sqrt()
    }

    /// Returns the normalized vector, or [`Vec4::ZERO`] if the length is zero.
    #[inline]
    pub fn normalize(self) -> Self {
        let len = self.length();
        if len == 0.0 {
            Self::ZERO
        } else {
            self / len
        }
    }

    /// Linearly interpolates between `self` and `rhs` by the factor `t`.
    #[inline]
    pub fn lerp(self, rhs: Self, t: f32) -> Self {
        Self {
            x: self.x + (rhs.x - self.x) * t,
            y: self.y + (rhs.y - self.y) * t,
            z: self.z + (rhs.z - self.z) * t,
            w: self.w + (rhs.w - self.w) * t,
        }
    }

    /// Returns the component-wise minimum of `self` and `other`.
    #[inline]
    pub fn min(self, other: Self) -> Self {
        Self {
            x: self.x.min(other.x),
            y: self.y.min(other.y),
            z: self.z.min(other.z),
            w: self.w.min(other.w),
        }
    }

    /// Returns the component-wise maximum of `self` and `other`.
    #[inline]
    pub fn max(self, other: Self) -> Self {
        Self {
            x: self.x.max(other.x),
            y: self.y.max(other.y),
            z: self.z.max(other.z),
            w: self.w.max(other.w),
        }
    }

    /// Returns a vector with the absolute value of each component.
    #[inline]
    pub fn abs(self) -> Self {
        Self {
            x: self.x.abs(),
            y: self.y.abs(),
            z: self.z.abs(),
            w: self.w.abs(),
        }
    }

    /// Truncates to a [`Vec3`], dropping the `w` component.
    #[inline]
    pub fn truncate(self) -> Vec3 {
        Vec3::new(self.x, self.y, self.z)
    }
}

// ---------------------------------------------------------------------------
// Operator implementations
// ---------------------------------------------------------------------------

impl Add for Vec4 {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
            z: self.z + rhs.z,
            w: self.w + rhs.w,
        }
    }
}

impl Sub for Vec4 {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
            z: self.z - rhs.z,
            w: self.w - rhs.w,
        }
    }
}

/// Scalar multiplication (`Vec4 * f32`).
impl Mul<f32> for Vec4 {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: f32) -> Self {
        Self {
            x: self.x * rhs,
            y: self.y * rhs,
            z: self.z * rhs,
            w: self.w * rhs,
        }
    }
}

/// Scalar division (`Vec4 / f32`).
impl Div<f32> for Vec4 {
    type Output = Self;

    #[inline]
    fn div(self, rhs: f32) -> Self {
        Self {
            x: self.x / rhs,
            y: self.y / rhs,
            z: self.z / rhs,
            w: self.w / rhs,
        }
    }
}

impl Neg for Vec4 {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self {
        Self {
            x: -self.x,
            y: -self.y,
            z: -self.z,
            w: -self.w,
        }
    }
}

impl AddAssign for Vec4 {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
        self.z += rhs.z;
        self.w += rhs.w;
    }
}

impl SubAssign for Vec4 {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.x -= rhs.x;
        self.y -= rhs.y;
        self.z -= rhs.z;
        self.w -= rhs.w;
    }
}

impl MulAssign<f32> for Vec4 {
    #[inline]
    fn mul_assign(&mut self, rhs: f32) {
        self.x *= rhs;
        self.y *= rhs;
        self.z *= rhs;
        self.w *= rhs;
    }
}

impl DivAssign<f32> for Vec4 {
    #[inline]
    fn div_assign(&mut self, rhs: f32) {
        self.x /= rhs;
        self.y /= rhs;
        self.z /= rhs;
        self.w /= rhs;
    }
}

/// Component-wise multiplication (`Vec4 * Vec4`).
impl Mul<Vec4> for Vec4 {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: Self) -> Self {
        Self {
            x: self.x * rhs.x,
            y: self.y * rhs.y,
            z: self.z * rhs.z,
            w: self.w * rhs.w,
        }
    }
}

/// Component-wise division (`Vec4 / Vec4`).
impl Div<Vec4> for Vec4 {
    type Output = Self;

    #[inline]
    fn div(self, rhs: Self) -> Self {
        Self {
            x: self.x / rhs.x,
            y: self.y / rhs.y,
            z: self.z / rhs.z,
            w: self.w / rhs.w,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-6;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < EPSILON
    }

    fn vec4_approx_eq(a: Vec4, b: Vec4) -> bool {
        approx_eq(a.x, b.x) && approx_eq(a.y, b.y) && approx_eq(a.z, b.z) && approx_eq(a.w, b.w)
    }

    // -- Arithmetic ---------------------------------------------------------

    #[test]
    fn add() {
        let a = Vec4::new(1.0, 2.0, 3.0, 4.0);
        let b = Vec4::new(5.0, 6.0, 7.0, 8.0);
        assert_eq!(a + b, Vec4::new(6.0, 8.0, 10.0, 12.0));
    }

    #[test]
    fn sub() {
        let a = Vec4::new(5.0, 6.0, 7.0, 8.0);
        let b = Vec4::new(1.0, 2.0, 3.0, 4.0);
        assert_eq!(a - b, Vec4::new(4.0, 4.0, 4.0, 4.0));
    }

    #[test]
    fn mul_scalar() {
        let a = Vec4::new(1.0, 2.0, 3.0, 4.0);
        assert_eq!(a * 2.0, Vec4::new(2.0, 4.0, 6.0, 8.0));
    }

    #[test]
    fn div_scalar() {
        let a = Vec4::new(2.0, 4.0, 6.0, 8.0);
        assert_eq!(a / 2.0, Vec4::new(1.0, 2.0, 3.0, 4.0));
    }

    #[test]
    fn neg() {
        let a = Vec4::new(1.0, -2.0, 3.0, -4.0);
        assert_eq!(-a, Vec4::new(-1.0, 2.0, -3.0, 4.0));
    }

    #[test]
    fn mul_component_wise() {
        let a = Vec4::new(1.0, 2.0, 3.0, 4.0);
        let b = Vec4::new(2.0, 3.0, 4.0, 5.0);
        assert_eq!(a * b, Vec4::new(2.0, 6.0, 12.0, 20.0));
    }

    #[test]
    fn div_component_wise() {
        let a = Vec4::new(4.0, 9.0, 16.0, 25.0);
        let b = Vec4::new(2.0, 3.0, 4.0, 5.0);
        assert_eq!(a / b, Vec4::new(2.0, 3.0, 4.0, 5.0));
    }

    #[test]
    fn add_assign() {
        let mut a = Vec4::new(1.0, 2.0, 3.0, 4.0);
        a += Vec4::new(5.0, 6.0, 7.0, 8.0);
        assert_eq!(a, Vec4::new(6.0, 8.0, 10.0, 12.0));
    }

    #[test]
    fn sub_assign() {
        let mut a = Vec4::new(5.0, 6.0, 7.0, 8.0);
        a -= Vec4::new(1.0, 2.0, 3.0, 4.0);
        assert_eq!(a, Vec4::new(4.0, 4.0, 4.0, 4.0));
    }

    #[test]
    fn mul_assign_scalar() {
        let mut a = Vec4::new(1.0, 2.0, 3.0, 4.0);
        a *= 3.0;
        assert_eq!(a, Vec4::new(3.0, 6.0, 9.0, 12.0));
    }

    #[test]
    fn div_assign_scalar() {
        let mut a = Vec4::new(3.0, 6.0, 9.0, 12.0);
        a /= 3.0;
        assert_eq!(a, Vec4::new(1.0, 2.0, 3.0, 4.0));
    }

    // -- Dot product --------------------------------------------------------

    #[test]
    fn dot_product() {
        let a = Vec4::new(1.0, 2.0, 3.0, 4.0);
        let b = Vec4::new(5.0, 6.0, 7.0, 8.0);
        // 1*5 + 2*6 + 3*7 + 4*8 = 5 + 12 + 21 + 32 = 70
        assert_eq!(a.dot(b), 70.0);
    }

    // -- Normalization ------------------------------------------------------

    #[test]
    fn normalize_nonzero() {
        let v = Vec4::new(3.0, 0.0, 0.0, 4.0);
        let n = v.normalize();
        assert!(approx_eq(n.length(), 1.0));
        assert!(approx_eq(n.x, 3.0 / 5.0));
        assert!(approx_eq(n.w, 4.0 / 5.0));
    }

    #[test]
    fn normalize_zero_returns_zero() {
        let n = Vec4::ZERO.normalize();
        assert_eq!(n, Vec4::ZERO);
    }

    // -- Lerp ---------------------------------------------------------------

    #[test]
    fn lerp_endpoints() {
        let a = Vec4::new(0.0, 0.0, 0.0, 0.0);
        let b = Vec4::new(10.0, 20.0, 30.0, 40.0);
        assert_eq!(a.lerp(b, 0.0), a);
        assert_eq!(a.lerp(b, 1.0), b);
    }

    #[test]
    fn lerp_midpoint() {
        let a = Vec4::new(0.0, 0.0, 0.0, 0.0);
        let b = Vec4::new(10.0, 20.0, 30.0, 40.0);
        let mid = a.lerp(b, 0.5);
        assert!(vec4_approx_eq(mid, Vec4::new(5.0, 10.0, 15.0, 20.0)));
    }

    // -- Truncate -----------------------------------------------------------

    #[test]
    fn truncate_to_vec3() {
        let v = Vec4::new(1.0, 2.0, 3.0, 99.0);
        let t = v.truncate();
        assert_eq!(t.x, 1.0);
        assert_eq!(t.y, 2.0);
        assert_eq!(t.z, 3.0);
    }

    // -- Misc ---------------------------------------------------------------

    #[test]
    fn length_and_length_squared() {
        let v = Vec4::new(1.0, 2.0, 3.0, 4.0);
        assert!(approx_eq(v.length_squared(), 30.0));
        assert!(approx_eq(v.length(), 30.0_f32.sqrt()));
    }

    #[test]
    fn splat() {
        assert_eq!(Vec4::splat(7.0), Vec4::new(7.0, 7.0, 7.0, 7.0));
    }

    #[test]
    fn min_max() {
        let a = Vec4::new(1.0, 5.0, 3.0, 7.0);
        let b = Vec4::new(4.0, 2.0, 6.0, 0.0);
        assert_eq!(a.min(b), Vec4::new(1.0, 2.0, 3.0, 0.0));
        assert_eq!(a.max(b), Vec4::new(4.0, 5.0, 6.0, 7.0));
    }

    #[test]
    fn abs_negative_components() {
        let v = Vec4::new(-1.0, 2.0, -3.0, 4.0);
        assert_eq!(v.abs(), Vec4::new(1.0, 2.0, 3.0, 4.0));
    }

    #[test]
    fn constants() {
        assert_eq!(Vec4::ZERO, Vec4::new(0.0, 0.0, 0.0, 0.0));
        assert_eq!(Vec4::ONE, Vec4::new(1.0, 1.0, 1.0, 1.0));
        assert_eq!(Vec4::W, Vec4::new(0.0, 0.0, 0.0, 1.0));
    }
}
