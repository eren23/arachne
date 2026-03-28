use arachne_math::Vec2;

use crate::narrowphase::ContactManifold;
use crate::rigid_body::RigidBodyData;

/// Cached impulse data for warm starting.
#[derive(Clone, Debug, Default)]
pub struct ContactCache {
    pub normal_impulse: f32,
    pub tangent_impulse: f32,
}

/// Velocity constraint for a single contact point.
#[derive(Clone, Debug)]
struct VelocityConstraint {
    body_a: u32,
    body_b: u32,
    #[allow(dead_code)]
    contact: Vec2,
    normal: Vec2,
    tangent: Vec2,
    r_a: Vec2,
    r_b: Vec2,
    normal_mass: f32,
    tangent_mass: f32,
    bias: f32,
    restitution_bias: f32,
    friction: f32,
    normal_impulse: f32,
    tangent_impulse: f32,
}

/// Sequential impulse constraint solver.
pub struct Solver {
    pub iterations: usize,
    /// Baumgarte stabilization factor.
    pub beta: f32,
    /// Penetration slop (allowed penetration before correction).
    pub slop: f32,
    velocity_constraints: Vec<VelocityConstraint>,
}

impl Default for Solver {
    fn default() -> Self {
        Self {
            iterations: 8,
            beta: 0.2,
            slop: 0.005,
            velocity_constraints: Vec::new(),
        }
    }
}

impl Solver {
    pub fn new(iterations: usize) -> Self {
        Self {
            iterations,
            ..Default::default()
        }
    }

    /// Prepares velocity constraints from contact manifolds.
    /// `restitution_values` must be the same length as `manifolds`.
    pub fn prepare(
        &mut self,
        bodies: &[RigidBodyData],
        manifolds: &[ContactManifold],
        friction_values: &[f32],
        restitution_values: &[f32],
        dt: f32,
        caches: &mut Vec<ContactCache>,
    ) {
        self.velocity_constraints.clear();
        let inv_dt = if dt > 0.0 { 1.0 / dt } else { 0.0 };

        // Ensure cache is large enough
        let total_contacts: usize = manifolds.iter().map(|m| m.point_count).sum();
        if caches.len() < total_contacts {
            caches.resize(total_contacts, ContactCache::default());
        }

        let mut cache_idx = 0;
        for (mi, manifold) in manifolds.iter().enumerate() {
            let a = manifold.body_a.0 as usize;
            let b = manifold.body_b.0 as usize;
            let body_a = &bodies[a];
            let body_b = &bodies[b];
            let friction = if mi < friction_values.len() {
                friction_values[mi]
            } else {
                0.3
            };
            let restitution = if mi < restitution_values.len() {
                restitution_values[mi]
            } else {
                0.0
            };

            for pi in 0..manifold.point_count {
                let contact = manifold.points[pi];
                let normal = manifold.normal;
                let tangent = Vec2::new(-normal.y, normal.x);
                let r_a = contact - body_a.position;
                let r_b = contact - body_b.position;

                // Effective mass along normal
                let rn_a = r_a.cross(normal);
                let rn_b = r_b.cross(normal);
                let k_normal = body_a.inv_mass
                    + body_b.inv_mass
                    + body_a.inv_inertia * rn_a * rn_a
                    + body_b.inv_inertia * rn_b * rn_b;
                let normal_mass = if k_normal > 0.0 { 1.0 / k_normal } else { 0.0 };

                // Effective mass along tangent
                let rt_a = r_a.cross(tangent);
                let rt_b = r_b.cross(tangent);
                let k_tangent = body_a.inv_mass
                    + body_b.inv_mass
                    + body_a.inv_inertia * rt_a * rt_a
                    + body_b.inv_inertia * rt_b * rt_b;
                let tangent_mass = if k_tangent > 0.0 { 1.0 / k_tangent } else { 0.0 };

                // Baumgarte bias for penetration correction
                let bias = self.beta * inv_dt * (manifold.depth - self.slop).max(0.0);

                // Restitution velocity bias
                let v_a = body_a.linear_velocity
                    + Vec2::new(-body_a.angular_velocity * r_a.y, body_a.angular_velocity * r_a.x);
                let v_b = body_b.linear_velocity
                    + Vec2::new(-body_b.angular_velocity * r_b.y, body_b.angular_velocity * r_b.x);
                let rel_vn = (v_b - v_a).dot(normal);
                let restitution_bias = if rel_vn < -1.0 {
                    -restitution * rel_vn
                } else {
                    0.0
                };

                let cached = &caches[cache_idx];

                self.velocity_constraints.push(VelocityConstraint {
                    body_a: manifold.body_a.0,
                    body_b: manifold.body_b.0,
                    contact,
                    normal,
                    tangent,
                    r_a,
                    r_b,
                    normal_mass,
                    tangent_mass,
                    bias,
                    restitution_bias,
                    friction,
                    normal_impulse: cached.normal_impulse,
                    tangent_impulse: cached.tangent_impulse,
                });

                cache_idx += 1;
            }
        }
    }

