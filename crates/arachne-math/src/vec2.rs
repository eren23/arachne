use core::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

/// A 2-dimensional vector with `f32` components.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    /// All zeros.
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };
    /// All ones.
    pub const ONE: Self = Self { x: 1.0, y: 1.0 };
    /// Unit vector along the X axis.
    pub const X: Self = Self { x: 1.0, y: 0.0 };
    /// Unit vector along the Y axis.
    pub const Y: Self = Self { x: 0.0, y: 1.0 };

    /// Creates a new [`Vec2`] from `x` and `y` components.
    #[inline]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Creates a new [`Vec2`] with both components set to `v`.
    #[inline]
    pub const fn splat(v: f32) -> Self {
        Self { x: v, y: v }
    }

    /// Returns the dot product of `self` and `rhs`.
    #[inline]
    pub fn dot(self, rhs: Self) -> f32 {
        self.x * rhs.x + self.y * rhs.y
    }

    /// Returns the 2D cross product (the z-component of the 3D cross product).
    ///
    /// This is equivalent to `self.x * rhs.y - self.y * rhs.x`.
    #[inline]
    pub fn cross(self, rhs: Self) -> f32 {
        self.x * rhs.y - self.y * rhs.x
    }

    /// Returns the length (magnitude) of the vector.
    #[inline]
    pub fn length(self) -> f32 {
        self.length_squared().sqrt()
    }

    /// Returns the squared length of the vector.
    ///
    /// This is cheaper than [`length`](Self::length) as it avoids a square root.
    #[inline]
    pub fn length_squared(self) -> f32 {
        self.dot(self)
    }

    /// Returns the vector normalized to unit length.
    ///
    /// If the vector is zero (or very close), returns [`Vec2::ZERO`].
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
    ///
    /// When `t == 0.0` the result is `self`, when `t == 1.0` the result is `rhs`.
    #[inline]
    pub fn lerp(self, rhs: Self, t: f32) -> Self {
        self * (1.0 - t) + rhs * t
    }

    /// Returns the angle (in radians) of the vector measured from the positive X axis.
    ///
    /// The result is in the range `(-PI, PI]`.
    #[inline]
    pub fn angle(self) -> f32 {
        self.y.atan2(self.x)
    }

    /// Rotates the vector by the given angle in radians.
    #[inline]
    pub fn rotate(self, angle: f32) -> Self {
        let (sin, cos) = angle.sin_cos();
        Self {
            x: self.x * cos - self.y * sin,
            y: self.x * sin + self.y * cos,
        }
    }

    /// Returns the Euclidean distance between `self` and `other`.
    #[inline]
    pub fn distance(self, other: Self) -> f32 {
        (self - other).length()
    }

    /// Returns a vector with the minimum of each component of `self` and `rhs`.
    #[inline]
    pub fn min(self, rhs: Self) -> Self {
        Self {
            x: self.x.min(rhs.x),
            y: self.y.min(rhs.y),
        }
    }

    /// Returns a vector with the maximum of each component of `self` and `rhs`.
    #[inline]
    pub fn max(self, rhs: Self) -> Self {
        Self {
            x: self.x.max(rhs.x),
            y: self.y.max(rhs.y),
        }
    }

    /// Returns a vector with the absolute value of each component.
    #[inline]
    pub fn abs(self) -> Self {
        Self {
            x: self.x.abs(),
            y: self.y.abs(),
        }
    }

    /// Returns a vector with each component rounded down to the nearest integer.
    #[inline]
    pub fn floor(self) -> Self {
        Self {
            x: self.x.floor(),
            y: self.y.floor(),
        }
    }

    /// Returns a vector with each component rounded up to the nearest integer.
    #[inline]
    pub fn ceil(self) -> Self {
        Self {
            x: self.x.ceil(),
            y: self.y.ceil(),
        }
    }

    /// Returns a vector with each component rounded to the nearest integer.
    #[inline]
    pub fn round(self) -> Self {
        Self {
            x: self.x.round(),
            y: self.y.round(),
        }
    }
}

// ---------------------------------------------------------------------------
// Operator implementations
// ---------------------------------------------------------------------------

