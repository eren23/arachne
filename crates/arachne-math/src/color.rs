//! RGBA color type for game engine math.

/// An RGBA color with `f32` components, each typically in the `[0, 1]` range.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

impl Color {
    /// Pure red `(1, 0, 0, 1)`.
    pub const RED: Self = Self {
        r: 1.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };

    /// Pure green `(0, 1, 0, 1)`.
    pub const GREEN: Self = Self {
        r: 0.0,
        g: 1.0,
        b: 0.0,
        a: 1.0,
    };

    /// Pure blue `(0, 0, 1, 1)`.
    pub const BLUE: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 1.0,
        a: 1.0,
    };

    /// White `(1, 1, 1, 1)`.
    pub const WHITE: Self = Self {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };

    /// Black `(0, 0, 0, 1)`.
    pub const BLACK: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };

    /// Fully transparent black `(0, 0, 0, 0)`.
    pub const TRANSPARENT: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };
}

// ---------------------------------------------------------------------------
// Constructors & conversions
// ---------------------------------------------------------------------------

impl Color {
    /// Creates a new color from individual components.
    #[inline]
    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Creates a new opaque color (alpha = 1.0).
    #[inline]
    pub fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    /// Creates a color from a `0xRRGGBB` hex value. Alpha is set to 1.0.
    #[inline]
    pub fn from_hex(hex: u32) -> Self {
        let r = ((hex >> 16) & 0xFF) as f32 / 255.0;
        let g = ((hex >> 8) & 0xFF) as f32 / 255.0;
        let b = (hex & 0xFF) as f32 / 255.0;
        Self { r, g, b, a: 1.0 }
    }

    /// Converts this color to a `0xRRGGBB` hex value (alpha is discarded).
    #[inline]
    pub fn to_hex(self) -> u32 {
        let r = (self.r * 255.0 + 0.5) as u32;
        let g = (self.g * 255.0 + 0.5) as u32;
        let b = (self.b * 255.0 + 0.5) as u32;
        (r << 16) | (g << 8) | b
    }

    /// Creates a color from a `0xRRGGBBAA` hex value.
    #[inline]
    pub fn from_hex_rgba(hex: u32) -> Self {
        let r = ((hex >> 24) & 0xFF) as f32 / 255.0;
        let g = ((hex >> 16) & 0xFF) as f32 / 255.0;
        let b = ((hex >> 8) & 0xFF) as f32 / 255.0;
        let a = (hex & 0xFF) as f32 / 255.0;
        Self { r, g, b, a }
    }

    /// Converts this color to a `0xRRGGBBAA` hex value.
    #[inline]
    pub fn to_hex_rgba(self) -> u32 {
        let r = (self.r * 255.0 + 0.5) as u32;
        let g = (self.g * 255.0 + 0.5) as u32;
        let b = (self.b * 255.0 + 0.5) as u32;
        let a = (self.a * 255.0 + 0.5) as u32;
        (r << 24) | (g << 16) | (b << 8) | a
    }

    /// Creates an opaque color from HSL values.
    ///
    /// * `h` - hue in degrees `[0, 360)`
    /// * `s` - saturation `[0, 1]`
    /// * `l` - lightness `[0, 1]`
    pub fn from_hsl(h: f32, s: f32, l: f32) -> Self {
        if s == 0.0 {
            return Self::rgb(l, l, l);
        }

        let q = if l < 0.5 {
            l * (1.0 + s)
        } else {
            l + s - l * s
        };
        let p = 2.0 * l - q;
        let h_norm = h / 360.0;

        let r = hue_to_rgb(p, q, h_norm + 1.0 / 3.0);
        let g = hue_to_rgb(p, q, h_norm);
        let b = hue_to_rgb(p, q, h_norm - 1.0 / 3.0);

        Self::rgb(r, g, b)
    }

    /// Converts this color to HSL, returning `(h, s, l)`.
    ///
    /// * `h` - hue in degrees `[0, 360)`
    /// * `s` - saturation `[0, 1]`
    /// * `l` - lightness `[0, 1]`
    pub fn to_hsl(self) -> (f32, f32, f32) {
        let max = self.r.max(self.g).max(self.b);
        let min = self.r.min(self.g).min(self.b);
        let l = (max + min) / 2.0;

        if (max - min).abs() < 1e-7 {
            return (0.0, 0.0, l);
        }

        let d = max - min;

        let s = if l > 0.5 {
            d / (2.0 - max - min)
        } else {
            d / (max + min)
        };

        let h = if (max - self.r).abs() < 1e-7 {
            let mut hh = (self.g - self.b) / d;
            if self.g < self.b {
                hh += 6.0;
            }
            hh
        } else if (max - self.g).abs() < 1e-7 {
            (self.b - self.r) / d + 2.0
        } else {
            (self.r - self.g) / d + 4.0
        };

        (h * 60.0, s, l)
    }
}

