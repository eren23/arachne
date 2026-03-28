use crate::tween::Lerp;

// INTERPOLATION MODE ------

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum InterpolationMode {
    Linear,
    Step,
    CubicBezier { cp1: f32, cp2: f32 },
}

// KEYFRAME ------

#[derive(Clone, Debug)]
pub struct Keyframe<T> {
    pub time: f32,
    pub value: T,
    pub interpolation: InterpolationMode,
}

impl<T> Keyframe<T> {
    #[inline]
    pub fn new(time: f32, value: T, interpolation: InterpolationMode) -> Self {
        Self {
            time,
            value,
            interpolation,
        }
    }
}

// CUBIC BEZIER HELPERS ------

fn sample_bezier_curve(t: f32, cp1: f32, cp2: f32) -> f32 {
    let u = 1.0 - t;
    let p0 = 0.0;
    let p3 = 1.0;
    u * u * u * p0 + 3.0 * u * u * t * cp1 + 3.0 * u * t * t * cp2 + t * t * t * p3
}

// KEYFRAME TRACK ------

#[derive(Clone)]
pub struct KeyframeTrack<T: Lerp + Clone> {
    pub keyframes: Vec<Keyframe<T>>,
}

impl<T: Lerp + Clone> KeyframeTrack<T> {
    #[inline]
    pub fn new() -> Self {
        Self {
            keyframes: Vec::new(),
        }
    }

    pub fn add_keyframe(&mut self, kf: Keyframe<T>) {
        let pos = self
            .keyframes
            .iter()
            .position(|k| k.time > kf.time)
            .unwrap_or(self.keyframes.len());
        self.keyframes.insert(pos, kf);
    }

    pub fn sample(&self, time: f32) -> Option<T> {
        if self.keyframes.is_empty() {
            return None;
        }

        if time <= self.keyframes[0].time {
            return Some(self.keyframes[0].value.clone());
        }

        let last = self.keyframes.len() - 1;
        if time >= self.keyframes[last].time {
            return Some(self.keyframes[last].value.clone());
        }

        // Binary search for the first keyframe with time > time
        let idx = match self.keyframes.binary_search_by(|k| {
            k.time.partial_cmp(&time).unwrap_or(core::cmp::Ordering::Equal)
        }) {
            Ok(i) => i + 1, // exact match: next keyframe
            Err(i) => i,    // insertion point: first keyframe > time
        };

        if idx == 0 {
            return Some(self.keyframes[0].value.clone());
        }

        let prev = &self.keyframes[idx - 1];
        let next = &self.keyframes[idx];
        let span = next.time - prev.time;
        if span <= 0.0 {
            return Some(prev.value.clone());
        }

        let raw_t = (time - prev.time) / span;

        match prev.interpolation {
            InterpolationMode::Linear => {
                Some(prev.value.lerp(&next.value, raw_t))
            }
            InterpolationMode::Step => {
                Some(prev.value.clone())
            }
            InterpolationMode::CubicBezier { cp1, cp2 } => {
                let eased_t = sample_bezier_curve(raw_t, cp1, cp2);
                Some(prev.value.lerp(&next.value, eased_t))
            }
        }
    }

    #[inline]
    pub fn duration(&self) -> f32 {
        self.keyframes.last().map(|k| k.time).unwrap_or(0.0)
    }
}

// ANY KEYFRAME TRACK ------

pub trait AnyKeyframeTrack {
    fn sample_f32(&self, time: f32) -> Option<f32>;
    fn duration(&self) -> f32;
}

impl AnyKeyframeTrack for KeyframeTrack<f32> {
    #[inline]
    fn sample_f32(&self, time: f32) -> Option<f32> {
        self.sample(time)
    }

    #[inline]
    fn duration(&self) -> f32 {
        self.duration()
    }
}