impl Add for Vec2 {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl Sub for Vec2 {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl Mul<f32> for Vec2 {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: f32) -> Self {
        Self {
            x: self.x * rhs,
            y: self.y * rhs,
        }
    }
}

impl Mul<Vec2> for f32 {
    type Output = Vec2;
    #[inline]
    fn mul(self, rhs: Vec2) -> Vec2 {
        Vec2 {
            x: self * rhs.x,
            y: self * rhs.y,
        }
    }
}

impl Div<f32> for Vec2 {
    type Output = Self;
    #[inline]
    fn div(self, rhs: f32) -> Self {
        Self {
            x: self.x / rhs,
            y: self.y / rhs,
        }
    }
}

/// Component-wise multiplication.
impl Mul<Vec2> for Vec2 {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: Self) -> Self {
        Self {
            x: self.x * rhs.x,
            y: self.y * rhs.y,
        }
    }
}

/// Component-wise division.
impl Div<Vec2> for Vec2 {
    type Output = Self;
    #[inline]
    fn div(self, rhs: Self) -> Self {
        Self {
            x: self.x / rhs.x,
            y: self.y / rhs.y,
        }
    }
}

impl Neg for Vec2 {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self {
        Self {
            x: -self.x,
            y: -self.y,
        }
    }
}

impl AddAssign for Vec2 {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

impl SubAssign for Vec2 {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.x -= rhs.x;
        self.y -= rhs.y;
    }
}

impl MulAssign<f32> for Vec2 {
    #[inline]
    fn mul_assign(&mut self, rhs: f32) {
        self.x *= rhs;
        self.y *= rhs;
    }
}

