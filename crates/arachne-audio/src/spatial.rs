//! 3D spatial audio: listener, spatial sources, distance attenuation, and
//! stereo panning derived from source position relative to listener.

use arachne_math::Vec3;

/// Represents the listener in 3D space.
#[derive(Clone, Copy, Debug)]
pub struct Listener {
    /// World position of the listener.
    pub position: Vec3,
    /// Forward direction (unit vector).
    pub forward: Vec3,
    /// Up direction (unit vector).
    pub up: Vec3,
}

impl Default for Listener {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            forward: Vec3::new(0.0, 0.0, -1.0),
            up: Vec3::Y,
        }
    }
}

impl Listener {
    /// Creates a new listener at the given position, looking in `forward` direction.
    pub fn new(position: Vec3, forward: Vec3, up: Vec3) -> Self {
        Self {
            position,
            forward: forward.normalize(),
            up: up.normalize(),
        }
    }

    /// Returns the right vector (cross product of forward and up).
    #[inline]
    pub fn right(&self) -> Vec3 {
        self.forward.cross(self.up).normalize()
    }
}

/// A spatial audio source in 3D space.
#[derive(Clone, Copy, Debug)]
pub struct SpatialSource {
    /// World position of the source.
    pub position: Vec3,
}

impl SpatialSource {
    pub fn new(position: Vec3) -> Self {
        Self { position }
    }
}

/// Distance attenuation models.
#[derive(Clone, Copy, Debug)]
pub enum DistanceModel {
    /// No distance attenuation.
    None,
    /// Linear falloff between `min_dist` and `max_dist`.
    Linear {
        min_dist: f32,
        max_dist: f32,
    },
    /// Inverse distance: gain = ref_dist / (ref_dist + rolloff * (dist - ref_dist))
    /// Clamped to [0, max_dist].
    Inverse {
        ref_dist: f32,
        max_dist: f32,
        rolloff: f32,
    },
    /// Exponential rolloff: gain = (dist / ref_dist) ^ -rolloff
    Exponential {
        ref_dist: f32,
        rolloff: f32,
    },
}

impl DistanceModel {
    /// Computes the attenuation factor for a given distance.
    /// Returns a value in [0.0, 1.0].
    pub fn attenuation(&self, distance: f32) -> f32 {
        match *self {
            DistanceModel::None => 1.0,
            DistanceModel::Linear { min_dist, max_dist } => {
                if distance <= min_dist {
                    1.0
                } else if distance >= max_dist {
                    0.0
                } else {
                    1.0 - (distance - min_dist) / (max_dist - min_dist)
                }
            }
            DistanceModel::Inverse { ref_dist, max_dist, rolloff } => {
                let dist = distance.clamp(ref_dist, max_dist);
                if ref_dist <= 0.0 {
                    return 0.0;
                }
                let gain = ref_dist / (ref_dist + rolloff * (dist - ref_dist));
                gain.clamp(0.0, 1.0)
            }
            DistanceModel::Exponential { ref_dist, rolloff } => {
                if distance <= ref_dist || ref_dist <= 0.0 {
                    return 1.0;
                }
                let gain = (distance / ref_dist).powf(-rolloff);
                gain.clamp(0.0, 1.0)
            }
        }
    }
}

/// Computes the stereo pan value [-1.0, 1.0] from a listener and source position.
///
/// Projects the listener-to-source vector onto the listener's right axis
/// to determine left/right bias.
///
/// Returns a pan value where -1.0 is fully left and 1.0 is fully right.
pub fn compute_pan(listener: &Listener, source_pos: Vec3) -> f32 {
    let to_source = source_pos - listener.position;
    let dist = to_source.length();

    if dist < 1e-8 {
        return 0.0;
    }

    let dir = to_source * (1.0 / dist);
    let right = listener.right();

    // Dot product with right vector gives sine of angle
    let pan = dir.dot(right);
    pan.clamp(-1.0, 1.0)
}

