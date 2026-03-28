//! Fixed-point Q16.16 number type.
//!
//! 16 bits integer, 16 bits fraction. Suitable for deterministic,
//! no_std game-engine math where floating-point reproducibility is a concern.

use core::fmt;
use core::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

/// Fractional scaling factor for Q16.16: 2^16 = 65536.
const FRAC_BITS: i32 = 16;
const SCALE: i32 = 1 << FRAC_BITS; // 65536

/// A fixed-point Q16.16 number.
///
/// The raw `bits` field stores the value multiplied by 65536.
/// For example, `1.0` is stored as `65536` and `0.5` as `32768`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Fixed {
    pub bits: i32,
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

impl Fixed {
    /// Zero (0.0).
    pub const ZERO: Fixed = Fixed { bits: 0 };

    /// One (1.0).
    pub const ONE: Fixed = Fixed { bits: SCALE };

    /// Negative one (-1.0).
    pub const NEG_ONE: Fixed = Fixed { bits: -SCALE };

    /// One half (0.5).
    pub const HALF: Fixed = Fixed { bits: SCALE / 2 }; // 32768

    /// Maximum representable value.
    pub const MAX: Fixed = Fixed { bits: i32::MAX };

    /// Minimum representable value.
    pub const MIN: Fixed = Fixed { bits: i32::MIN };

    /// Approximate pi (3.14159265...).
    /// 3.14159265 * 65536 ≈ 205887.
    pub const PI: Fixed = Fixed { bits: 205887 };

    /// Approximate 2*pi.
    /// 6.28318530 * 65536 ≈ 411775.
    pub const TWO_PI: Fixed = Fixed { bits: 411775 };

    /// Approximate pi/2.
    /// 1.57079632 * 65536 ≈ 102944.
    pub const HALF_PI: Fixed = Fixed { bits: 102944 };

    /// The smallest representable positive value (1 / 65536 ≈ 0.0000153).
    pub const EPSILON: Fixed = Fixed { bits: 1 };
}

// ---------------------------------------------------------------------------
// Constructors & conversions
// ---------------------------------------------------------------------------

impl Fixed {
    /// Create a `Fixed` from raw Q16.16 bits.
    #[inline]
    pub fn new(bits: i32) -> Self {
        Fixed { bits }
    }

    /// Create a `Fixed` from a whole integer value.
    #[inline]
    pub fn from_i32(v: i32) -> Self {
        Fixed { bits: v << FRAC_BITS }
    }

    /// Create a `Fixed` from an `f32`.
    #[inline]
    pub fn from_f32(v: f32) -> Self {
        Fixed {
            bits: (v * SCALE as f32) as i32,
        }
    }

    /// Convert to `f32`.
    #[inline]
    pub fn to_f32(self) -> f32 {
        self.bits as f32 / SCALE as f32
    }

    /// Convert to `i32`, truncating the fractional part toward zero.
    #[inline]
    pub fn to_i32(self) -> i32 {
        // Division truncates toward zero in Rust, which is what we want.
        self.bits / SCALE
    }
}

// ---------------------------------------------------------------------------
// Numeric helpers
// ---------------------------------------------------------------------------

impl Fixed {
    /// Absolute value.
    #[inline]
    pub fn abs(self) -> Self {
        Fixed {
            bits: self.bits.wrapping_abs(),
        }
    }

    /// Floor: largest integer value less than or equal to `self`.
    ///
    /// For non-negative values this simply clears the fractional bits.
    /// For negative values with a non-zero fractional part, the result
    /// is one integer step lower.
    #[inline]
    pub fn floor(self) -> Self {
        Fixed {
            bits: self.bits & !0xFFFF,
        }
    }

    /// Ceiling: smallest integer value greater than or equal to `self`.
    #[inline]
    pub fn ceil(self) -> Self {
        let frac = self.bits & 0xFFFF;
        if frac == 0 {
            self
        } else {
            Fixed {
                bits: (self.bits & !0xFFFF).wrapping_add(SCALE),
            }
        }
    }