// TESTS ------

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-5;

    #[test]
    fn linear_interpolation_midpoint() {
        let mut track = KeyframeTrack::<f32>::new();
        track.add_keyframe(Keyframe::new(0.0, 0.0, InterpolationMode::Linear));
        track.add_keyframe(Keyframe::new(1.0, 100.0, InterpolationMode::Linear));

        let v = track.sample(0.5).unwrap();
        assert!(
            (v - 50.0).abs() < EPSILON,
            "expected ~50 at midpoint, got {v}"
        );
    }

    #[test]
    fn step_interpolation_holds_value() {
        let mut track = KeyframeTrack::<f32>::new();
        track.add_keyframe(Keyframe::new(0.0, 0.0, InterpolationMode::Step));
        track.add_keyframe(Keyframe::new(1.0, 100.0, InterpolationMode::Step));

        let v = track.sample(0.5).unwrap();
        assert!(
            (v - 0.0).abs() < EPSILON,
            "step should hold previous value, got {v}"
        );

        let v2 = track.sample(0.99).unwrap();
        assert!(
            (v2 - 0.0).abs() < EPSILON,
            "step should still hold 0 at 0.99, got {v2}"
        );
    }

    #[test]
    fn cubic_bezier_interpolation() {
        let mut track = KeyframeTrack::<f32>::new();
        track.add_keyframe(Keyframe::new(
            0.0,
            0.0,
            InterpolationMode::CubicBezier { cp1: 0.25, cp2: 0.75 },
        ));
        track.add_keyframe(Keyframe::new(1.0, 100.0, InterpolationMode::Linear));

        let v = track.sample(0.5).unwrap();
        assert!(
            v > 0.0 && v < 100.0,
            "cubic bezier should produce value between 0 and 100, got {v}"
        );

        let v_start = track.sample(0.0).unwrap();
        let v_end = track.sample(1.0).unwrap();
        assert!(
            (v_start - 0.0).abs() < EPSILON,
            "bezier start should be ~0, got {v_start}"
        );
        assert!(
            (v_end - 100.0).abs() < EPSILON,
            "bezier end should be ~100, got {v_end}"
        );
    }

    #[test]
    fn before_first_returns_first() {
        let mut track = KeyframeTrack::<f32>::new();
        track.add_keyframe(Keyframe::new(1.0, 42.0, InterpolationMode::Linear));
        track.add_keyframe(Keyframe::new(2.0, 100.0, InterpolationMode::Linear));

        let v = track.sample(0.0).unwrap();
        assert!(
            (v - 42.0).abs() < EPSILON,
            "before first should return first value, got {v}"
        );
    }

    #[test]
    fn after_last_returns_last() {
        let mut track = KeyframeTrack::<f32>::new();
        track.add_keyframe(Keyframe::new(0.0, 0.0, InterpolationMode::Linear));
        track.add_keyframe(Keyframe::new(1.0, 42.0, InterpolationMode::Linear));

        let v = track.sample(5.0).unwrap();
        assert!(
            (v - 42.0).abs() < EPSILON,
            "after last should return last value, got {v}"
        );
    }

    #[test]
    fn sort_order_maintained() {
        let mut track = KeyframeTrack::<f32>::new();
        track.add_keyframe(Keyframe::new(2.0, 200.0, InterpolationMode::Linear));
        track.add_keyframe(Keyframe::new(0.0, 0.0, InterpolationMode::Linear));
        track.add_keyframe(Keyframe::new(1.0, 100.0, InterpolationMode::Linear));

        assert!((track.keyframes[0].time - 0.0).abs() < EPSILON);
        assert!((track.keyframes[1].time - 1.0).abs() < EPSILON);
        assert!((track.keyframes[2].time - 2.0).abs() < EPSILON);
    }

    #[test]
    fn any_keyframe_track_f32() {
        let mut track = KeyframeTrack::<f32>::new();
        track.add_keyframe(Keyframe::new(0.0, 0.0, InterpolationMode::Linear));
        track.add_keyframe(Keyframe::new(1.0, 100.0, InterpolationMode::Linear));

        let any_track: &dyn AnyKeyframeTrack = &track;
        let v = any_track.sample_f32(0.5).unwrap();
        assert!(
            (v - 50.0).abs() < EPSILON,
            "AnyKeyframeTrack sample_f32 expected ~50, got {v}"
        );
        assert!(
            (any_track.duration() - 1.0).abs() < EPSILON,
            "AnyKeyframeTrack duration expected 1.0, got {}",
            any_track.duration()
        );
    }

    #[test]
    fn keyframe_track_1000_keyframes() {
        let mut track = KeyframeTrack::<f32>::new();
        for i in 0..1000 {
            let t = i as f32;
            track.add_keyframe(Keyframe::new(t, t * 10.0, InterpolationMode::Linear));
        }

        let start = std::time::Instant::now();
        // Sample between keyframes (not past last)
        for i in 0..999 {
            let t = i as f32 + 0.5;
            let v = track.sample(t).unwrap();
            let expected = t * 10.0;
            assert!(
                (v - expected).abs() < 0.1,
                "at t={t}, expected ~{expected}, got {v}"
            );
        }
        let elapsed = start.elapsed();
        eprintln!("999 samples on 1000-keyframe track: {:.3}ms", elapsed.as_secs_f64() * 1000.0);
    }
}
