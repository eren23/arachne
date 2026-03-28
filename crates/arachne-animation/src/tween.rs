use arachne_math::{Vec3, Quat, Transform};

// LERP TRAIT ------

pub trait Lerp: Clone {
    fn lerp(&self, other: &Self, t: f32) -> Self;
}

impl Lerp for f32 {
    #[inline]
    fn lerp(&self, other: &Self, t: f32) -> Self {
        self + (other - self) * t
    }
}

impl Lerp for Vec3 {
    #[inline]
    fn lerp(&self, other: &Self, t: f32) -> Self {
        Vec3::lerp(*self, *other, t)
    }
}

impl Lerp for Quat {
    #[inline]
    fn lerp(&self, other: &Self, t: f32) -> Self {
        self.slerp(*other, t)
    }
}

impl Lerp for Transform {
    #[inline]
    fn lerp(&self, other: &Self, t: f32) -> Self {
        Transform {
            position: Vec3::lerp(self.position, other.position, t),
            rotation: self.rotation.slerp(other.rotation, t),
            scale: Vec3::lerp(self.scale, other.scale, t),
        }
    }
}

// TWEEN STATE ------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TweenState {
    Playing,
    Paused,
    Completed,
}

// LOOP MODE ------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoopMode {
    Once,
    Loop,
    PingPong,
}

// TWEEN ------

#[derive(Clone)]
pub struct Tween<T: Lerp> {
    pub start: T,
    pub end: T,
    pub duration: f32,
    pub elapsed: f32,
    pub easing: fn(f32) -> f32,
    pub state: TweenState,
    pub loop_mode: LoopMode,
    pub direction: f32,
}

impl<T: Lerp> Tween<T> {
    #[inline]
    pub fn new(start: T, end: T, duration: f32, easing: fn(f32) -> f32) -> Self {
        Self {
            start,
            end,
            duration,
            elapsed: 0.0,
            easing,
            state: TweenState::Playing,
            loop_mode: LoopMode::Once,
            direction: 1.0,
        }
    }

    pub fn update(&mut self, dt: f32) {
        if self.state != TweenState::Playing {
            return;
        }

        self.elapsed += dt * self.direction;

        match self.loop_mode {
            LoopMode::Once => {
                if self.elapsed >= self.duration {
                    self.elapsed = self.duration;
                    self.state = TweenState::Completed;
                } else if self.elapsed < 0.0 {
                    self.elapsed = 0.0;
                    self.state = TweenState::Completed;
                }
            }
            LoopMode::Loop => {
                while self.elapsed >= self.duration {
                    self.elapsed -= self.duration;
                }
                while self.elapsed < 0.0 {
                    self.elapsed += self.duration;
                }
            }
            LoopMode::PingPong => {
                if self.elapsed >= self.duration {
                    self.elapsed = self.duration - (self.elapsed - self.duration);
                    self.direction = -self.direction;
                } else if self.elapsed < 0.0 {
                    self.elapsed = -self.elapsed;
                    self.direction = -self.direction;
                }
            }
        }
    }

    #[inline]
    pub fn value(&self) -> T {
        let raw_t = if self.duration > 0.0 {
            (self.elapsed / self.duration).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let eased_t = (self.easing)(raw_t);
        self.start.lerp(&self.end, eased_t)
    }

    #[inline]
    pub fn pause(&mut self) {
        if self.state == TweenState::Playing {
            self.state = TweenState::Paused;
        }
    }

    #[inline]
    pub fn resume(&mut self) {
        if self.state == TweenState::Paused {
            self.state = TweenState::Playing;
        }
    }

    #[inline]
    pub fn reset(&mut self) {
        self.elapsed = 0.0;
        self.direction = 1.0;
        self.state = TweenState::Playing;
    }
}

// TWEEN SEQUENCE ------

#[derive(Clone)]
pub struct TweenSequence<T: Lerp> {
    pub tweens: Vec<Tween<T>>,
    pub current_index: usize,
}

impl<T: Lerp> TweenSequence<T> {
    #[inline]
    pub fn new(tweens: Vec<Tween<T>>) -> Self {
        Self {
            tweens,
            current_index: 0,
        }
    }

    pub fn update(&mut self, dt: f32) {
        if self.current_index >= self.tweens.len() {
            return;
        }

        let mut remaining_dt = dt;
        while remaining_dt > 0.0 && self.current_index < self.tweens.len() {
            let tween = &mut self.tweens[self.current_index];
            let time_left = tween.duration - tween.elapsed;

            if remaining_dt >= time_left {
                tween.elapsed = tween.duration;
                tween.state = TweenState::Completed;
                remaining_dt -= time_left;
                self.current_index += 1;
            } else {
                tween.update(remaining_dt);
                remaining_dt = 0.0;
            }
        }
    }

    #[inline]
    pub fn value(&self) -> T {
        let idx = if self.current_index >= self.tweens.len() {
            self.tweens.len() - 1
        } else {
            self.current_index
        };
        self.tweens[idx].value()
    }
}

// TWEEN PARALLEL ------

#[derive(Clone)]
pub struct TweenParallel<T: Lerp> {
    pub tweens: Vec<Tween<T>>,
}

impl<T: Lerp> TweenParallel<T> {
    #[inline]
    pub fn new(tweens: Vec<Tween<T>>) -> Self {
        Self { tweens }
    }