impl DivAssign<f32> for Vec2 {
    #[inline]
    fn div_assign(&mut self, rhs: f32) {
        self.x /= rhs;
        self.y /= rhs;
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

    fn vec2_approx_eq(a: Vec2, b: Vec2) -> bool {
        approx_eq(a.x, b.x) && approx_eq(a.y, b.y)
    }

    // -- Constants --

    #[test]
    fn constants() {
        assert_eq!(Vec2::ZERO, Vec2::new(0.0, 0.0));
        assert_eq!(Vec2::ONE, Vec2::new(1.0, 1.0));
        assert_eq!(Vec2::X, Vec2::new(1.0, 0.0));
        assert_eq!(Vec2::Y, Vec2::new(0.0, 1.0));
    }

    #[test]
    fn default_is_zero() {
        assert_eq!(Vec2::default(), Vec2::ZERO);
    }

    // -- Construction --

    #[test]
    fn new_and_splat() {
        let v = Vec2::new(3.0, 4.0);
        assert_eq!(v.x, 3.0);
        assert_eq!(v.y, 4.0);

        let s = Vec2::splat(5.0);
        assert_eq!(s.x, 5.0);
        assert_eq!(s.y, 5.0);
    }

    // -- Arithmetic: Add, Sub, Mul, Div --

    #[test]
    fn add() {
        let a = Vec2::new(1.0, 2.0);
        let b = Vec2::new(3.0, 4.0);
        assert_eq!(a + b, Vec2::new(4.0, 6.0));
    }

    #[test]
    fn sub() {
        let a = Vec2::new(5.0, 7.0);
        let b = Vec2::new(2.0, 3.0);
        assert_eq!(a - b, Vec2::new(3.0, 4.0));
    }

    #[test]
    fn mul_scalar() {
        let v = Vec2::new(2.0, 3.0);
        assert_eq!(v * 2.0, Vec2::new(4.0, 6.0));
    }

    #[test]
    fn scalar_mul_vec() {
        let v = Vec2::new(2.0, 3.0);
        assert_eq!(2.0 * v, Vec2::new(4.0, 6.0));
    }

    #[test]
    fn div_scalar() {
        let v = Vec2::new(6.0, 8.0);
        assert_eq!(v / 2.0, Vec2::new(3.0, 4.0));
    }

    #[test]
    fn mul_component_wise() {
        let a = Vec2::new(2.0, 3.0);
        let b = Vec2::new(4.0, 5.0);
        assert_eq!(a * b, Vec2::new(8.0, 15.0));
    }

    #[test]
    fn div_component_wise() {
        let a = Vec2::new(10.0, 9.0);
        let b = Vec2::new(2.0, 3.0);
        assert_eq!(a / b, Vec2::new(5.0, 3.0));
    }

    #[test]
    fn neg() {
        let v = Vec2::new(1.0, -2.0);
        assert_eq!(-v, Vec2::new(-1.0, 2.0));
    }

    // -- Assign ops --

    #[test]
    fn add_assign() {
        let mut v = Vec2::new(1.0, 2.0);
        v += Vec2::new(3.0, 4.0);
        assert_eq!(v, Vec2::new(4.0, 6.0));
    }

    #[test]
    fn sub_assign() {
        let mut v = Vec2::new(5.0, 7.0);
        v -= Vec2::new(2.0, 3.0);
        assert_eq!(v, Vec2::new(3.0, 4.0));
    }

    #[test]
    fn mul_assign() {
        let mut v = Vec2::new(2.0, 3.0);
        v *= 3.0;
        assert_eq!(v, Vec2::new(6.0, 9.0));
    }

    #[test]
    fn div_assign() {
        let mut v = Vec2::new(6.0, 9.0);
        v /= 3.0;
        assert_eq!(v, Vec2::new(2.0, 3.0));
    }

    // -- Dot product --

    #[test]
    fn dot_product() {
        let a = Vec2::new(1.0, 2.0);
        let b = Vec2::new(3.0, 4.0);
        assert!(approx_eq(a.dot(b), 11.0)); // 1*3 + 2*4
    }

    #[test]
    fn dot_perpendicular() {
        assert!(approx_eq(Vec2::X.dot(Vec2::Y), 0.0));
    }

    // -- Cross product --

    #[test]
    fn cross_product() {
        let a = Vec2::new(1.0, 2.0);
        let b = Vec2::new(3.0, 4.0);
        // 1*4 - 2*3 = -2
        assert!(approx_eq(a.cross(b), -2.0));
    }

    #[test]
    fn cross_unit_vectors() {
        // X cross Y = 1 (right-handed)
        assert!(approx_eq(Vec2::X.cross(Vec2::Y), 1.0));
        // Y cross X = -1
        assert!(approx_eq(Vec2::Y.cross(Vec2::X), -1.0));
    }

    #[test]
    fn cross_parallel_is_zero() {
        let v = Vec2::new(3.0, 4.0);
        assert!(approx_eq(v.cross(v), 0.0));
    }

    // -- Length --

    #[test]
    fn length_3_4_5() {
        let v = Vec2::new(3.0, 4.0);
        assert!(approx_eq(v.length(), 5.0));
        assert!(approx_eq(v.length_squared(), 25.0));
    }

    #[test]
    fn length_zero() {
        assert!(approx_eq(Vec2::ZERO.length(), 0.0));
    }

    // -- Normalize --

    #[test]
    fn normalize_unit() {
        let v = Vec2::new(3.0, 4.0).normalize();
        assert!(approx_eq(v.length(), 1.0));
        assert!(approx_eq(v.x, 0.6));
        assert!(approx_eq(v.y, 0.8));
    }

    #[test]
    fn normalize_zero_vector_returns_zero() {
        let v = Vec2::ZERO.normalize();
        assert_eq!(v, Vec2::ZERO);
    }

    #[test]
    fn normalize_already_unit() {
        let v = Vec2::X.normalize();
        assert!(vec2_approx_eq(v, Vec2::X));
    }

    // -- Lerp --

    #[test]
    fn lerp_at_zero() {
        let a = Vec2::new(1.0, 2.0);
        let b = Vec2::new(5.0, 6.0);
        assert!(vec2_approx_eq(a.lerp(b, 0.0), a));
    }

    #[test]
    fn lerp_at_one() {
        let a = Vec2::new(1.0, 2.0);
        let b = Vec2::new(5.0, 6.0);
        assert!(vec2_approx_eq(a.lerp(b, 1.0), b));
    }

    #[test]
    fn lerp_at_half() {
        let a = Vec2::new(0.0, 0.0);
        let b = Vec2::new(10.0, 20.0);
        let mid = a.lerp(b, 0.5);
        assert!(vec2_approx_eq(mid, Vec2::new(5.0, 10.0)));
    }

    // -- Angle --

    #[test]
    fn angle_along_x() {
        assert!(approx_eq(Vec2::X.angle(), 0.0));
    }

    #[test]
    fn angle_along_y() {
        assert!(approx_eq(Vec2::Y.angle(), core::f32::consts::FRAC_PI_2));
    }

    // -- Rotate --

    #[test]
    fn rotate_90_degrees() {
        let v = Vec2::X;
        let rotated = v.rotate(core::f32::consts::FRAC_PI_2);
        assert!(vec2_approx_eq(rotated, Vec2::Y));
    }

    #[test]
    fn rotate_180_degrees() {
        let v = Vec2::new(1.0, 0.0);
        let rotated = v.rotate(core::f32::consts::PI);
        assert!(vec2_approx_eq(rotated, Vec2::new(-1.0, 0.0)));
    }

    #[test]
    fn rotate_360_degrees() {
        let v = Vec2::new(3.0, 4.0);
        let rotated = v.rotate(2.0 * core::f32::consts::PI);
        assert!(vec2_approx_eq(rotated, v));
    }

    #[test]
    fn rotate_arbitrary() {
        // Rotate (1, 0) by 45 degrees -> (sqrt(2)/2, sqrt(2)/2)
        let v = Vec2::X;
        let rotated = v.rotate(core::f32::consts::FRAC_PI_4);
        let expected = Vec2::splat(core::f32::consts::FRAC_1_SQRT_2);
        assert!(vec2_approx_eq(rotated, expected));
    }

    // -- Distance --

    #[test]
    fn distance_basic() {
        let a = Vec2::new(0.0, 0.0);
        let b = Vec2::new(3.0, 4.0);
        assert!(approx_eq(a.distance(b), 5.0));
    }

    #[test]
    fn distance_same_point() {
        let v = Vec2::new(7.0, 11.0);
        assert!(approx_eq(v.distance(v), 0.0));
    }

    #[test]
    fn distance_is_symmetric() {
        let a = Vec2::new(1.0, 2.0);
        let b = Vec2::new(4.0, 6.0);
        assert!(approx_eq(a.distance(b), b.distance(a)));
    }

    // -- Min / Max --

    #[test]
    fn min_max() {
        let a = Vec2::new(1.0, 5.0);
        let b = Vec2::new(3.0, 2.0);
        assert_eq!(a.min(b), Vec2::new(1.0, 2.0));
        assert_eq!(a.max(b), Vec2::new(3.0, 5.0));
    }

    // -- Abs / Floor / Ceil / Round --

    #[test]
    fn abs_negative() {
        let v = Vec2::new(-3.0, -4.0);
        assert_eq!(v.abs(), Vec2::new(3.0, 4.0));
    }

    #[test]
    fn floor_ceil_round() {
        let v = Vec2::new(1.7, -2.3);
        assert_eq!(v.floor(), Vec2::new(1.0, -3.0));
        assert_eq!(v.ceil(), Vec2::new(2.0, -2.0));
        assert_eq!(v.round(), Vec2::new(2.0, -2.0));
    }

    // -- Edge cases --

    #[test]
    fn very_large_vectors() {
        let big = 1e18_f32;
        let a = Vec2::new(big, big);
        let b = Vec2::new(big, big);
        let sum = a + b;
        assert_eq!(sum.x, 2.0 * big);
        assert_eq!(sum.y, 2.0 * big);
    }

    #[test]
    fn very_large_vector_length() {
        let big = 1e18_f32;
        let v = Vec2::new(big, 0.0);
        assert!(approx_eq(v.length(), big));
    }

    #[test]
    fn very_large_vector_normalize() {
        let big = 1e18_f32;
        let v = Vec2::new(big, 0.0).normalize();
        assert!(approx_eq(v.length(), 1.0));
    }
}
