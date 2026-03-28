use arachne_math::Vec2;

/// Unique handle to a rigid body in the physics world.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BodyHandle(pub u32);

/// The type of a rigid body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyType {
    /// Zero mass, zero velocity, never moves.
    Static,
    /// Fully simulated: responds to forces, impulses, collisions.
    Dynamic,
    /// Externally controlled velocity; infinite mass for collision response.
    Kinematic,
}

/// All data for a single rigid body.
#[derive(Clone, Debug)]
pub struct RigidBodyData {
    pub body_type: BodyType,
    pub position: Vec2,
    pub rotation: f32,

    pub linear_velocity: Vec2,
    pub angular_velocity: f32,

    pub mass: f32,
    pub inv_mass: f32,
    pub inertia: f32,
    pub inv_inertia: f32,

    pub linear_damping: f32,
    pub angular_damping: f32,
    pub gravity_scale: f32,

    // Force/torque accumulators (cleared each step).
    pub force: Vec2,
    pub torque: f32,
}

impl RigidBodyData {
    /// Creates a new dynamic body at the given position.
    pub fn new_dynamic(position: Vec2, mass: f32, inertia: f32) -> Self {
        assert!(mass > 0.0, "Dynamic body mass must be > 0");
        assert!(inertia > 0.0, "Dynamic body inertia must be > 0");
        Self {
            body_type: BodyType::Dynamic,
            position,
            rotation: 0.0,
            linear_velocity: Vec2::ZERO,
            angular_velocity: 0.0,
            mass,
            inv_mass: 1.0 / mass,
            inertia,
            inv_inertia: 1.0 / inertia,
            linear_damping: 0.0,
            angular_damping: 0.0,
            gravity_scale: 1.0,
            force: Vec2::ZERO,
            torque: 0.0,
        }
    }

    /// Creates a new static body at the given position.
    pub fn new_static(position: Vec2) -> Self {
        Self {
            body_type: BodyType::Static,
            position,
            rotation: 0.0,
            linear_velocity: Vec2::ZERO,
            angular_velocity: 0.0,
            mass: 0.0,
            inv_mass: 0.0,
            inertia: 0.0,
            inv_inertia: 0.0,
            linear_damping: 0.0,
            angular_damping: 0.0,
            gravity_scale: 0.0,
            force: Vec2::ZERO,
            torque: 0.0,
        }
    }

    /// Creates a new kinematic body at the given position.
    pub fn new_kinematic(position: Vec2) -> Self {
        Self {
            body_type: BodyType::Kinematic,
            position,
            rotation: 0.0,
            linear_velocity: Vec2::ZERO,
            angular_velocity: 0.0,
            mass: 0.0,
            inv_mass: 0.0,
            inertia: 0.0,
            inv_inertia: 0.0,
            linear_damping: 0.0,
            angular_damping: 0.0,
            gravity_scale: 0.0,
            force: Vec2::ZERO,
            torque: 0.0,
        }
    }

    /// Applies a force at the center of mass (accumulated until next step).
    #[inline]
    pub fn apply_force(&mut self, f: Vec2) {
        if self.body_type == BodyType::Dynamic {
            self.force += f;
        }
    }

    /// Applies a torque (accumulated until next step).
    #[inline]
    pub fn apply_torque(&mut self, t: f32) {
        if self.body_type == BodyType::Dynamic {
            self.torque += t;
        }
    }

    /// Applies an instantaneous impulse at center of mass.
    #[inline]
    pub fn apply_impulse(&mut self, impulse: Vec2) {
        if self.body_type == BodyType::Dynamic {
            self.linear_velocity += impulse * self.inv_mass;
        }
    }

    /// Applies an impulse at a world-space point, affecting both linear and angular velocity.
    #[inline]
    pub fn apply_impulse_at_point(&mut self, impulse: Vec2, point: Vec2) {
        if self.body_type == BodyType::Dynamic {
            self.linear_velocity += impulse * self.inv_mass;
            let r = point - self.position;
            self.angular_velocity += r.cross(impulse) * self.inv_inertia;
        }
    }

    /// Semi-implicit Euler: integrate forces to update velocity.
    #[inline]
    pub fn integrate_forces(&mut self, gravity: Vec2, dt: f32) {
        if self.body_type != BodyType::Dynamic {
            return;
        }
        // Apply gravity
        self.linear_velocity += (gravity * self.gravity_scale + self.force * self.inv_mass) * dt;
        self.angular_velocity += self.torque * self.inv_inertia * dt;

        // Apply damping
        self.linear_velocity = self.linear_velocity * (1.0 / (1.0 + self.linear_damping * dt));
        self.angular_velocity = self.angular_velocity * (1.0 / (1.0 + self.angular_damping * dt));

        // Clear accumulators
        self.force = Vec2::ZERO;
        self.torque = 0.0;
    }