    pub fn update(&mut self, dt: f32) {
        for tween in &mut self.tweens {
            tween.update(dt);
        }
    }

    #[inline]
    pub fn value(&self) -> T {
        self.tweens[0].value()
    }
}

// TESTS ------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::easing;

    const EPSILON: f32 = 1e-5;

    #[test]
    fn f32_tween_midpoint() {
        let mut tw = Tween::new(0.0_f32, 100.0, 1.0, easing::linear);
        tw.update(0.5);
        let v = tw.value();
        assert!(
            (v - 50.0).abs() < EPSILON,
            "expected ~50 at t=0.5, got {v}"
        );
    }

    #[test]
    fn f32_tween_completes() {
        let mut tw = Tween::new(0.0_f32, 100.0, 1.0, easing::linear);
        tw.update(1.5);
        assert_eq!(tw.state, TweenState::Completed);
        let v = tw.value();
        assert!(
            (v - 100.0).abs() < EPSILON,
            "expected 100 after completion, got {v}"
        );
    }

    #[test]
    fn loop_mode_restarts() {
        let mut tw = Tween::new(0.0_f32, 100.0, 1.0, easing::linear);
        tw.loop_mode = LoopMode::Loop;
        tw.update(1.5);
        assert_eq!(tw.state, TweenState::Playing);
        let v = tw.value();
        assert!(
            (v - 50.0).abs() < EPSILON,
            "expected ~50 after loop wrap, got {v}"
        );
    }

    #[test]
    fn pingpong_reverses() {
        let mut tw = Tween::new(0.0_f32, 100.0, 1.0, easing::linear);
        tw.loop_mode = LoopMode::PingPong;
        tw.update(1.25);
        assert_eq!(tw.state, TweenState::Playing);
        let v = tw.value();
        assert!(
            (v - 75.0).abs() < EPSILON,
            "expected ~75 after pingpong reverse at 1.25s, got {v}"
        );
    }

    #[test]
    fn sequence_plays_in_order() {
        let tw1 = Tween::new(0.0_f32, 10.0, 1.0, easing::linear);
        let tw2 = Tween::new(10.0_f32, 20.0, 1.0, easing::linear);
        let mut seq = TweenSequence::new(vec![tw1, tw2]);

        seq.update(0.5);
        let v = seq.value();
        assert!(
            (v - 5.0).abs() < EPSILON,
            "expected ~5 at 0.5s in first tween, got {v}"
        );

        seq.update(1.0);
        let v = seq.value();
        assert!(
            (v - 15.0).abs() < EPSILON,
            "expected ~15 at 0.5s into second tween, got {v}"
        );
    }

    #[test]
    fn parallel_all_advance() {
        let tw1 = Tween::new(0.0_f32, 100.0, 1.0, easing::linear);
        let tw2 = Tween::new(0.0_f32, 200.0, 2.0, easing::linear);
        let mut par = TweenParallel::new(vec![tw1, tw2]);

        par.update(0.5);

        let v0 = par.tweens[0].value();
        let v1 = par.tweens[1].value();
        assert!(
            (v0 - 50.0).abs() < EPSILON,
            "tween0 expected ~50, got {v0}"
        );
        assert!(
            (v1 - 50.0).abs() < EPSILON,
            "tween1 expected ~50 at t=0.5 of 2s, got {v1}"
        );

        let primary = par.value();
        assert!(
            (primary - 50.0).abs() < EPSILON,
            "primary value expected ~50, got {primary}"
        );
    }

    #[test]
    fn pause_resume() {
        let mut tw = Tween::new(0.0_f32, 100.0, 1.0, easing::linear);
        tw.update(0.3);
        tw.pause();
        tw.update(0.5);
        let v = tw.value();
        assert!(
            (v - 30.0).abs() < EPSILON,
            "expected ~30 after pause, got {v}"
        );
        tw.resume();
        tw.update(0.2);
        let v = tw.value();
        assert!(
            (v - 50.0).abs() < EPSILON,
            "expected ~50 after resume, got {v}"
        );
    }

    #[test]
    fn vec3_lerp_impl() {
        let a = Vec3::ZERO;
        let b = Vec3::new(10.0, 20.0, 30.0);
        let mid = Lerp::lerp(&a, &b, 0.5);
        assert!((mid.x - 5.0).abs() < EPSILON);
        assert!((mid.y - 10.0).abs() < EPSILON);
        assert!((mid.z - 15.0).abs() < EPSILON);
    }

    #[test]
    fn quat_lerp_impl() {
        let a = Quat::IDENTITY;
        let b = Quat::from_axis_angle(Vec3::Y, core::f32::consts::FRAC_PI_2);
        let mid = Lerp::lerp(&a, &b, 0.5);
        let expected = Quat::from_axis_angle(Vec3::Y, core::f32::consts::FRAC_PI_4);
        assert!((mid.x - expected.x).abs() < 1e-4);
        assert!((mid.y - expected.y).abs() < 1e-4);
        assert!((mid.z - expected.z).abs() < 1e-4);
        assert!((mid.w - expected.w).abs() < 1e-4);
    }

    #[test]
    fn transform_lerp_impl() {
        let a = Transform::IDENTITY;
        let b = Transform::from_position(Vec3::new(10.0, 0.0, 0.0));
        let mid = Lerp::lerp(&a, &b, 0.5);
        assert!((mid.position.x - 5.0).abs() < EPSILON);
    }
}