    /// Applies warm starting impulses from previous frame.
    pub fn warm_start(&self, bodies: &mut [RigidBodyData]) {
        for vc in &self.velocity_constraints {
            let impulse = vc.normal * vc.normal_impulse + vc.tangent * vc.tangent_impulse;
            let a = vc.body_a as usize;
            let b = vc.body_b as usize;

            bodies[a].linear_velocity -= impulse * bodies[a].inv_mass;
            bodies[a].angular_velocity -= vc.r_a.cross(impulse) * bodies[a].inv_inertia;
            bodies[b].linear_velocity += impulse * bodies[b].inv_mass;
            bodies[b].angular_velocity += vc.r_b.cross(impulse) * bodies[b].inv_inertia;
        }
    }

    /// Runs sequential impulse iterations to resolve contacts.
    pub fn solve(&mut self, bodies: &mut [RigidBodyData]) {
        for _ in 0..self.iterations {
            for vc in &mut self.velocity_constraints {
                let a = vc.body_a as usize;
                let b = vc.body_b as usize;

                // Relative velocity at contact point
                let v_a = bodies[a].linear_velocity
                    + Vec2::new(
                        -bodies[a].angular_velocity * vc.r_a.y,
                        bodies[a].angular_velocity * vc.r_a.x,
                    );
                let v_b = bodies[b].linear_velocity
                    + Vec2::new(
                        -bodies[b].angular_velocity * vc.r_b.y,
                        bodies[b].angular_velocity * vc.r_b.x,
                    );
                let dv = v_b - v_a;

                // Normal impulse with restitution
                let vn = dv.dot(vc.normal);
                let lambda_n = vc.normal_mass * (-vn + vc.bias + vc.restitution_bias);
                let old_impulse = vc.normal_impulse;
                vc.normal_impulse = (old_impulse + lambda_n).max(0.0);
                let impulse_n = vc.normal * (vc.normal_impulse - old_impulse);

                bodies[a].linear_velocity -= impulse_n * bodies[a].inv_mass;
                bodies[a].angular_velocity -= vc.r_a.cross(impulse_n) * bodies[a].inv_inertia;
                bodies[b].linear_velocity += impulse_n * bodies[b].inv_mass;
                bodies[b].angular_velocity += vc.r_b.cross(impulse_n) * bodies[b].inv_inertia;

                // Tangent (friction) impulse
                let v_a2 = bodies[a].linear_velocity
                    + Vec2::new(
                        -bodies[a].angular_velocity * vc.r_a.y,
                        bodies[a].angular_velocity * vc.r_a.x,
                    );
                let v_b2 = bodies[b].linear_velocity
                    + Vec2::new(
                        -bodies[b].angular_velocity * vc.r_b.y,
                        bodies[b].angular_velocity * vc.r_b.x,
                    );
                let dv2 = v_b2 - v_a2;
                let vt = dv2.dot(vc.tangent);
                let lambda_t = vc.tangent_mass * (-vt);
                let max_friction = vc.friction * vc.normal_impulse;
                let old_tangent = vc.tangent_impulse;
                vc.tangent_impulse = (old_tangent + lambda_t).clamp(-max_friction, max_friction);
                let impulse_t = vc.tangent * (vc.tangent_impulse - old_tangent);

                bodies[a].linear_velocity -= impulse_t * bodies[a].inv_mass;
                bodies[a].angular_velocity -= vc.r_a.cross(impulse_t) * bodies[a].inv_inertia;
                bodies[b].linear_velocity += impulse_t * bodies[b].inv_mass;
                bodies[b].angular_velocity += vc.r_b.cross(impulse_t) * bodies[b].inv_inertia;
            }
        }
    }

