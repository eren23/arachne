use arachne_math::{Rect, Vec2};

use crate::broadphase::SpatialHashGrid;
use crate::collider::{Collider, ColliderShape};
use crate::rigid_body::{BodyHandle, RigidBodyData};

/// Result of a raycast query.
#[derive(Clone, Debug)]
pub struct RayHit {
    pub point: Vec2,
    pub normal: Vec2,
    pub distance: f32,
    pub body_handle: BodyHandle,
}

/// Casts a ray through the physics world using DDA through the spatial hash grid.
///
/// Returns the closest hit, if any.
pub fn raycast(
    origin: Vec2,
    direction: Vec2,
    max_distance: f32,
    bodies: &[RigidBodyData],
    colliders: &[Option<Collider>],
    grid: &SpatialHashGrid,
) -> Option<RayHit> {
    let dir_len = direction.length();
    if dir_len < 1e-8 {
        return None;
    }
    let dir = direction * (1.0 / dir_len);

    // Compute the ray's AABB for a coarse query
    let end = origin + dir * max_distance;
    let ray_aabb = Rect::new(
        Vec2::new(origin.x.min(end.x), origin.y.min(end.y)),
        Vec2::new(origin.x.max(end.x), origin.y.max(end.y)),
    );

    let candidates = grid.query_aabb(ray_aabb);

    let mut closest: Option<RayHit> = None;

    for handle in candidates {
        let idx = handle.0 as usize;
        if idx >= bodies.len() {
            continue;
        }
        let body = &bodies[idx];
        let collider = match &colliders[idx] {
            Some(c) => c,
            None => continue,
        };

        if let Some(hit) = ray_shape(
            origin,
            dir,
            max_distance,
            body.position,
            body.rotation,
            collider,
            handle,
        ) {
            let dominated = closest.as_ref().map_or(false, |c| c.distance <= hit.distance);
            if !dominated {
                closest = Some(hit);
            }
        }
    }

    closest
}

/// Tests a ray against a single collider shape.
fn ray_shape(
    origin: Vec2,
    dir: Vec2,
    max_distance: f32,
    body_pos: Vec2,
    body_rot: f32,
    collider: &Collider,
    handle: BodyHandle,
) -> Option<RayHit> {
    let center = collider.world_center(body_pos, body_rot);
    match &collider.shape {
        ColliderShape::Circle { radius } => {
            ray_circle(origin, dir, max_distance, center, *radius, handle)
        }
        ColliderShape::AABB { half_extents } => {
            ray_aabb(origin, dir, max_distance, center, *half_extents, body_rot, handle)
        }
        ColliderShape::Polygon { vertices: _ } => {
            let world_verts = collider.world_vertices(body_pos, body_rot);
            ray_polygon(origin, dir, max_distance, &world_verts, handle)
        }
        ColliderShape::Capsule {
            half_height,
            radius,
        } => ray_capsule(
            origin,
            dir,
            max_distance,
            center,
            body_rot,
            *half_height,
            *radius,
            handle,
        ),
    }
}

fn ray_circle(
    origin: Vec2,
    dir: Vec2,
    max_distance: f32,
    center: Vec2,
    radius: f32,
    handle: BodyHandle,
) -> Option<RayHit> {
    let oc = origin - center;
    let a = dir.dot(dir);
    let b = 2.0 * oc.dot(dir);
    let c = oc.dot(oc) - radius * radius;
    let discriminant = b * b - 4.0 * a * c;

    if discriminant < 0.0 {
        return None;
    }

    let sqrt_d = discriminant.sqrt();
    let t = (-b - sqrt_d) / (2.0 * a);

    if t < 0.0 || t > max_distance {
        // Try the far intersection
        let t2 = (-b + sqrt_d) / (2.0 * a);
        if t2 < 0.0 || t2 > max_distance {
            return None;
        }
        let point = origin + dir * t2;
        let normal = (point - center).normalize();
        return Some(RayHit {
            point,
            normal,
            distance: t2,
            body_handle: handle,
        });
    }

    let point = origin + dir * t;
    let normal = (point - center).normalize();
    Some(RayHit {
        point,
        normal,
        distance: t,
        body_handle: handle,
    })
}