    /// Fractional part: `self - self.floor()`.
    #[inline]
    pub fn fract(self) -> Self {
        Fixed {
            bits: self.bits - (self.bits & !0xFFFF),
        }
    }

    /// Square root via Newton-Raphson in integer arithmetic.
    ///
    /// Only valid for non-negative values; returns `ZERO` for negative inputs.
    ///
    /// We want `result` in Q16.16 such that `result / 2^16 = sqrt(self / 2^16)`.
    /// Therefore `result = sqrt(self.bits * 2^16)` (integer square root).
    pub fn sqrt(self) -> Fixed {
        if self.bits <= 0 {
            return Fixed::ZERO;
        }

        let n = (self.bits as u64) << 16;

        // Newton-Raphson for integer square root of n.
        let mut x = n;
        let mut y = (x + 1) / 2;
        while y < x {
            x = y;
            y = (x + n / x) / 2;
        }

        Fixed { bits: x as i32 }
    }

    /// Returns the smaller of `self` and `other`.
    #[inline]
    pub fn min(self, other: Self) -> Self {
        if self.bits <= other.bits {
            self
        } else {
            other
        }
    }

    /// Returns the larger of `self` and `other`.
    #[inline]
    pub fn max(self, other: Self) -> Self {
        if self.bits >= other.bits {
            self
        } else {
            other
        }
    }

    /// Clamp `self` to the range `[min, max]`.
    #[inline]
    pub fn clamp(self, min: Self, max: Self) -> Self {
        self.max(min).min(max)
    }

    /// Linear interpolation: `self + (other - self) * t`.
    #[inline]
    pub fn lerp(self, other: Self, t: Self) -> Self {
        self + (other - self) * t
    }
}

// ---------------------------------------------------------------------------
// Operator implementations
// ---------------------------------------------------------------------------

impl Add for Fixed {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self {
        Fixed {
            bits: self.bits.wrapping_add(rhs.bits),
        }
    }
}

impl Sub for Fixed {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Fixed {
            bits: self.bits.wrapping_sub(rhs.bits),
        }
    }
}

impl Mul for Fixed {
    type Output = Self;
    /// Multiply using i64 intermediate to avoid overflow.
    #[inline]
    fn mul(self, rhs: Self) -> Self {
        Fixed {
            bits: ((self.bits as i64 * rhs.bits as i64) >> FRAC_BITS) as i32,
        }
    }
}

impl Div for Fixed {
    type Output = Self;
    /// Divide using i64 intermediate to preserve precision.
    #[inline]
    fn div(self, rhs: Self) -> Self {
        Fixed {
            bits: (((self.bits as i64) << FRAC_BITS) / rhs.bits as i64) as i32,
        }
    }
}

impl Neg for Fixed {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self {
        Fixed {
            bits: self.bits.wrapping_neg(),
        }
    }
}

impl AddAssign for Fixed {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl SubAssign for Fixed {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl MulAssign for Fixed {
    #[inline]
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}

impl DivAssign for Fixed {
    #[inline]
    fn div_assign(&mut self, rhs: Self) {
        *self = *self / rhs;
    }
}

// ---------------------------------------------------------------------------
// Display
// ---------------------------------------------------------------------------

impl fmt::Display for Fixed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_f32())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: assert two f32 values are within `eps` of each other.
    fn assert_approx(a: f32, b: f32, eps: f32) {
        assert!(
            (a - b).abs() < eps,
            "expected {} ≈ {} (within {}), diff = {}",
            a,
            b,
            eps,
            (a - b).abs()
        );
    }

    // -- from_f32 / to_f32 round-trip --

    #[test]
    fn roundtrip_zero() {
        let f = Fixed::from_f32(0.0);
        assert_approx(f.to_f32(), 0.0, 1e-4);
    }

    #[test]
    fn roundtrip_one() {
        let f = Fixed::from_f32(1.0);
        assert_approx(f.to_f32(), 1.0, 1e-4);
    }