    /// Writes accumulated impulses back to the cache for warm starting next frame.
    pub fn store_impulses(&self, caches: &mut [ContactCache]) {
        for (i, vc) in self.velocity_constraints.iter().enumerate() {
            if i < caches.len() {
                caches[i].normal_impulse = vc.normal_impulse;
                caches[i].tangent_impulse = vc.tangent_impulse;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::narrowphase::ContactManifold;
    use crate::rigid_body::{BodyHandle, RigidBodyData};

    #[test]
    fn two_circles_separate_after_solve() {
        let mut bodies = vec![
            RigidBodyData::new_dynamic(Vec2::new(0.0, 0.0), 1.0, 1.0),
            RigidBodyData::new_dynamic(Vec2::new(1.5, 0.0), 1.0, 1.0),
        ];

        let manifold = ContactManifold {
            body_a: BodyHandle(0),
            body_b: BodyHandle(1),
            normal: Vec2::new(1.0, 0.0),
            depth: 0.5,
            points: [Vec2::new(0.75, 0.0), Vec2::ZERO],
            point_count: 1,
        };

        let dt = 1.0 / 60.0;
        let mut solver = Solver::new(8);
        let mut caches = vec![ContactCache::default(); 1];
        solver.prepare(&bodies, &[manifold], &[0.3], &[0.0], dt, &mut caches);
        solver.warm_start(&mut bodies);
        solver.solve(&mut bodies);

        // After solving, bodies should have separating velocities
        let rel_v = bodies[1].linear_velocity.x - bodies[0].linear_velocity.x;
        assert!(rel_v > 0.0, "Bodies should be separating, rel_v={}", rel_v);
    }

    #[test]
    fn static_body_not_affected() {
        let mut bodies = vec![
            RigidBodyData::new_dynamic(Vec2::new(0.0, 1.0), 1.0, 1.0),
            RigidBodyData::new_static(Vec2::ZERO),
        ];

        let manifold = ContactManifold {
            body_a: BodyHandle(0),
            body_b: BodyHandle(1),
            normal: Vec2::new(0.0, -1.0),
            depth: 0.1,
            points: [Vec2::new(0.0, 0.5), Vec2::ZERO],
            point_count: 1,
        };

        let dt = 1.0 / 60.0;
        let mut solver = Solver::new(8);
        let mut caches = vec![ContactCache::default(); 1];
        solver.prepare(&bodies, &[manifold], &[0.3], &[0.0], dt, &mut caches);
        solver.solve(&mut bodies);

        assert!((bodies[1].linear_velocity.x).abs() < 1e-6);
        assert!((bodies[1].linear_velocity.y).abs() < 1e-6);
    }

    #[test]
    fn restitution_bounces_back() {
        let mut bodies = vec![
            RigidBodyData::new_dynamic(Vec2::new(0.0, 0.0), 1.0, 1.0),
            RigidBodyData::new_static(Vec2::new(0.0, -1.0)),
        ];
        bodies[0].linear_velocity = Vec2::new(0.0, -10.0);

        let manifold = ContactManifold {
            body_a: BodyHandle(0),
            body_b: BodyHandle(1),
            normal: Vec2::new(0.0, -1.0),
            depth: 0.01,
            points: [Vec2::new(0.0, -0.5), Vec2::ZERO],
            point_count: 1,
        };

        let dt = 1.0 / 60.0;
        let mut solver = Solver::new(8);
        let mut caches = vec![ContactCache::default(); 1];
        solver.prepare(&bodies, &[manifold], &[0.0], &[1.0], dt, &mut caches);
        solver.solve(&mut bodies);

        // With restitution=1.0, should bounce back up
        assert!(
            bodies[0].linear_velocity.y > 5.0,
            "Should bounce back, vy={}",
            bodies[0].linear_velocity.y
        );
    }
}
