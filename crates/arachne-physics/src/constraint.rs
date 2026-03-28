use arachne_math::Vec2;

use crate::rigid_body::{BodyHandle, RigidBodyData};

/// A physics constraint.
#[derive(Clone, Debug)]
pub enum Constraint {
    Distance(DistanceConstraint),
    Revolute(RevoluteConstraint),
    Prismatic(PrismaticConstraint),
}

/// Maintains a fixed distance between two anchor points on two bodies.
#[derive(Clone, Debug)]
pub struct DistanceConstraint {
    pub body_a: BodyHandle,
    pub body_b: BodyHandle,
    /// Anchor point in body A's local space.
    pub local_anchor_a: Vec2,
    /// Anchor point in body B's local space.
    pub local_anchor_b: Vec2,
    /// Target distance.
    pub distance: f32,
    /// Accumulated impulse for warm starting.
    pub impulse: f32,
}

impl DistanceConstraint {
    pub fn new(
        body_a: BodyHandle,
        body_b: BodyHandle,
        local_anchor_a: Vec2,
        local_anchor_b: Vec2,
        distance: f32,
    ) -> Self {
        Self {
            body_a,
            body_b,
            local_anchor_a,
            local_anchor_b,
            distance,
            impulse: 0.0,
        }
    }

    fn world_anchor(body: &RigidBodyData, local_anchor: Vec2) -> Vec2 {
        body.position + local_anchor.rotate(body.rotation)
    }

    pub fn solve(&mut self, bodies: &mut [RigidBodyData], dt: f32) {
        let a = self.body_a.0 as usize;
        let b = self.body_b.0 as usize;

        let anchor_a = Self::world_anchor(&bodies[a], self.local_anchor_a);
        let anchor_b = Self::world_anchor(&bodies[b], self.local_anchor_b);

        let delta = anchor_b - anchor_a;
        let current_dist = delta.length();
        if current_dist < 1e-8 {
            return;
        }
        let normal = delta * (1.0 / current_dist);
        let error = current_dist - self.distance;

        let r_a = anchor_a - bodies[a].position;
        let r_b = anchor_b - bodies[b].position;

        let rn_a = r_a.cross(normal);
        let rn_b = r_b.cross(normal);
        let k = bodies[a].inv_mass
            + bodies[b].inv_mass
            + bodies[a].inv_inertia * rn_a * rn_a
            + bodies[b].inv_inertia * rn_b * rn_b;

        if k < 1e-12 {
            return;
        }

        let inv_dt = if dt > 0.0 { 1.0 / dt } else { 0.0 };
        let bias = 0.1 * inv_dt * error;

        // Relative velocity along normal
        let v_a = bodies[a].velocity_at_point(anchor_a);
        let v_b = bodies[b].velocity_at_point(anchor_b);
        let vn = (v_b - v_a).dot(normal);

        let lambda = -(vn + bias) / k;
        self.impulse += lambda;
        let impulse = normal * lambda;

        bodies[a].linear_velocity -= impulse * bodies[a].inv_mass;
        bodies[a].angular_velocity -= r_a.cross(impulse) * bodies[a].inv_inertia;
        bodies[b].linear_velocity += impulse * bodies[b].inv_mass;
        bodies[b].angular_velocity += r_b.cross(impulse) * bodies[b].inv_inertia;
    }
}

/// Pin joint: constrains two bodies to share a common world-space point.
#[derive(Clone, Debug)]
pub struct RevoluteConstraint {
    pub body_a: BodyHandle,
    pub body_b: BodyHandle,
    pub local_anchor_a: Vec2,
    pub local_anchor_b: Vec2,
    pub impulse: Vec2,
}

impl RevoluteConstraint {
    pub fn new(
        body_a: BodyHandle,
        body_b: BodyHandle,
        local_anchor_a: Vec2,
        local_anchor_b: Vec2,
    ) -> Self {
        Self {
            body_a,
            body_b,
            local_anchor_a,
            local_anchor_b,
            impulse: Vec2::ZERO,
        }
    }

    fn world_anchor(body: &RigidBodyData, local_anchor: Vec2) -> Vec2 {
        body.position + local_anchor.rotate(body.rotation)
    }

