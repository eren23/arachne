use core::f32::consts::PI;

// LINEAR ------

#[inline]
pub fn linear(t: f32) -> f32 {
    t
}

// QUAD ------

#[inline]
pub fn ease_in_quad(t: f32) -> f32 {
    t * t
}

#[inline]
pub fn ease_out_quad(t: f32) -> f32 {
    let u = 1.0 - t;
    1.0 - u * u
}

#[inline]
pub fn ease_in_out_quad(t: f32) -> f32 {
    if t < 0.5 {
        2.0 * t * t
    } else {
        let u = -2.0 * t + 2.0;
        1.0 - u * u / 2.0
    }
}

// CUBIC ------

#[inline]
pub fn ease_in_cubic(t: f32) -> f32 {
    t * t * t
}

#[inline]
pub fn ease_out_cubic(t: f32) -> f32 {
    let u = 1.0 - t;
    1.0 - u * u * u
}

#[inline]
pub fn ease_in_out_cubic(t: f32) -> f32 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        let u = -2.0 * t + 2.0;
        1.0 - u * u * u / 2.0
    }
}

// QUART ------

#[inline]
pub fn ease_in_quart(t: f32) -> f32 {
    t * t * t * t
}

#[inline]
pub fn ease_out_quart(t: f32) -> f32 {
    let u = 1.0 - t;
    1.0 - u * u * u * u
}

#[inline]
pub fn ease_in_out_quart(t: f32) -> f32 {
    if t < 0.5 {
        8.0 * t * t * t * t
    } else {
        let u = -2.0 * t + 2.0;
        1.0 - u * u * u * u / 2.0
    }
}

// QUINT ------

#[inline]
pub fn ease_in_quint(t: f32) -> f32 {
    t * t * t * t * t
}

#[inline]
pub fn ease_out_quint(t: f32) -> f32 {
    let u = 1.0 - t;
    1.0 - u * u * u * u * u
}

#[inline]
pub fn ease_in_out_quint(t: f32) -> f32 {
    if t < 0.5 {
        16.0 * t * t * t * t * t
    } else {
        let u = -2.0 * t + 2.0;
        1.0 - u * u * u * u * u / 2.0
    }
}

// SINE ------

#[inline]
pub fn ease_in_sine(t: f32) -> f32 {
    1.0 - (t * PI / 2.0).cos()
}

#[inline]
pub fn ease_out_sine(t: f32) -> f32 {
    (t * PI / 2.0).sin()
}

#[inline]
pub fn ease_in_out_sine(t: f32) -> f32 {
    -(((t * PI).cos() - 1.0) / 2.0)
}

// EXPO ------

#[inline]
pub fn ease_in_expo(t: f32) -> f32 {
    if t == 0.0 {
        0.0
    } else {
        (2.0_f32).powf(10.0 * (t - 1.0))
    }
}

#[inline]
pub fn ease_out_expo(t: f32) -> f32 {
    if t == 1.0 {
        1.0
    } else {
        1.0 - (2.0_f32).powf(-10.0 * t)
    }
}

#[inline]
pub fn ease_in_out_expo(t: f32) -> f32 {
    if t == 0.0 {
        return 0.0;
    }
    if t == 1.0 {
        return 1.0;
    }
    if t < 0.5 {
        (2.0_f32).powf(20.0 * t - 10.0) / 2.0
    } else {
        (2.0 - (2.0_f32).powf(-20.0 * t + 10.0)) / 2.0
    }
}

// CIRC ------

#[inline]
pub fn ease_in_circ(t: f32) -> f32 {
    1.0 - (1.0 - t * t).sqrt()
}

#[inline]
pub fn ease_out_circ(t: f32) -> f32 {
    let u = t - 1.0;
    (1.0 - u * u).sqrt()
}

#[inline]
pub fn ease_in_out_circ(t: f32) -> f32 {
    if t < 0.5 {
        (1.0 - (1.0 - (2.0 * t) * (2.0 * t)).sqrt()) / 2.0
    } else {
        let u = -2.0 * t + 2.0;
        ((1.0 - u * u).sqrt() + 1.0) / 2.0
    }
}

// ELASTIC ------

#[inline]
pub fn ease_in_elastic(t: f32) -> f32 {
    if t == 0.0 {
        return 0.0;
    }
    if t == 1.0 {
        return 1.0;
    }
    let c4 = (2.0 * PI) / 3.0;
    -(2.0_f32).powf(10.0 * t - 10.0) * ((10.0 * t - 10.75) * c4).sin()
}

#[inline]
pub fn ease_out_elastic(t: f32) -> f32 {
    if t == 0.0 {
        return 0.0;
    }
    if t == 1.0 {
        return 1.0;
    }
    let c4 = (2.0 * PI) / 3.0;
    (2.0_f32).powf(-10.0 * t) * ((10.0 * t - 0.75) * c4).sin() + 1.0
}