    #[test]
    fn roundtrip_neg_one() {
        let f = Fixed::from_f32(-1.0);
        assert_approx(f.to_f32(), -1.0, 1e-4);
    }

    #[test]
    fn roundtrip_half() {
        let f = Fixed::from_f32(0.5);
        assert_approx(f.to_f32(), 0.5, 1e-4);
    }

    #[test]
    fn roundtrip_quarter() {
        let f = Fixed::from_f32(0.25);
        assert_approx(f.to_f32(), 0.25, 1e-4);
    }

    #[test]
    fn roundtrip_hundred() {
        let f = Fixed::from_f32(100.0);
        assert_approx(f.to_f32(), 100.0, 1e-4);
    }

    #[test]
    fn roundtrip_neg_hundred() {
        let f = Fixed::from_f32(-100.0);
        assert_approx(f.to_f32(), -100.0, 1e-4);
    }

    // -- from_i32 / to_i32 --

    #[test]
    fn from_i32_to_i32() {
        assert_eq!(Fixed::from_i32(0).to_i32(), 0);
        assert_eq!(Fixed::from_i32(1).to_i32(), 1);
        assert_eq!(Fixed::from_i32(-1).to_i32(), -1);
        assert_eq!(Fixed::from_i32(42).to_i32(), 42);
        assert_eq!(Fixed::from_i32(-1000).to_i32(), -1000);
    }

    #[test]
    fn from_i32_bits() {
        assert_eq!(Fixed::from_i32(1).bits, 65536);
        assert_eq!(Fixed::from_i32(2).bits, 131072);
    }

    // -- Arithmetic --

    #[test]
    fn add_basic() {
        let a = Fixed::from_f32(1.5);
        let b = Fixed::from_f32(2.25);
        assert_approx((a + b).to_f32(), 3.75, 1e-4);
    }

    #[test]
    fn sub_basic() {
        let a = Fixed::from_f32(5.0);
        let b = Fixed::from_f32(3.25);
        assert_approx((a - b).to_f32(), 1.75, 1e-4);
    }

    #[test]
    fn mul_precision() {
        let a = Fixed::from_f32(1.5);
        let b = Fixed::from_f32(2.5);
        assert_approx((a * b).to_f32(), 3.75, 1e-4);
    }

    #[test]
    fn div_basic() {
        let a = Fixed::from_f32(10.0);
        let b = Fixed::from_f32(3.0);
        assert_approx((a / b).to_f32(), 3.3333, 1e-3);
    }

    #[test]
    fn arithmetic_matches_f32() {
        // Test a handful of representative values.
        let values: &[f32] = &[0.0, 1.0, -1.0, 0.5, -0.5, 7.75, -13.125, 100.0, -999.0, 42.42];
        for &a_f in values {
            for &b_f in values {
                let a = Fixed::from_f32(a_f);
                let b = Fixed::from_f32(b_f);

                assert_approx((a + b).to_f32(), a_f + b_f, 1e-2);
                assert_approx((a - b).to_f32(), a_f - b_f, 1e-2);

                // Mul: avoid overflow for large products.
                if (a_f * b_f).abs() < 30000.0 {
                    assert_approx((a * b).to_f32(), a_f * b_f, 1e-1);
                }

                // Div: skip division by zero.
                if b_f.abs() > 0.001 {
                    let expected = a_f / b_f;
                    if expected.abs() < 30000.0 {
                        assert_approx((a / b).to_f32(), expected, 1e-1);
                    }
                }
            }
        }
    }

    // -- sqrt --

    #[test]
    fn sqrt_four() {
        let v = Fixed::from_f32(4.0);
        assert_approx(v.sqrt().to_f32(), 2.0, 1e-4);
    }

    #[test]
    fn sqrt_two() {
        let v = Fixed::from_f32(2.0);
        assert_approx(v.sqrt().to_f32(), 1.4142, 1e-3);
    }