// ---------------------------------------------------------------------------
// Operations
// ---------------------------------------------------------------------------

impl Color {
    /// Linearly interpolates between `self` and `other` by `t` (component-wise).
    #[inline]
    pub fn lerp(self, other: Color, t: f32) -> Color {
        Color {
            r: self.r + (other.r - self.r) * t,
            g: self.g + (other.g - self.g) * t,
            b: self.b + (other.b - self.b) * t,
            a: self.a + (other.a - self.a) * t,
        }
    }

    /// Returns the premultiplied-alpha version of this color (r, g, b multiplied
    /// by a).
    #[inline]
    pub fn premultiply(self) -> Color {
        Color {
            r: self.r * self.a,
            g: self.g * self.a,
            b: self.b * self.a,
            a: self.a,
        }
    }

    /// Reverses premultiplied alpha by dividing r, g, b by a.
    ///
    /// If alpha is zero the color is returned unchanged to avoid division by
    /// zero.
    #[inline]
    pub fn unpremultiply(self) -> Color {
        if self.a > 0.0 {
            Color {
                r: self.r / self.a,
                g: self.g / self.a,
                b: self.b / self.a,
                a: self.a,
            }
        } else {
            self
        }
    }

    /// Returns the color as a four-element array `[r, g, b, a]`.
    #[inline]
    pub fn to_array(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }

    /// Creates a color from a four-element array `[r, g, b, a]`.
    #[inline]
    pub fn from_array(arr: [f32; 4]) -> Self {
        Self {
            r: arr[0],
            g: arr[1],
            b: arr[2],
            a: arr[3],
        }
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// HSL helper: converts a single hue sector to an RGB channel value.
fn hue_to_rgb(p: f32, q: f32, mut t: f32) -> f32 {
    if t < 0.0 {
        t += 1.0;
    }
    if t > 1.0 {
        t -= 1.0;
    }

    if t < 1.0 / 6.0 {
        return p + (q - p) * 6.0 * t;
    }
    if t < 1.0 / 2.0 {
        return q;
    }
    if t < 2.0 / 3.0 {
        return p + (q - p) * (2.0 / 3.0 - t) * 6.0;
    }
    p
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: approximate equality for a Color.
    fn approx_eq(a: Color, b: Color, eps: f32) -> bool {
        (a.r - b.r).abs() < eps
            && (a.g - b.g).abs() < eps
            && (a.b - b.b).abs() < eps
            && (a.a - b.a).abs() < eps
    }

    // ----- Constants -----

    #[test]
    fn constant_red() {
        assert_eq!(Color::RED, Color::new(1.0, 0.0, 0.0, 1.0));
    }

    #[test]
    fn constant_green() {
        assert_eq!(Color::GREEN, Color::new(0.0, 1.0, 0.0, 1.0));
    }

    #[test]
    fn constant_blue() {
        assert_eq!(Color::BLUE, Color::new(0.0, 0.0, 1.0, 1.0));
    }

    #[test]
    fn constant_white() {
        assert_eq!(Color::WHITE, Color::new(1.0, 1.0, 1.0, 1.0));
    }

    #[test]
    fn constant_black() {
        assert_eq!(Color::BLACK, Color::new(0.0, 0.0, 0.0, 1.0));
    }

    #[test]
    fn constant_transparent() {
        assert_eq!(Color::TRANSPARENT, Color::new(0.0, 0.0, 0.0, 0.0));
    }

    // ----- from_hex / to_hex roundtrip -----

    #[test]
    fn hex_roundtrip_red() {
        let c = Color::from_hex(0xFF0000);
        assert!(approx_eq(c, Color::RED, 1e-3));
        assert_eq!(c.to_hex(), 0xFF0000);
    }

    #[test]
    fn hex_roundtrip_green() {
        let c = Color::from_hex(0x00FF00);
        assert!(approx_eq(c, Color::GREEN, 1e-3));
        assert_eq!(c.to_hex(), 0x00FF00);
    }

    #[test]
    fn hex_roundtrip_blue() {
        let c = Color::from_hex(0x0000FF);
        assert!(approx_eq(c, Color::BLUE, 1e-3));
        assert_eq!(c.to_hex(), 0x0000FF);
    }

    // ----- from_hex_rgba / to_hex_rgba roundtrip -----

    #[test]
    fn hex_rgba_roundtrip() {
        let hex: u32 = 0xFF8040CC;
        let c = Color::from_hex_rgba(hex);
        assert_eq!(c.to_hex_rgba(), hex);
    }

    #[test]
    fn hex_rgba_roundtrip_transparent() {
        let hex: u32 = 0x00000000;
        let c = Color::from_hex_rgba(hex);
        assert!(approx_eq(c, Color::TRANSPARENT, 1e-3));
        assert_eq!(c.to_hex_rgba(), hex);
    }

    // ----- from_hsl / to_hsl roundtrip -----

    fn assert_hsl_roundtrip(color: Color, expected_h: f32, expected_s: f32, expected_l: f32) {
        let (h, s, l) = color.to_hsl();
        assert!(
            (h - expected_h).abs() < 1e-3,
            "h: expected {}, got {}",
            expected_h,
            h,
        );
        assert!(
            (s - expected_s).abs() < 1e-3,
            "s: expected {}, got {}",
            expected_s,
            s,
        );
        assert!(
            (l - expected_l).abs() < 1e-3,
            "l: expected {}, got {}",
            expected_l,
            l,
        );

        let reconstructed = Color::from_hsl(h, s, l);
        assert!(
            approx_eq(reconstructed, color, 1e-3),
            "roundtrip failed: {:?} -> ({}, {}, {}) -> {:?}",
            color,
            h,
            s,
            l,
            reconstructed,
        );
    }

    #[test]
    fn hsl_roundtrip_red() {
        assert_hsl_roundtrip(Color::RED, 0.0, 1.0, 0.5);
    }

    #[test]
    fn hsl_roundtrip_green() {
        assert_hsl_roundtrip(Color::GREEN, 120.0, 1.0, 0.5);
    }

    #[test]
    fn hsl_roundtrip_blue() {
        assert_hsl_roundtrip(Color::BLUE, 240.0, 1.0, 0.5);
    }

    #[test]
    fn hsl_roundtrip_white() {
        assert_hsl_roundtrip(Color::WHITE, 0.0, 0.0, 1.0);
    }

    // ----- lerp -----

    #[test]
    fn lerp_black_to_white_midpoint() {
        let gray = Color::BLACK.lerp(Color::WHITE, 0.5);
        assert!(approx_eq(
            gray,
            Color::new(0.5, 0.5, 0.5, 1.0),
            1e-6,
        ));
    }

    #[test]
    fn lerp_endpoints() {
        let c = Color::RED.lerp(Color::BLUE, 0.0);
        assert_eq!(c, Color::RED);

        let c = Color::RED.lerp(Color::BLUE, 1.0);
        assert_eq!(c, Color::BLUE);
    }

    // ----- premultiply / unpremultiply -----

    #[test]
    fn premultiply_half_alpha() {
        let c = Color::new(1.0, 0.8, 0.6, 0.5);
        let pm = c.premultiply();
        assert!(approx_eq(
            pm,
            Color::new(0.5, 0.4, 0.3, 0.5),
            1e-6,
        ));
    }

    #[test]
    fn unpremultiply_reverses_premultiply() {
        let original = Color::new(0.8, 0.6, 0.4, 0.5);
        let roundtripped = original.premultiply().unpremultiply();
        assert!(approx_eq(roundtripped, original, 1e-6));
    }

    #[test]
    fn unpremultiply_zero_alpha_returns_self() {
        let c = Color::new(0.0, 0.0, 0.0, 0.0);
        assert_eq!(c.unpremultiply(), c);
    }

    // ----- to_array / from_array -----

    #[test]
    fn array_roundtrip() {
        let c = Color::new(0.1, 0.2, 0.3, 0.4);
        let arr = c.to_array();
        assert_eq!(arr, [0.1, 0.2, 0.3, 0.4]);
        assert_eq!(Color::from_array(arr), c);
    }
}
