use std::collections::HashMap;

use crate::keyframe::AnyKeyframeTrack;

// PROPERTY PATH ------

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PropertyPath(pub String);

impl PropertyPath {
    #[inline]
    pub fn new(path: &str) -> Self {
        Self(path.to_string())
    }
}

// ANIMATION CLIP ------

pub struct AnimationClip {
    pub name: String,
    pub duration: f32,
    pub tracks: HashMap<PropertyPath, Box<dyn AnyKeyframeTrack>>,
}

impl AnimationClip {
    #[inline]
    pub fn new(name: &str, duration: f32) -> Self {
        Self {
            name: name.to_string(),
            duration,
            tracks: HashMap::new(),
        }
    }

    #[inline]
    pub fn add_track(&mut self, path: PropertyPath, track: Box<dyn AnyKeyframeTrack>) {
        self.tracks.insert(path, track);
    }

    #[inline]
    pub fn sample_f32(&self, path: &PropertyPath, time: f32) -> Option<f32> {
        self.tracks.get(path)?.sample_f32(time)
    }
}

// CLIP PLAYBACK ------

#[derive(Clone, Debug, PartialEq)]
pub struct ClipPlayback {
    pub clip_index: usize,
    pub current_time: f32,
    pub speed: f32,
    pub looping: bool,
}

impl ClipPlayback {
    #[inline]
    pub fn new(clip_index: usize, speed: f32, looping: bool) -> Self {
        Self {
            clip_index,
            current_time: 0.0,
            speed,
            looping,
        }
    }

    pub fn advance(&mut self, dt: f32, duration: f32) {
        self.current_time += dt * self.speed;
        if self.looping && duration > 0.0 {
            while self.current_time >= duration {
                self.current_time -= duration;
            }
            while self.current_time < 0.0 {
                self.current_time += duration;
            }
        } else {
            self.current_time = self.current_time.clamp(0.0, duration);
        }
    }
}

// TESTS ------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keyframe::{InterpolationMode, Keyframe, KeyframeTrack};

    const EPSILON: f32 = 1e-5;

    #[test]
    fn clip_sample_tracks() {
        let mut clip = AnimationClip::new("test_clip", 2.0);

        let mut track = KeyframeTrack::<f32>::new();
        track.add_keyframe(Keyframe::new(0.0, 0.0, InterpolationMode::Linear));
        track.add_keyframe(Keyframe::new(2.0, 100.0, InterpolationMode::Linear));

        let path = PropertyPath::new("transform.position.x");
        clip.add_track(path.clone(), Box::new(track));

        let v0 = clip.sample_f32(&path, 0.0).unwrap();
        assert!((v0 - 0.0).abs() < EPSILON, "expected 0 at t=0, got {v0}");

        let v1 = clip.sample_f32(&path, 1.0).unwrap();
        assert!((v1 - 50.0).abs() < EPSILON, "expected 50 at t=1, got {v1}");

        let v2 = clip.sample_f32(&path, 2.0).unwrap();
        assert!((v2 - 100.0).abs() < EPSILON, "expected 100 at t=2, got {v2}");
    }

    #[test]
    fn clip_missing_path_returns_none() {
        let clip = AnimationClip::new("empty", 1.0);
        let path = PropertyPath::new("nonexistent");
        assert!(clip.sample_f32(&path, 0.5).is_none());
    }

    #[test]
    fn playback_advance_looping() {
        let mut pb = ClipPlayback::new(0, 1.0, true);
        pb.advance(2.5, 2.0);
        assert!(
            (pb.current_time - 0.5).abs() < EPSILON,
            "expected 0.5 after loop, got {}",
            pb.current_time
        );
    }

    #[test]
    fn playback_advance_clamped() {
        let mut pb = ClipPlayback::new(0, 1.0, false);
        pb.advance(5.0, 2.0);
        assert!(
            (pb.current_time - 2.0).abs() < EPSILON,
            "expected clamped to 2.0, got {}",
            pb.current_time
        );
    }

    #[test]
    fn property_path_hash_eq() {
        let a = PropertyPath::new("transform.position.x");
        let b = PropertyPath::new("transform.position.x");
        let c = PropertyPath::new("transform.position.y");
        assert_eq!(a, b);
        assert_ne!(a, c);

        let mut map = HashMap::new();
        map.insert(a, 42);
        assert_eq!(map.get(&b), Some(&42));
    }
}