    #[test]
    fn sqrt_quarter() {
        let v = Fixed::from_f32(0.25);
        assert_approx(v.sqrt().to_f32(), 0.5, 1e-3);
    }

    #[test]
    fn sqrt_zero() {
        assert_eq!(Fixed::ZERO.sqrt(), Fixed::ZERO);
    }

    #[test]
    fn sqrt_negative() {
        let v = Fixed::from_f32(-4.0);
        assert_eq!(v.sqrt(), Fixed::ZERO);
    }

    #[test]
    fn sqrt_one() {
        assert_approx(Fixed::ONE.sqrt().to_f32(), 1.0, 1e-4);
    }

    // -- abs --

    #[test]
    fn abs_positive() {
        let v = Fixed::from_f32(3.5);
        assert_eq!(v.abs(), v);
    }

    #[test]
    fn abs_negative() {
        let v = Fixed::from_f32(-3.5);
        assert_approx(v.abs().to_f32(), 3.5, 1e-4);
    }

    #[test]
    fn abs_zero() {
        assert_eq!(Fixed::ZERO.abs(), Fixed::ZERO);
    }

    // -- floor --

    #[test]
    fn floor_positive_frac() {
        let v = Fixed::from_f32(3.7);
        assert_approx(v.floor().to_f32(), 3.0, 1e-4);
    }

    #[test]
    fn floor_negative_frac() {
        // floor(-3.7) should be -4.0
        let v = Fixed::from_f32(-3.7);
        assert_approx(v.floor().to_f32(), -4.0, 1e-4);
    }

    #[test]
    fn floor_whole() {
        let v = Fixed::from_f32(5.0);
        assert_approx(v.floor().to_f32(), 5.0, 1e-4);
    }

    // -- ceil --

    #[test]
    fn ceil_positive_frac() {
        let v = Fixed::from_f32(3.2);
        assert_approx(v.ceil().to_f32(), 4.0, 1e-4);
    }

    #[test]
    fn ceil_negative_frac() {
        // ceil(-3.2) should be -3.0
        let v = Fixed::from_f32(-3.2);
        assert_approx(v.ceil().to_f32(), -3.0, 1e-4);
    }

    #[test]
    fn ceil_whole() {
        let v = Fixed::from_f32(5.0);
        assert_approx(v.ceil().to_f32(), 5.0, 1e-4);
    }

    // -- fract --

    #[test]
    fn fract_positive() {
        let v = Fixed::from_f32(3.75);
        assert_approx(v.fract().to_f32(), 0.75, 1e-4);
    }

    #[test]
    fn fract_negative() {
        // fract(-3.75) = -3.75 - floor(-3.75) = -3.75 - (-4.0) = 0.25
        let v = Fixed::from_f32(-3.75);
        assert_approx(v.fract().to_f32(), 0.25, 1e-4);
    }

    // -- lerp --

    #[test]
    fn lerp_zero() {
        let a = Fixed::from_f32(0.0);
        let b = Fixed::from_f32(10.0);
        assert_approx(a.lerp(b, Fixed::ZERO).to_f32(), 0.0, 1e-4);
    }

    #[test]
    fn lerp_one() {
        let a = Fixed::from_f32(0.0);
        let b = Fixed::from_f32(10.0);
        assert_approx(a.lerp(b, Fixed::ONE).to_f32(), 10.0, 1e-4);
    }

    #[test]
    fn lerp_half() {
        let a = Fixed::from_f32(2.0);
        let b = Fixed::from_f32(8.0);
        assert_approx(a.lerp(b, Fixed::HALF).to_f32(), 5.0, 1e-4);
    }

    // -- min / max / clamp --

    #[test]
    fn min_max() {
        let a = Fixed::from_f32(3.0);
        let b = Fixed::from_f32(7.0);
        assert_eq!(a.min(b), a);
        assert_eq!(a.max(b), b);
    }