fn ray_aabb(
    origin: Vec2,
    dir: Vec2,
    max_distance: f32,
    center: Vec2,
    half_extents: Vec2,
    rotation: f32,
    handle: BodyHandle,
) -> Option<RayHit> {
    // Transform ray into AABB-local space
    let (sin, cos) = rotation.sin_cos();
    let delta = origin - center;
    let local_origin = Vec2::new(delta.x * cos + delta.y * sin, -delta.x * sin + delta.y * cos);
    let local_dir = Vec2::new(dir.x * cos + dir.y * sin, -dir.x * sin + dir.y * cos);

    let mut tmin = f32::MIN;
    let mut tmax = f32::MAX;
    let mut hit_normal_local = Vec2::ZERO;

    // X axis
    if local_dir.x.abs() < 1e-8 {
        if local_origin.x < -half_extents.x || local_origin.x > half_extents.x {
            return None;
        }
    } else {
        let inv_d = 1.0 / local_dir.x;
        let mut t1 = (-half_extents.x - local_origin.x) * inv_d;
        let mut t2 = (half_extents.x - local_origin.x) * inv_d;
        let mut n = Vec2::new(-1.0, 0.0);
        if t1 > t2 {
            std::mem::swap(&mut t1, &mut t2);
            n = Vec2::new(1.0, 0.0);
        }
        if t1 > tmin {
            tmin = t1;
            hit_normal_local = n;
        }
        tmax = tmax.min(t2);
        if tmin > tmax {
            return None;
        }
    }

    // Y axis
    if local_dir.y.abs() < 1e-8 {
        if local_origin.y < -half_extents.y || local_origin.y > half_extents.y {
            return None;
        }
    } else {
        let inv_d = 1.0 / local_dir.y;
        let mut t1 = (-half_extents.y - local_origin.y) * inv_d;
        let mut t2 = (half_extents.y - local_origin.y) * inv_d;
        let mut n = Vec2::new(0.0, -1.0);
        if t1 > t2 {
            std::mem::swap(&mut t1, &mut t2);
            n = Vec2::new(0.0, 1.0);
        }
        if t1 > tmin {
            tmin = t1;
            hit_normal_local = n;
        }
        tmax = tmax.min(t2);
        if tmin > tmax {
            return None;
        }
    }

    if tmin < 0.0 || tmin > max_distance {
        return None;
    }

    let point = origin + dir * tmin;
    // Transform normal back to world space
    let normal = Vec2::new(
        hit_normal_local.x * cos - hit_normal_local.y * sin,
        hit_normal_local.x * sin + hit_normal_local.y * cos,
    );

    Some(RayHit {
        point,
        normal,
        distance: tmin,
        body_handle: handle,
    })
}

fn ray_polygon(
    origin: Vec2,
    dir: Vec2,
    max_distance: f32,
    world_verts: &[Vec2],
    handle: BodyHandle,
) -> Option<RayHit> {
    let n = world_verts.len();
    let mut closest_t = max_distance;
    let mut hit: Option<RayHit> = None;

    for i in 0..n {
        let v0 = world_verts[i];
        let v1 = world_verts[(i + 1) % n];

        if let Some(t) = ray_segment_intersect(origin, dir, v0, v1) {
            if t >= 0.0 && t < closest_t {
                closest_t = t;
                let point = origin + dir * t;
                let edge = v1 - v0;
                let normal = Vec2::new(edge.y, -edge.x).normalize();
                // Ensure normal points toward ray origin
                if normal.dot(dir) > 0.0 {
                    hit = Some(RayHit {
                        point,
                        normal: -normal,
                        distance: t,
                        body_handle: handle,
                    });
                } else {
                    hit = Some(RayHit {
                        point,
                        normal,
                        distance: t,
                        body_handle: handle,
                    });
                }
            }
        }
    }

    hit
}

fn ray_segment_intersect(origin: Vec2, dir: Vec2, a: Vec2, b: Vec2) -> Option<f32> {
    let v = b - a;
    let denom = dir.cross(v);
    if denom.abs() < 1e-10 {
        return None; // Parallel
    }
    let w = a - origin;
    let t = w.cross(v) / denom;
    let u = w.cross(dir) / denom;

    if t >= 0.0 && u >= 0.0 && u <= 1.0 {
        Some(t)
    } else {
        None
    }
}

fn ray_capsule(
    origin: Vec2,
    dir: Vec2,
    max_distance: f32,
    center: Vec2,
    rotation: f32,
    half_height: f32,
    radius: f32,
    handle: BodyHandle,
) -> Option<RayHit> {
    // Test against the two end circles and the rectangular body
    let cap_dir = Vec2::new(0.0, half_height).rotate(rotation);
    let a = center - cap_dir;
    let b = center + cap_dir;

    let mut best: Option<RayHit> = None;

    // Test circles at endpoints
    for &c in &[a, b] {
        if let Some(hit) = ray_circle(origin, dir, max_distance, c, radius, handle) {
            let dominated = best.as_ref().map_or(false, |bh| bh.distance <= hit.distance);
            if !dominated {
                best = Some(hit);
            }
        }
    }

    // Test the rectangular body (expanded line segment)
    let perp = Vec2::new(-cap_dir.y, cap_dir.x).normalize() * radius;
    let rect_verts = vec![
        a + perp,
        b + perp,
        b - perp,
        a - perp,
    ];
    if let Some(hit) = ray_polygon(origin, dir, max_distance, &rect_verts, handle) {
        let dominated = best.as_ref().map_or(false, |bh| bh.distance <= hit.distance);
        if !dominated {
            best = Some(hit);
        }
    }

    best
}