    pub fn solve(&mut self, bodies: &mut [RigidBodyData], dt: f32) {
        let a = self.body_a.0 as usize;
        let b = self.body_b.0 as usize;

        let anchor_a = Self::world_anchor(&bodies[a], self.local_anchor_a);
        let anchor_b = Self::world_anchor(&bodies[b], self.local_anchor_b);

        let r_a = anchor_a - bodies[a].position;
        let r_b = anchor_b - bodies[b].position;

        // Solve for each axis independently (2D)
        let inv_dt = if dt > 0.0 { 1.0 / dt } else { 0.0 };
        let bias = (anchor_b - anchor_a) * 0.1 * inv_dt;

        // Relative velocity at anchor points
        let v_a = bodies[a].velocity_at_point(anchor_a);
        let v_b = bodies[b].velocity_at_point(anchor_b);
        let dv = v_b - v_a;

        // 2x2 effective mass matrix (simplified: solve each axis independently)
        for axis in 0..2 {
            let n = if axis == 0 {
                Vec2::new(1.0, 0.0)
            } else {
                Vec2::new(0.0, 1.0)
            };

            let rn_a = r_a.cross(n);
            let rn_b = r_b.cross(n);
            let k = bodies[a].inv_mass
                + bodies[b].inv_mass
                + bodies[a].inv_inertia * rn_a * rn_a
                + bodies[b].inv_inertia * rn_b * rn_b;

            if k < 1e-12 {
                continue;
            }

            let cdot = dv.dot(n);
            let b_val = bias.dot(n);
            let lambda = -(cdot + b_val) / k;
            let impulse = n * lambda;

            bodies[a].linear_velocity -= impulse * bodies[a].inv_mass;
            bodies[a].angular_velocity -= r_a.cross(impulse) * bodies[a].inv_inertia;
            bodies[b].linear_velocity += impulse * bodies[b].inv_mass;
            bodies[b].angular_velocity += r_b.cross(impulse) * bodies[b].inv_inertia;

            if axis == 0 {
                self.impulse.x += lambda;
            } else {
                self.impulse.y += lambda;
            }
        }
    }
}

/// Constrains motion along a single axis; perpendicular motion is constrained.
#[derive(Clone, Debug)]
pub struct PrismaticConstraint {
    pub body_a: BodyHandle,
    pub body_b: BodyHandle,
    pub local_anchor_a: Vec2,
    pub local_anchor_b: Vec2,
    /// The constrained axis in body A's local space.
    pub local_axis: Vec2,
    pub impulse: f32,
}

impl PrismaticConstraint {
    pub fn new(
        body_a: BodyHandle,
        body_b: BodyHandle,
        local_anchor_a: Vec2,
        local_anchor_b: Vec2,
        local_axis: Vec2,
    ) -> Self {
        Self {
            body_a,
            body_b,
            local_anchor_a,
            local_anchor_b,
            local_axis: local_axis.normalize(),
            impulse: 0.0,
        }
    }

    fn world_anchor(body: &RigidBodyData, local_anchor: Vec2) -> Vec2 {
        body.position + local_anchor.rotate(body.rotation)
    }

    pub fn solve(&mut self, bodies: &mut [RigidBodyData], dt: f32) {
        let a = self.body_a.0 as usize;
        let b = self.body_b.0 as usize;

        let anchor_a = Self::world_anchor(&bodies[a], self.local_anchor_a);
        let anchor_b = Self::world_anchor(&bodies[b], self.local_anchor_b);

        // World-space axis and its perpendicular
        let axis = self.local_axis.rotate(bodies[a].rotation);
        let perp = Vec2::new(-axis.y, axis.x);

        let r_a = anchor_a - bodies[a].position;
        let r_b = anchor_b - bodies[b].position;

        // Constrain perpendicular displacement
        let delta = anchor_b - anchor_a;
        let error_perp = delta.dot(perp);

        let inv_dt = if dt > 0.0 { 1.0 / dt } else { 0.0 };
        let bias = 0.1 * inv_dt * error_perp;

        let rn_a = r_a.cross(perp);
        let rn_b = r_b.cross(perp);
        let k = bodies[a].inv_mass
            + bodies[b].inv_mass
            + bodies[a].inv_inertia * rn_a * rn_a
            + bodies[b].inv_inertia * rn_b * rn_b;

        if k < 1e-12 {
            return;
        }

        let v_a = bodies[a].velocity_at_point(anchor_a);
        let v_b = bodies[b].velocity_at_point(anchor_b);
        let cdot = (v_b - v_a).dot(perp);

        let lambda = -(cdot + bias) / k;
        self.impulse += lambda;
        let impulse = perp * lambda;

        bodies[a].linear_velocity -= impulse * bodies[a].inv_mass;
        bodies[a].angular_velocity -= r_a.cross(impulse) * bodies[a].inv_inertia;
        bodies[b].linear_velocity += impulse * bodies[b].inv_mass;
        bodies[b].angular_velocity += r_b.cross(impulse) * bodies[b].inv_inertia;
    }
}