    #[test]
    fn clamp_within() {
        let v = Fixed::from_f32(5.0);
        let lo = Fixed::from_f32(2.0);
        let hi = Fixed::from_f32(8.0);
        assert_eq!(v.clamp(lo, hi), v);
    }

    #[test]
    fn clamp_below() {
        let v = Fixed::from_f32(1.0);
        let lo = Fixed::from_f32(2.0);
        let hi = Fixed::from_f32(8.0);
        assert_eq!(v.clamp(lo, hi), lo);
    }

    #[test]
    fn clamp_above() {
        let v = Fixed::from_f32(10.0);
        let lo = Fixed::from_f32(2.0);
        let hi = Fixed::from_f32(8.0);
        assert_eq!(v.clamp(lo, hi), hi);
    }

    // -- Display --

    #[test]
    fn display_one() {
        let s = format!("{}", Fixed::ONE);
        assert_eq!(s, "1");
    }

    #[test]
    fn display_half() {
        let s = format!("{}", Fixed::HALF);
        assert_eq!(s, "0.5");
    }

    // -- Neg --

    #[test]
    fn neg_one_is_neg_one() {
        assert_eq!(-Fixed::ONE, Fixed::NEG_ONE);
    }

    #[test]
    fn neg_zero() {
        assert_eq!(-Fixed::ZERO, Fixed::ZERO);
    }

    // -- Ordering --

    #[test]
    fn ordering() {
        assert!(Fixed::ONE > Fixed::ZERO);
        assert!(Fixed::ZERO > Fixed::NEG_ONE);
        assert!(Fixed::ONE > Fixed::NEG_ONE);
    }

    // -- Assign ops --

    #[test]
    fn add_assign() {
        let mut a = Fixed::from_f32(1.0);
        a += Fixed::from_f32(2.0);
        assert_approx(a.to_f32(), 3.0, 1e-4);
    }

    #[test]
    fn sub_assign() {
        let mut a = Fixed::from_f32(5.0);
        a -= Fixed::from_f32(2.0);
        assert_approx(a.to_f32(), 3.0, 1e-4);
    }

    #[test]
    fn mul_assign() {
        let mut a = Fixed::from_f32(3.0);
        a *= Fixed::from_f32(4.0);
        assert_approx(a.to_f32(), 12.0, 1e-4);
    }

    #[test]
    fn div_assign() {
        let mut a = Fixed::from_f32(12.0);
        a /= Fixed::from_f32(4.0);
        assert_approx(a.to_f32(), 3.0, 1e-4);
    }

    // -- Constants sanity --

    #[test]
    fn constants_values() {
        assert_eq!(Fixed::ZERO.bits, 0);
        assert_eq!(Fixed::ONE.bits, 65536);
        assert_eq!(Fixed::NEG_ONE.bits, -65536);
        assert_eq!(Fixed::HALF.bits, 32768);
        assert_eq!(Fixed::EPSILON.bits, 1);
        assert_eq!(Fixed::MAX.bits, i32::MAX);
        assert_eq!(Fixed::MIN.bits, i32::MIN);
    }

    #[test]
    fn pi_approximate() {
        assert_approx(Fixed::PI.to_f32(), core::f32::consts::PI, 1e-4);
    }

    #[test]
    fn two_pi_approximate() {
        assert_approx(Fixed::TWO_PI.to_f32(), 2.0 * core::f32::consts::PI, 1e-3);
    }

    #[test]
    fn half_pi_approximate() {
        assert_approx(
            Fixed::HALF_PI.to_f32(),
            core::f32::consts::PI / 2.0,
            1e-4,
        );
    }

    // -- to_i32 truncation toward zero --

    #[test]
    fn to_i32_truncates_positive() {
        let v = Fixed::from_f32(3.9);
        assert_eq!(v.to_i32(), 3);
    }

    #[test]
    fn to_i32_truncates_negative() {
        // -3.9 truncated toward zero should be -3
        let v = Fixed::from_f32(-3.9);
        assert_eq!(v.to_i32(), -3);
    }
}