    /// Semi-implicit Euler: integrate velocity to update position.
    #[inline]
    pub fn integrate_positions(&mut self, dt: f32) {
        if self.body_type == BodyType::Static {
            return;
        }
        self.position += self.linear_velocity * dt;
        self.rotation += self.angular_velocity * dt;
    }

    /// Returns the velocity of a world-space point on this body.
    #[inline]
    pub fn velocity_at_point(&self, point: Vec2) -> Vec2 {
        let r = point - self.position;
        // v = v_cm + omega x r  (in 2D: omega x r = (-omega*r.y, omega*r.x))
        Vec2::new(
            self.linear_velocity.x - self.angular_velocity * r.y,
            self.linear_velocity.y + self.angular_velocity * r.x,
        )
    }

    /// Sets mass and computes inverse mass. Use 0 for infinite mass (static-like).
    pub fn set_mass(&mut self, mass: f32) {
        self.mass = mass;
        self.inv_mass = if mass > 0.0 { 1.0 / mass } else { 0.0 };
    }

    /// Sets rotational inertia and computes inverse.
    pub fn set_inertia(&mut self, inertia: f32) {
        self.inertia = inertia;
        self.inv_inertia = if inertia > 0.0 { 1.0 / inertia } else { 0.0 };
    }
}

/// Compute mass and rotational inertia for common shapes.
pub mod mass {
    /// Circle: mass = density * pi * r^2, inertia = 0.5 * m * r^2.
    pub fn circle(density: f32, radius: f32) -> (f32, f32) {
        let m = density * std::f32::consts::PI * radius * radius;
        let i = 0.5 * m * radius * radius;
        (m, i)
    }

    /// Rectangle (AABB): mass = density * w * h, inertia = m/12 * (w^2 + h^2).
    pub fn rectangle(density: f32, half_w: f32, half_h: f32) -> (f32, f32) {
        let w = half_w * 2.0;
        let h = half_h * 2.0;
        let m = density * w * h;
        let i = m / 12.0 * (w * w + h * h);
        (m, i)
    }

    /// Capsule: two semicircles + rectangle.
    pub fn capsule(density: f32, half_height: f32, radius: f32) -> (f32, f32) {
        let rect_h = half_height * 2.0;
        let rect_w = radius * 2.0;
        let m_rect = density * rect_w * rect_h;
        let m_circle = density * std::f32::consts::PI * radius * radius;
        let m = m_rect + m_circle;

        // Rectangle inertia about center
        let i_rect = m_rect / 12.0 * (rect_w * rect_w + rect_h * rect_h);
        // Two semicircles = one full circle, shifted by half_height via parallel axis
        let i_circle_local = 0.5 * m_circle * radius * radius;
        // Parallel axis for each semicircle offset by half_height
        // Each semicircle has mass m_circle/2, offset half_height from center
        let i_circle = i_circle_local + m_circle * half_height * half_height;

        (m, i_rect + i_circle)
    }