impl Constraint {
    pub fn solve(&mut self, bodies: &mut [RigidBodyData], dt: f32) {
        match self {
            Constraint::Distance(c) => c.solve(bodies, dt),
            Constraint::Revolute(c) => c.solve(bodies, dt),
            Constraint::Prismatic(c) => c.solve(bodies, dt),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_constraint_maintains_distance() {
        let target_dist = 5.0;
        let mut bodies = vec![
            RigidBodyData::new_dynamic(Vec2::new(0.0, 0.0), 1.0, 1.0),
            RigidBodyData::new_dynamic(Vec2::new(6.0, 0.0), 1.0, 1.0),
        ];

        let mut constraint = DistanceConstraint::new(
            BodyHandle(0),
            BodyHandle(1),
            Vec2::ZERO,
            Vec2::ZERO,
            target_dist,
        );

        let dt = 1.0 / 60.0;
        // Run many iterations
        for _ in 0..100 {
            for _ in 0..8 {
                constraint.solve(&mut bodies, dt);
            }
            for body in bodies.iter_mut() {
                body.integrate_positions(dt);
            }
        }

        let actual_dist = (bodies[1].position - bodies[0].position).length();
        assert!(
            (actual_dist - target_dist).abs() < 1e-2,
            "Distance {} should be ~{}, diff={}",
            actual_dist,
            target_dist,
            (actual_dist - target_dist).abs()
        );
    }

    #[test]
    fn revolute_constraint_shared_point() {
        let mut bodies = vec![
            RigidBodyData::new_dynamic(Vec2::new(0.0, 0.0), 1.0, 1.0),
            RigidBodyData::new_dynamic(Vec2::new(2.0, 0.0), 1.0, 1.0),
        ];

        let mut constraint = RevoluteConstraint::new(
            BodyHandle(0),
            BodyHandle(1),
            Vec2::new(1.0, 0.0),
            Vec2::new(-1.0, 0.0),
        );

        let dt = 1.0 / 60.0;
        for _ in 0..200 {
            for _ in 0..8 {
                constraint.solve(&mut bodies, dt);
            }
            for body in bodies.iter_mut() {
                body.integrate_positions(dt);
            }
        }

        let anchor_a = bodies[0].position + Vec2::new(1.0, 0.0).rotate(bodies[0].rotation);
        let anchor_b = bodies[1].position + Vec2::new(-1.0, 0.0).rotate(bodies[1].rotation);
        let diff = (anchor_a - anchor_b).length();
        assert!(
            diff < 0.1,
            "Revolute anchors should converge, diff={}",
            diff
        );
    }

    #[test]
    fn prismatic_constraint_constrains_perpendicular() {
        let mut bodies = vec![
            RigidBodyData::new_static(Vec2::ZERO),
            RigidBodyData::new_dynamic(Vec2::new(2.0, 1.0), 1.0, 1.0),
        ];
        // Constrain along x axis — body B should only slide along x
        bodies[1].linear_velocity = Vec2::new(0.0, 5.0); // Perpendicular velocity

        let mut constraint = PrismaticConstraint::new(
            BodyHandle(0),
            BodyHandle(1),
            Vec2::ZERO,
            Vec2::ZERO,
            Vec2::new(1.0, 0.0),
        );

        let dt = 1.0 / 60.0;
        for _ in 0..100 {
            for _ in 0..8 {
                constraint.solve(&mut bodies, dt);
            }
            for body in bodies.iter_mut() {
                body.integrate_positions(dt);
            }
        }

        // Perpendicular velocity should have been absorbed by the constraint
        assert!(
            bodies[1].linear_velocity.y.abs() < 1.0,
            "Perpendicular velocity should be constrained, vy={}",
            bodies[1].linear_velocity.y
        );
    }
}