/// Queries all bodies whose AABBs overlap the given rectangle.
pub fn query_aabb(
    region: Rect,
    bodies: &[RigidBodyData],
    colliders: &[Option<Collider>],
    grid: &SpatialHashGrid,
) -> Vec<BodyHandle> {
    let candidates = grid.query_aabb(region);
    let mut results = Vec::new();

    for handle in candidates {
        let idx = handle.0 as usize;
        if idx >= bodies.len() {
            continue;
        }
        if let Some(col) = &colliders[idx] {
            let aabb = col.compute_aabb(bodies[idx].position, bodies[idx].rotation);
            let body_rect = Rect::new(aabb.min, aabb.max);
            if region.intersects(body_rect) {
                results.push(handle);
            }
        }
    }

    results
}

/// Queries which body contains the given point.
pub fn query_point(
    point: Vec2,
    bodies: &[RigidBodyData],
    colliders: &[Option<Collider>],
    grid: &SpatialHashGrid,
) -> Option<BodyHandle> {
    let candidates = grid.query_point(point);

    for handle in candidates {
        let idx = handle.0 as usize;
        if idx >= bodies.len() {
            continue;
        }
        let body = &bodies[idx];
        let collider = match &colliders[idx] {
            Some(c) => c,
            None => continue,
        };
        if point_in_shape(point, body.position, body.rotation, collider) {
            return Some(handle);
        }
    }

    None
}

fn point_in_shape(
    point: Vec2,
    body_pos: Vec2,
    body_rot: f32,
    collider: &Collider,
) -> bool {
    let center = collider.world_center(body_pos, body_rot);
    match &collider.shape {
        ColliderShape::Circle { radius } => {
            (point - center).length_squared() <= radius * radius
        }
        ColliderShape::AABB { half_extents } => {
            // Transform point to local space
            let (sin, cos) = body_rot.sin_cos();
            let delta = point - center;
            let local = Vec2::new(delta.x * cos + delta.y * sin, -delta.x * sin + delta.y * cos);
            local.x.abs() <= half_extents.x && local.y.abs() <= half_extents.y
        }
        ColliderShape::Polygon { vertices: _ } => {
            let world_verts = collider.world_vertices(body_pos, body_rot);
            point_in_convex(&world_verts, point)
        }
        ColliderShape::Capsule {
            half_height,
            radius,
        } => {
            let dir = Vec2::new(0.0, *half_height).rotate(body_rot);
            let a = center - dir;
            let b = center + dir;
            let closest = closest_point_on_segment(point, a, b);
            (point - closest).length_squared() <= radius * radius
        }
    }
}

fn point_in_convex(verts: &[Vec2], point: Vec2) -> bool {
    let n = verts.len();
    let mut sign = 0i32;
    for i in 0..n {
        let v0 = verts[i];
        let v1 = verts[(i + 1) % n];
        let cross = (v1 - v0).cross(point - v0);
        if cross > 1e-8 {
            if sign < 0 {
                return false;
            }
            sign = 1;
        } else if cross < -1e-8 {
            if sign > 0 {
                return false;
            }
            sign = -1;
        }
    }
    true
}