/// Computes the angle in degrees between the listener's forward direction
/// and the direction to the source, projected onto the horizontal plane.
pub fn compute_angle_degrees(listener: &Listener, source_pos: Vec3) -> f32 {
    let to_source = source_pos - listener.position;
    let dist = to_source.length();

    if dist < 1e-8 {
        return 0.0;
    }

    let dir = to_source * (1.0 / dist);

    // Project onto the listener's horizontal plane (forward, right)
    let right = listener.right();
    let fwd_component = dir.dot(listener.forward);
    let right_component = dir.dot(right);

    // atan2 gives angle from forward axis
    right_component.atan2(fwd_component).to_degrees()
}

/// Full spatial audio parameters computed from listener and source.
#[derive(Clone, Copy, Debug)]
pub struct SpatialParams {
    /// Stereo pan [-1.0, 1.0].
    pub pan: f32,
    /// Distance attenuation [0.0, 1.0].
    pub attenuation: f32,
    /// Angle in degrees from listener forward.
    pub angle_degrees: f32,
}

/// Computes all spatial parameters for a source relative to a listener.
pub fn compute_spatial(
    listener: &Listener,
    source: &SpatialSource,
    model: &DistanceModel,
) -> SpatialParams {
    let distance = listener.position.distance(source.position);
    let pan = compute_pan(listener, source.position);
    let attenuation = model.attenuation(distance);
    let angle_degrees = compute_angle_degrees(listener, source.position);

    SpatialParams {
        pan,
        attenuation,
        angle_degrees,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-5;
    const ANGLE_EPSILON: f32 = 1.0; // 1 degree tolerance

    fn assert_f32_approx(a: f32, b: f32) {
        assert!(
            (a - b).abs() < EPSILON,
            "assertion failed: {a} != {b} (within epsilon {EPSILON})"
        );
    }

    fn assert_angle_approx(a: f32, b: f32) {
        assert!(
            (a - b).abs() < ANGLE_EPSILON,
            "angle assertion failed: {a} != {b} (within {ANGLE_EPSILON} degrees)"
        );
    }

    fn default_listener() -> Listener {
        Listener::new(
            Vec3::ZERO,
            Vec3::new(0.0, 0.0, -1.0), // looking -Z
            Vec3::Y,
        )
    }

    #[test]
    fn test_source_directly_right_pans_right() {
        let listener = default_listener();
        let source_pos = Vec3::new(10.0, 0.0, 0.0); // 10 units right
        let pan = compute_pan(&listener, source_pos);

        // With listener looking -Z, right is +X
        assert!(pan > 0.9, "expected pan > 0.9, got {pan}");
    }

    #[test]
    fn test_source_directly_left_pans_left() {
        let listener = default_listener();
        let source_pos = Vec3::new(-10.0, 0.0, 0.0);
        let pan = compute_pan(&listener, source_pos);

        assert!(pan < -0.9, "expected pan < -0.9, got {pan}");
    }

    #[test]
    fn test_source_directly_ahead_center_pan() {
        let listener = default_listener();
        let source_pos = Vec3::new(0.0, 0.0, -10.0); // directly ahead
        let pan = compute_pan(&listener, source_pos);

        assert_f32_approx(pan, 0.0);
    }

    #[test]
    fn test_source_at_listener_center_pan() {
        let listener = default_listener();
        let pan = compute_pan(&listener, Vec3::ZERO);
        assert_f32_approx(pan, 0.0);
    }

    #[test]
    fn test_angle_right_90_degrees() {
        let listener = default_listener();
        let source_pos = Vec3::new(10.0, 0.0, 0.0);
        let angle = compute_angle_degrees(&listener, source_pos);

        assert_angle_approx(angle, 90.0);
    }

    #[test]
    fn test_angle_left_minus_90_degrees() {
        let listener = default_listener();
        let source_pos = Vec3::new(-10.0, 0.0, 0.0);
        let angle = compute_angle_degrees(&listener, source_pos);

        assert_angle_approx(angle, -90.0);
    }

    #[test]
    fn test_angle_directly_ahead() {
        let listener = default_listener();
        let source_pos = Vec3::new(0.0, 0.0, -10.0);
        let angle = compute_angle_degrees(&listener, source_pos);

        assert_angle_approx(angle, 0.0);
    }

    #[test]
    fn test_angle_behind() {
        let listener = default_listener();
        let source_pos = Vec3::new(0.0, 0.0, 10.0);
        let angle = compute_angle_degrees(&listener, source_pos);

        // Behind = 180 or -180 degrees
        assert!(angle.abs() > 179.0, "expected ~180 degrees, got {angle}");
    }

    #[test]
    fn test_linear_attenuation() {
        let model = DistanceModel::Linear { min_dist: 1.0, max_dist: 11.0 };

        assert_f32_approx(model.attenuation(0.5), 1.0); // closer than min
        assert_f32_approx(model.attenuation(1.0), 1.0); // at min
        assert_f32_approx(model.attenuation(6.0), 0.5); // halfway
        assert_f32_approx(model.attenuation(11.0), 0.0); // at max
        assert_f32_approx(model.attenuation(20.0), 0.0); // beyond max
    }

    #[test]
    fn test_inverse_attenuation() {
        let model = DistanceModel::Inverse {
            ref_dist: 1.0,
            max_dist: 100.0,
            rolloff: 1.0,
        };

        assert_f32_approx(model.attenuation(1.0), 1.0);
        assert_f32_approx(model.attenuation(2.0), 0.5);
        assert_f32_approx(model.attenuation(10.0), 0.1);
    }

    #[test]
    fn test_exponential_attenuation() {
        let model = DistanceModel::Exponential {
            ref_dist: 1.0,
            rolloff: 2.0,
        };

        assert_f32_approx(model.attenuation(0.5), 1.0); // clamped to ref
        assert_f32_approx(model.attenuation(1.0), 1.0);
        // At dist=2, rolloff=2: (2/1)^-2 = 0.25
        assert_f32_approx(model.attenuation(2.0), 0.25);
    }

    #[test]
    fn test_no_attenuation() {
        let model = DistanceModel::None;
        assert_f32_approx(model.attenuation(100.0), 1.0);
    }

    #[test]
    fn test_spatial_far_source_attenuated() {
        let listener = default_listener();
        let source = SpatialSource::new(Vec3::new(50.0, 0.0, 0.0));
        let model = DistanceModel::Linear { min_dist: 1.0, max_dist: 20.0 };

        let params = compute_spatial(&listener, &source, &model);
        assert_f32_approx(params.attenuation, 0.0);
        assert!(params.pan > 0.9);
    }

    #[test]
    fn test_spatial_close_source_loud() {
        let listener = default_listener();
        let source = SpatialSource::new(Vec3::new(0.5, 0.0, -0.5));
        let model = DistanceModel::Linear { min_dist: 1.0, max_dist: 20.0 };

        let params = compute_spatial(&listener, &source, &model);
        assert_f32_approx(params.attenuation, 1.0); // within min_dist
    }

    #[test]
    fn test_spatial_10_units_right() {
        let listener = default_listener();
        let source = SpatialSource::new(Vec3::new(10.0, 0.0, 0.0));
        let model = DistanceModel::Linear { min_dist: 1.0, max_dist: 20.0 };

        let params = compute_spatial(&listener, &source, &model);
        assert!(params.pan > 0.9, "should pan right, got {}", params.pan);
        assert!(params.attenuation > 0.0 && params.attenuation < 1.0);
        assert_angle_approx(params.angle_degrees, 90.0);
    }

    #[test]
    fn test_listener_right_vector() {
        let listener = default_listener();
        let right = listener.right();
        // forward = (0,0,-1), up = (0,1,0)
        // right = forward x up = (0,0,-1) x (0,1,0)
        // = (0*0 - (-1)*1, (-1)*0 - 0*0, 0*1 - 0*0) = (1, 0, 0)
        assert_f32_approx(right.x, 1.0);
        assert_f32_approx(right.y, 0.0);
        assert_f32_approx(right.z, 0.0);
    }

    #[test]
    fn test_rotated_listener() {
        // Listener looking +X (right), up = +Y
        let listener = Listener::new(Vec3::ZERO, Vec3::X, Vec3::Y);

        // Source at (0, 0, -10) -> to the left of this listener
        let pan = compute_pan(&listener, Vec3::new(0.0, 0.0, -10.0));
        assert!(pan < -0.9, "expected left pan, got {pan}");

        // Source at (0, 0, 10) -> to the right
        let pan = compute_pan(&listener, Vec3::new(0.0, 0.0, 10.0));
        assert!(pan > 0.9, "expected right pan, got {pan}");
    }
}