#[inline]
pub fn ease_in_out_elastic(t: f32) -> f32 {
    if t == 0.0 {
        return 0.0;
    }
    if t == 1.0 {
        return 1.0;
    }
    let c5 = (2.0 * PI) / 4.5;
    if t < 0.5 {
        -((2.0_f32).powf(20.0 * t - 10.0) * ((20.0 * t - 11.125) * c5).sin()) / 2.0
    } else {
        ((2.0_f32).powf(-20.0 * t + 10.0) * ((20.0 * t - 11.125) * c5).sin()) / 2.0 + 1.0
    }
}

// BOUNCE ------

#[inline]
pub fn ease_out_bounce(t: f32) -> f32 {
    let n1: f32 = 7.5625;
    let d1: f32 = 2.75;
    if t < 1.0 / d1 {
        n1 * t * t
    } else if t < 2.0 / d1 {
        let t2 = t - 1.5 / d1;
        n1 * t2 * t2 + 0.75
    } else if t < 2.5 / d1 {
        let t2 = t - 2.25 / d1;
        n1 * t2 * t2 + 0.9375
    } else {
        let t2 = t - 2.625 / d1;
        n1 * t2 * t2 + 0.984375
    }
}

#[inline]
pub fn ease_in_bounce(t: f32) -> f32 {
    1.0 - ease_out_bounce(1.0 - t)
}

#[inline]
pub fn ease_in_out_bounce(t: f32) -> f32 {
    if t < 0.5 {
        (1.0 - ease_out_bounce(1.0 - 2.0 * t)) / 2.0
    } else {
        (1.0 + ease_out_bounce(2.0 * t - 1.0)) / 2.0
    }
}

// BACK ------

#[inline]
pub fn ease_in_back(t: f32) -> f32 {
    let c1: f32 = 1.70158;
    let c3 = c1 + 1.0;
    c3 * t * t * t - c1 * t * t
}

#[inline]
pub fn ease_out_back(t: f32) -> f32 {
    let c1: f32 = 1.70158;
    let c3 = c1 + 1.0;
    let u = t - 1.0;
    1.0 + c3 * u * u * u + c1 * u * u
}

#[inline]
pub fn ease_in_out_back(t: f32) -> f32 {
    let c1: f32 = 1.70158;
    let c2 = c1 * 1.525;
    if t < 0.5 {
        ((2.0 * t) * (2.0 * t) * ((c2 + 1.0) * 2.0 * t - c2)) / 2.0
    } else {
        let u = 2.0 * t - 2.0;
        (u * u * ((c2 + 1.0) * u + c2) + 2.0) / 2.0
    }
}

// EASING FUNCTION ENUM ------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EasingFunction {
    Linear,
    EaseInQuad,
    EaseOutQuad,
    EaseInOutQuad,
    EaseInCubic,
    EaseOutCubic,
    EaseInOutCubic,
    EaseInQuart,
    EaseOutQuart,
    EaseInOutQuart,
    EaseInQuint,
    EaseOutQuint,
    EaseInOutQuint,
    EaseInSine,
    EaseOutSine,
    EaseInOutSine,
    EaseInExpo,
    EaseOutExpo,
    EaseInOutExpo,
    EaseInCirc,
    EaseOutCirc,
    EaseInOutCirc,
    EaseInElastic,
    EaseOutElastic,
    EaseInOutElastic,
    EaseInBounce,
    EaseOutBounce,
    EaseInOutBounce,
    EaseInBack,
    EaseOutBack,
    EaseInOutBack,
}

impl EasingFunction {
    #[inline]
    pub fn apply(self, t: f32) -> f32 {
        match self {
            Self::Linear => linear(t),
            Self::EaseInQuad => ease_in_quad(t),
            Self::EaseOutQuad => ease_out_quad(t),
            Self::EaseInOutQuad => ease_in_out_quad(t),
            Self::EaseInCubic => ease_in_cubic(t),
            Self::EaseOutCubic => ease_out_cubic(t),
            Self::EaseInOutCubic => ease_in_out_cubic(t),
            Self::EaseInQuart => ease_in_quart(t),
            Self::EaseOutQuart => ease_out_quart(t),
            Self::EaseInOutQuart => ease_in_out_quart(t),
            Self::EaseInQuint => ease_in_quint(t),
            Self::EaseOutQuint => ease_out_quint(t),
            Self::EaseInOutQuint => ease_in_out_quint(t),
            Self::EaseInSine => ease_in_sine(t),
            Self::EaseOutSine => ease_out_sine(t),
            Self::EaseInOutSine => ease_in_out_sine(t),
            Self::EaseInExpo => ease_in_expo(t),
            Self::EaseOutExpo => ease_out_expo(t),
            Self::EaseInOutExpo => ease_in_out_expo(t),
            Self::EaseInCirc => ease_in_circ(t),
            Self::EaseOutCirc => ease_out_circ(t),
            Self::EaseInOutCirc => ease_in_out_circ(t),
            Self::EaseInElastic => ease_in_elastic(t),
            Self::EaseOutElastic => ease_out_elastic(t),
            Self::EaseInOutElastic => ease_in_out_elastic(t),
            Self::EaseInBounce => ease_in_bounce(t),
            Self::EaseOutBounce => ease_out_bounce(t),
            Self::EaseInOutBounce => ease_in_out_bounce(t),
            Self::EaseInBack => ease_in_back(t),
            Self::EaseOutBack => ease_out_back(t),
            Self::EaseInOutBack => ease_in_out_back(t),
        }
    }
}