fn closest_point_on_segment(point: Vec2, a: Vec2, b: Vec2) -> Vec2 {
    let ab = b - a;
    let len_sq = ab.length_squared();
    if len_sq < 1e-12 {
        return a;
    }
    let t = ((point - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    a + ab * t
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::broadphase::SpatialHashGrid;
    use crate::collider::Collider;
    use crate::rigid_body::RigidBodyData;

    fn setup_circle_at(pos: Vec2, radius: f32) -> (Vec<RigidBodyData>, Vec<Option<Collider>>, SpatialHashGrid) {
        let bodies = vec![RigidBodyData::new_dynamic(pos, 1.0, 1.0)];
        let colliders: Vec<Option<Collider>> = vec![Some(Collider::circle(radius))];
        let mut grid = SpatialHashGrid::new(10.0);
        let aabb = colliders[0].as_ref().unwrap().compute_aabb(pos, 0.0);
        grid.insert(BodyHandle(0), aabb);
        (bodies, colliders, grid)
    }

    #[test]
    fn raycast_hit_circle() {
        let (bodies, colliders, grid) = setup_circle_at(Vec2::new(5.0, 0.0), 1.0);
        let hit = raycast(
            Vec2::ZERO,
            Vec2::new(1.0, 0.0),
            100.0,
            &bodies,
            &colliders,
            &grid,
        );
        let hit = hit.expect("should hit circle");
        assert!((hit.distance - 4.0).abs() < 1e-3, "dist={}", hit.distance);
        assert!((hit.point.x - 4.0).abs() < 1e-3);
        assert!((hit.normal.x - (-1.0)).abs() < 1e-3);
    }

    #[test]
    fn raycast_miss() {
        let (bodies, colliders, grid) = setup_circle_at(Vec2::new(5.0, 0.0), 1.0);
        let hit = raycast(
            Vec2::ZERO,
            Vec2::new(0.0, 1.0), // Shooting up, circle is to the right
            100.0,
            &bodies,
            &colliders,
            &grid,
        );
        assert!(hit.is_none());
    }

    #[test]
    fn raycast_closest_with_multiple_bodies() {
        let bodies = vec![
            RigidBodyData::new_dynamic(Vec2::new(5.0, 0.0), 1.0, 1.0),
            RigidBodyData::new_dynamic(Vec2::new(10.0, 0.0), 1.0, 1.0),
        ];
        let colliders: Vec<Option<Collider>> = vec![
            Some(Collider::circle(1.0)),
            Some(Collider::circle(1.0)),
        ];
        let mut grid = SpatialHashGrid::new(10.0);
        for (i, (body, col)) in bodies.iter().zip(colliders.iter()).enumerate() {
            if let Some(c) = col {
                let aabb = c.compute_aabb(body.position, body.rotation);
                grid.insert(BodyHandle(i as u32), aabb);
            }
        }

        let hit = raycast(
            Vec2::ZERO,
            Vec2::new(1.0, 0.0),
            100.0,
            &bodies,
            &colliders,
            &grid,
        );
        let hit = hit.expect("should hit");
        assert_eq!(hit.body_handle, BodyHandle(0)); // Closer body
        assert!((hit.distance - 4.0).abs() < 1e-3);
    }

    #[test]
    fn aabb_query_returns_all_in_region() {
        let bodies = vec![
            RigidBodyData::new_dynamic(Vec2::new(5.0, 5.0), 1.0, 1.0),
            RigidBodyData::new_dynamic(Vec2::new(50.0, 50.0), 1.0, 1.0),
            RigidBodyData::new_dynamic(Vec2::new(8.0, 8.0), 1.0, 1.0),
        ];
        let colliders: Vec<Option<Collider>> = vec![
            Some(Collider::circle(1.0)),
            Some(Collider::circle(1.0)),
            Some(Collider::circle(1.0)),
        ];
        let mut grid = SpatialHashGrid::new(10.0);
        for (i, (body, col)) in bodies.iter().zip(colliders.iter()).enumerate() {
            if let Some(c) = col {
                let aabb = c.compute_aabb(body.position, body.rotation);
                grid.insert(BodyHandle(i as u32), aabb);
            }
        }

        let region = Rect::new(Vec2::new(0.0, 0.0), Vec2::new(12.0, 12.0));
        let results = query_aabb(region, &bodies, &colliders, &grid);

        assert!(results.iter().any(|h| h.0 == 0));
        assert!(!results.iter().any(|h| h.0 == 1)); // Far away
        assert!(results.iter().any(|h| h.0 == 2));
    }

    #[test]
    fn aabb_query_none_outside() {
        let bodies = vec![RigidBodyData::new_dynamic(Vec2::new(5.0, 5.0), 1.0, 1.0)];
        let colliders: Vec<Option<Collider>> = vec![Some(Collider::circle(1.0))];
        let mut grid = SpatialHashGrid::new(10.0);
        let aabb = colliders[0].as_ref().unwrap().compute_aabb(Vec2::new(5.0, 5.0), 0.0);
        grid.insert(BodyHandle(0), aabb);

        let region = Rect::new(Vec2::new(100.0, 100.0), Vec2::new(200.0, 200.0));
        let results = query_aabb(region, &bodies, &colliders, &grid);
        assert!(results.is_empty());
    }

    #[test]
    fn point_query_inside_circle() {
        let (bodies, colliders, grid) = setup_circle_at(Vec2::new(5.0, 5.0), 2.0);
        let result = query_point(Vec2::new(5.0, 5.0), &bodies, &colliders, &grid);
        assert_eq!(result, Some(BodyHandle(0)));
    }

    #[test]
    fn point_query_outside() {
        let (bodies, colliders, grid) = setup_circle_at(Vec2::new(5.0, 5.0), 1.0);
        let result = query_point(Vec2::new(100.0, 100.0), &bodies, &colliders, &grid);
        assert!(result.is_none());
    }
}
