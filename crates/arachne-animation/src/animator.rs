use arachne_math::Transform;

// ANIMATOR ------

#[derive(Clone, Debug)]
pub struct Animator {
    pub current_clip: Option<usize>,
    pub playback_time: f32,
    pub speed: f32,
    pub looping: bool,
    pub blend_weight: f32,
}

impl Animator {
    #[inline]
    pub fn new() -> Self {
        Self {
            current_clip: None,
            playback_time: 0.0,
            speed: 1.0,
            looping: false,
            blend_weight: 1.0,
        }
    }

    #[inline]
    pub fn play(&mut self, clip_index: usize, speed: f32, looping: bool) {
        self.current_clip = Some(clip_index);
        self.playback_time = 0.0;
        self.speed = speed;
        self.looping = looping;
    }

    pub fn update(&mut self, dt: f32, clip_duration: f32) {
        if self.current_clip.is_none() {
            return;
        }

        self.playback_time += dt * self.speed;

        if self.looping && clip_duration > 0.0 {
            while self.playback_time >= clip_duration {
                self.playback_time -= clip_duration;
            }
            while self.playback_time < 0.0 {
                self.playback_time += clip_duration;
            }
        } else {
            self.playback_time = self.playback_time.clamp(0.0, clip_duration);
        }
    }

    #[inline]
    pub fn stop(&mut self) {
        self.current_clip = None;
        self.playback_time = 0.0;
    }
}

// BLEND TRANSFORMS ------

#[inline]
pub fn blend_transforms(a: &Transform, b: &Transform, weight: f32) -> Transform {
    Transform {
        position: a.position.lerp(b.position, weight),
        rotation: a.rotation.slerp(b.rotation, weight),
        scale: a.scale.lerp(b.scale, weight),
    }
}

// CROSSFADE ------

#[derive(Clone, Debug)]
pub struct Crossfade {
    pub from_clip: usize,
    pub to_clip: usize,
    pub duration: f32,
    pub elapsed: f32,
}

impl Crossfade {
    #[inline]
    pub fn new(from_clip: usize, to_clip: usize, duration: f32) -> Self {
        Self {
            from_clip,
            to_clip,
            duration,
            elapsed: 0.0,
        }
    }

    #[inline]
    pub fn update(&mut self, dt: f32) -> f32 {
        self.elapsed += dt;
        if self.elapsed > self.duration {
            self.elapsed = self.duration;
        }
        if self.duration > 0.0 {
            self.elapsed / self.duration
        } else {
            1.0
        }
    }

    #[inline]
    pub fn is_complete(&self) -> bool {
        self.elapsed >= self.duration
    }
}

// TESTS ------

#[cfg(test)]
mod tests {
    use super::*;
    use arachne_math::{Vec3, Quat};

    const EPSILON: f32 = 1e-5;

    #[test]
    fn play_and_advance() {
        let mut anim = Animator::new();
        anim.play(0, 1.0, false);
        anim.update(0.5, 2.0);
        assert!(
            (anim.playback_time - 0.5).abs() < EPSILON,
            "expected 0.5, got {}",
            anim.playback_time
        );
    }

    #[test]
    fn play_looping() {
        let mut anim = Animator::new();
        anim.play(0, 1.0, true);
        anim.update(2.5, 2.0);
        assert!(
            (anim.playback_time - 0.5).abs() < EPSILON,
            "expected 0.5 after loop, got {}",
            anim.playback_time
        );
    }

    #[test]
    fn play_clamped() {
        let mut anim = Animator::new();
        anim.play(0, 1.0, false);
        anim.update(5.0, 2.0);
        assert!(
            (anim.playback_time - 2.0).abs() < EPSILON,
            "expected clamped to 2.0, got {}",
            anim.playback_time
        );
    }

    #[test]
    fn stop_resets() {
        let mut anim = Animator::new();
        anim.play(0, 1.0, false);
        anim.update(1.0, 2.0);
        anim.stop();
        assert!(anim.current_clip.is_none());
        assert!((anim.playback_time - 0.0).abs() < EPSILON);
    }

    #[test]
    fn blend_transforms_50_percent() {
        let a = Transform::from_position(Vec3::new(0.0, 0.0, 0.0));
        let b = Transform::from_position(Vec3::new(10.0, 20.0, 30.0));
        let result = blend_transforms(&a, &b, 0.5);

        assert!(
            (result.position.x - 5.0).abs() < EPSILON,
            "expected x=5, got {}",
            result.position.x
        );
        assert!(
            (result.position.y - 10.0).abs() < EPSILON,
            "expected y=10, got {}",
            result.position.y
        );
        assert!(
            (result.position.z - 15.0).abs() < EPSILON,
            "expected z=15, got {}",
            result.position.z
        );
    }

    #[test]
    fn blend_transforms_with_rotation() {
        let a = Transform::from_rotation(Quat::IDENTITY);
        let b = Transform::from_rotation(
            Quat::from_axis_angle(Vec3::Y, core::f32::consts::FRAC_PI_2),
        );
        let result = blend_transforms(&a, &b, 0.5);
        let expected_rot = Quat::from_axis_angle(Vec3::Y, core::f32::consts::FRAC_PI_4);
        assert!((result.rotation.x - expected_rot.x).abs() < 1e-4);
        assert!((result.rotation.y - expected_rot.y).abs() < 1e-4);
        assert!((result.rotation.z - expected_rot.z).abs() < 1e-4);
        assert!((result.rotation.w - expected_rot.w).abs() < 1e-4);
    }

    #[test]
    fn crossfade_progression() {
        let mut cf = Crossfade::new(0, 1, 0.5);
        assert!(!cf.is_complete());

        let w1 = cf.update(0.25);
        assert!(
            (w1 - 0.5).abs() < EPSILON,
            "expected weight 0.5 halfway, got {w1}"
        );
        assert!(!cf.is_complete());

        let w2 = cf.update(0.25);
        assert!(
            (w2 - 1.0).abs() < EPSILON,
            "expected weight 1.0 at end, got {w2}"
        );
        assert!(cf.is_complete());
    }

    #[test]
    fn crossfade_starts_at_zero() {
        let cf = Crossfade::new(0, 1, 1.0);
        let weight = if cf.duration > 0.0 {
            cf.elapsed / cf.duration
        } else {
            1.0
        };
        assert!(
            (weight - 0.0).abs() < EPSILON,
            "expected initial weight 0, got {weight}"
        );
    }
}