    /// Convex polygon via shoelace area + polygon inertia formula.
    pub fn polygon(density: f32, vertices: &[arachne_math::Vec2]) -> (f32, f32) {
        let n = vertices.len();
        if n < 3 {
            return (0.0, 0.0);
        }

        let mut area = 0.0_f32;
        let mut inertia_sum = 0.0_f32;

        for i in 0..n {
            let v0 = vertices[i];
            let v1 = vertices[(i + 1) % n];
            let cross = v0.cross(v1);
            area += cross;
            inertia_sum += cross * (v0.dot(v0) + v0.dot(v1) + v1.dot(v1));
        }

        area = area.abs() * 0.5;
        let m = density * area;
        let i = (density / 6.0) * inertia_sum.abs();
        (m, i)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_force_and_integrate() {
        let mut body = RigidBodyData::new_dynamic(Vec2::ZERO, 2.0, 1.0);
        body.apply_force(Vec2::new(10.0, 0.0));
        body.integrate_forces(Vec2::ZERO, 1.0);
        // v = F * inv_m * dt = 10 * 0.5 * 1 = 5
        assert!((body.linear_velocity.x - 5.0).abs() < 1e-6);
        assert!((body.linear_velocity.y).abs() < 1e-6);
    }

    #[test]
    fn torque_to_angular_velocity() {
        let mut body = RigidBodyData::new_dynamic(Vec2::ZERO, 1.0, 2.0);
        body.apply_torque(4.0);
        body.integrate_forces(Vec2::ZERO, 1.0);
        // omega = torque * inv_inertia * dt = 4 * 0.5 * 1 = 2
        assert!((body.angular_velocity - 2.0).abs() < 1e-6);
    }

    #[test]
    fn integrate_positions() {
        let mut body = RigidBodyData::new_dynamic(Vec2::ZERO, 1.0, 1.0);
        body.linear_velocity = Vec2::new(3.0, 4.0);
        body.angular_velocity = 1.0;
        body.integrate_positions(0.5);
        assert!((body.position.x - 1.5).abs() < 1e-6);
        assert!((body.position.y - 2.0).abs() < 1e-6);
        assert!((body.rotation - 0.5).abs() < 1e-6);
    }

    #[test]
    fn static_body_never_moves() {
        let mut body = RigidBodyData::new_static(Vec2::new(5.0, 5.0));
        body.apply_force(Vec2::new(100.0, 100.0));
        body.apply_torque(100.0);
        body.integrate_forces(Vec2::new(0.0, -10.0), 1.0);
        body.integrate_positions(1.0);
        assert!((body.linear_velocity.x).abs() < 1e-6);
        assert!((body.linear_velocity.y).abs() < 1e-6);
        assert!((body.position.x - 5.0).abs() < 1e-6);
        assert!((body.position.y - 5.0).abs() < 1e-6);
    }

    #[test]
    fn kinematic_body_uses_set_velocity() {
        let mut body = RigidBodyData::new_kinematic(Vec2::ZERO);
        body.apply_force(Vec2::new(100.0, 0.0));
        body.integrate_forces(Vec2::new(0.0, -10.0), 1.0);
        // Forces ignored
        assert!((body.linear_velocity.x).abs() < 1e-6);

        // Externally set velocity
        body.linear_velocity = Vec2::new(5.0, 0.0);
        body.integrate_positions(1.0);
        assert!((body.position.x - 5.0).abs() < 1e-6);
    }

    #[test]
    fn apply_impulse() {
        let mut body = RigidBodyData::new_dynamic(Vec2::ZERO, 4.0, 1.0);
        body.apply_impulse(Vec2::new(8.0, 0.0));
        // v = impulse * inv_mass = 8 * 0.25 = 2
        assert!((body.linear_velocity.x - 2.0).abs() < 1e-6);
    }

    #[test]
    fn apply_impulse_at_point() {
        let mut body = RigidBodyData::new_dynamic(Vec2::ZERO, 1.0, 1.0);
        // Apply impulse (0,1) at point (1,0) -> angular = r.cross(impulse) = (1,0).cross(0,1) = 1
        body.apply_impulse_at_point(Vec2::new(0.0, 1.0), Vec2::new(1.0, 0.0));
        assert!((body.linear_velocity.y - 1.0).abs() < 1e-6);
        assert!((body.angular_velocity - 1.0).abs() < 1e-6);
    }

    #[test]
    fn free_fall_accuracy() {
        let gravity = Vec2::new(0.0, -9.81);
        let dt = 0.01;
        let steps = 100;
        let mut body = RigidBodyData::new_dynamic(Vec2::ZERO, 1.0, 1.0);

        for _ in 0..steps {
            body.integrate_forces(gravity, dt);
            body.integrate_positions(dt);
        }

        let t = dt * steps as f32;
        let expected_y = 0.5 * (-9.81) * t * t;
        // Semi-implicit Euler accumulates O(dt) drift; allow proportional margin
        assert!(
            (body.position.y - expected_y).abs() < 1e-1,
            "y={} expected={}",
            body.position.y,
            expected_y
        );
    }

    #[test]
    fn mass_circle() {
        let (m, i) = mass::circle(1.0, 1.0);
        assert!((m - std::f32::consts::PI).abs() < 1e-4);
        assert!((i - std::f32::consts::PI * 0.5).abs() < 1e-4);
    }

    #[test]
    fn mass_rectangle() {
        let (m, i) = mass::rectangle(1.0, 1.0, 0.5);
        // 2x1 rect, area=2, m=2. I = 2/12*(4+1) = 10/12
        assert!((m - 2.0).abs() < 1e-4);
        assert!((i - 10.0 / 12.0).abs() < 1e-4);
    }

    #[test]
    fn velocity_at_point_includes_angular() {
        let mut body = RigidBodyData::new_dynamic(Vec2::ZERO, 1.0, 1.0);
        body.linear_velocity = Vec2::new(1.0, 0.0);
        body.angular_velocity = 2.0;
        // At point (0, 1): v = (1,0) + 2 * (-1, 0) ... wait
        // v = v_cm + (-omega*r.y, omega*r.x) = (1, 0) + (-2*1, 2*0) = (-1, 0)
        let v = body.velocity_at_point(Vec2::new(0.0, 1.0));
        assert!((v.x - (-1.0)).abs() < 1e-6);
        assert!((v.y - 0.0).abs() < 1e-6);
    }

    #[test]
    fn damping_reduces_velocity() {
        let mut body = RigidBodyData::new_dynamic(Vec2::ZERO, 1.0, 1.0);
        body.linear_velocity = Vec2::new(10.0, 0.0);
        body.linear_damping = 1.0;
        body.integrate_forces(Vec2::ZERO, 1.0);
        // v *= 1/(1+1*1) = 0.5
        assert!((body.linear_velocity.x - 5.0).abs() < 1e-4);
    }
}