// TESTS ------

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-6;

    fn assert_endpoints(f: fn(f32) -> f32, name: &str) {
        let v0 = f(0.0);
        let v1 = f(1.0);
        assert!(
            (v0 - 0.0).abs() < EPSILON,
            "{name}(0) = {v0}, expected 0"
        );
        assert!(
            (v1 - 1.0).abs() < EPSILON,
            "{name}(1) = {v1}, expected 1"
        );
    }

    #[test]
    fn all_easing_endpoints() {
        let functions: &[(fn(f32) -> f32, &str)] = &[
            (linear, "linear"),
            (ease_in_quad, "ease_in_quad"),
            (ease_out_quad, "ease_out_quad"),
            (ease_in_out_quad, "ease_in_out_quad"),
            (ease_in_cubic, "ease_in_cubic"),
            (ease_out_cubic, "ease_out_cubic"),
            (ease_in_out_cubic, "ease_in_out_cubic"),
            (ease_in_quart, "ease_in_quart"),
            (ease_out_quart, "ease_out_quart"),
            (ease_in_out_quart, "ease_in_out_quart"),
            (ease_in_quint, "ease_in_quint"),
            (ease_out_quint, "ease_out_quint"),
            (ease_in_out_quint, "ease_in_out_quint"),
            (ease_in_sine, "ease_in_sine"),
            (ease_out_sine, "ease_out_sine"),
            (ease_in_out_sine, "ease_in_out_sine"),
            (ease_in_expo, "ease_in_expo"),
            (ease_out_expo, "ease_out_expo"),
            (ease_in_out_expo, "ease_in_out_expo"),
            (ease_in_circ, "ease_in_circ"),
            (ease_out_circ, "ease_out_circ"),
            (ease_in_out_circ, "ease_in_out_circ"),
            (ease_in_elastic, "ease_in_elastic"),
            (ease_out_elastic, "ease_out_elastic"),
            (ease_in_out_elastic, "ease_in_out_elastic"),
            (ease_in_bounce, "ease_in_bounce"),
            (ease_out_bounce, "ease_out_bounce"),
            (ease_in_out_bounce, "ease_in_out_bounce"),
            (ease_in_back, "ease_in_back"),
            (ease_out_back, "ease_out_back"),
            (ease_in_out_back, "ease_in_out_back"),
        ];

        assert!(functions.len() >= 30, "need at least 30 easing functions, got {}", functions.len());

        for &(f, name) in functions {
            assert_endpoints(f, name);
        }
    }

    #[test]
    fn enum_apply_matches_function() {
        let t = 0.37;
        assert!((EasingFunction::Linear.apply(t) - linear(t)).abs() < EPSILON);
        assert!((EasingFunction::EaseInQuad.apply(t) - ease_in_quad(t)).abs() < EPSILON);
        assert!((EasingFunction::EaseOutBounce.apply(t) - ease_out_bounce(t)).abs() < EPSILON);
        assert!((EasingFunction::EaseInOutBack.apply(t) - ease_in_out_back(t)).abs() < EPSILON);
    }

    #[test]
    fn linear_is_identity() {
        for i in 0..=10 {
            let t = i as f32 / 10.0;
            assert!((linear(t) - t).abs() < EPSILON);
        }
    }

    #[test]
    fn ease_in_quad_midpoint() {
        let v = ease_in_quad(0.5);
        assert!((v - 0.25).abs() < EPSILON);
    }

    #[test]
    fn ease_out_quad_midpoint() {
        let v = ease_out_quad(0.5);
        assert!((v - 0.75).abs() < EPSILON);
    }

    #[test]
    fn ease_in_out_quad_midpoint() {
        let v = ease_in_out_quad(0.5);
        assert!((v - 0.5).abs() < EPSILON);
    }
}
