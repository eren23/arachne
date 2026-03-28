use arachne_math::Vec2;

use crate::collider::ColliderShape;
use crate::rigid_body::BodyHandle;

/// A contact manifold produced by narrow-phase collision detection.
#[derive(Clone, Debug)]
pub struct ContactManifold {
    pub body_a: BodyHandle,
    pub body_b: BodyHandle,
    /// Collision normal pointing from A to B.
    pub normal: Vec2,
    /// Penetration depth (positive = overlapping).
    pub depth: f32,
    /// Contact points in world space (1 or 2).
    pub points: [Vec2; 2],
    pub point_count: usize,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Tests two colliders for collision and returns a contact manifold if colliding.
///
/// `pos_a/rot_a` and `pos_b/rot_b` are world-space position+rotation of each body.
/// The `offset_a/offset_b` are the collider offsets from body center.
pub fn test_collision(
    handle_a: BodyHandle,
    shape_a: &ColliderShape,
    pos_a: Vec2,
    rot_a: f32,
    offset_a: Vec2,
    handle_b: BodyHandle,
    shape_b: &ColliderShape,
    pos_b: Vec2,
    rot_b: f32,
    offset_b: Vec2,
) -> Option<ContactManifold> {
    let center_a = pos_a + offset_a.rotate(rot_a);
    let center_b = pos_b + offset_b.rotate(rot_b);

    let result = match (shape_a, shape_b) {
        (ColliderShape::Circle { radius: ra }, ColliderShape::Circle { radius: rb }) => {
            circle_circle(center_a, *ra, center_b, *rb)
        }
        (ColliderShape::Circle { radius }, ColliderShape::AABB { half_extents }) => {
            circle_aabb(center_a, *radius, center_b, *half_extents, rot_b)
        }
        (ColliderShape::AABB { half_extents }, ColliderShape::Circle { radius }) => {
            circle_aabb(center_b, *radius, center_a, *half_extents, rot_a).map(|mut m| {
                m.normal = -m.normal;
                m
            })
        }
        (
            ColliderShape::AABB {
                half_extents: he_a,
            },
            ColliderShape::AABB {
                half_extents: he_b,
            },
        ) => aabb_aabb(center_a, *he_a, rot_a, center_b, *he_b, rot_b),
        (ColliderShape::Circle { radius }, ColliderShape::Polygon { vertices }) => {
            circle_polygon(center_a, *radius, center_b, rot_b, vertices)
        }
        (ColliderShape::Polygon { vertices }, ColliderShape::Circle { radius }) => {
            circle_polygon(center_b, *radius, center_a, rot_a, vertices).map(|mut m| {
                m.normal = -m.normal;
                m
            })
        }
        (ColliderShape::Circle { radius }, ColliderShape::Capsule { half_height, radius: cap_r }) => {
            circle_capsule(center_a, *radius, center_b, rot_b, *half_height, *cap_r)
        }
        (ColliderShape::Capsule { half_height, radius: cap_r }, ColliderShape::Circle { radius }) => {
            circle_capsule(center_b, *radius, center_a, rot_a, *half_height, *cap_r).map(|mut m| {
                m.normal = -m.normal;
                m
            })
        }
        (ColliderShape::AABB { half_extents }, ColliderShape::Polygon { vertices }) => {
            aabb_polygon(center_a, *half_extents, rot_a, center_b, rot_b, vertices)
        }
        (ColliderShape::Polygon { vertices }, ColliderShape::AABB { half_extents }) => {
            aabb_polygon(center_b, *half_extents, rot_b, center_a, rot_a, vertices).map(|mut m| {
                m.normal = -m.normal;
                m
            })
        }
        (
            ColliderShape::Polygon {
                vertices: verts_a,
            },
            ColliderShape::Polygon {
                vertices: verts_b,
            },
        ) => polygon_polygon(center_a, rot_a, verts_a, center_b, rot_b, verts_b),
        (ColliderShape::Capsule { half_height: hh_a, radius: r_a }, ColliderShape::Capsule { half_height: hh_b, radius: r_b }) => {
            capsule_capsule(center_a, rot_a, *hh_a, *r_a, center_b, rot_b, *hh_b, *r_b)
        }
        (ColliderShape::AABB { half_extents }, ColliderShape::Capsule { half_height, radius: cap_r }) => {
            aabb_capsule(center_a, *half_extents, rot_a, center_b, rot_b, *half_height, *cap_r)
        }
        (ColliderShape::Capsule { half_height, radius: cap_r }, ColliderShape::AABB { half_extents }) => {
            aabb_capsule(center_b, *half_extents, rot_b, center_a, rot_a, *half_height, *cap_r).map(|mut m| {
                m.normal = -m.normal;
                m
            })
        }
        (ColliderShape::Polygon { vertices }, ColliderShape::Capsule { half_height, radius: cap_r }) => {
            polygon_capsule(center_a, rot_a, vertices, center_b, rot_b, *half_height, *cap_r)
        }
        (ColliderShape::Capsule { half_height, radius: cap_r }, ColliderShape::Polygon { vertices }) => {
            polygon_capsule(center_b, rot_b, vertices, center_a, rot_a, *half_height, *cap_r).map(|mut m| {
                m.normal = -m.normal;
                m
            })
        }
    };

    result.map(|mut m| {
        m.body_a = handle_a;
        m.body_b = handle_b;
        m
    })
}

// ---------------------------------------------------------------------------
// Circle-Circle
// ---------------------------------------------------------------------------

fn circle_circle(
    center_a: Vec2,
    radius_a: f32,
    center_b: Vec2,
    radius_b: f32,
) -> Option<ContactManifold> {
    let delta = center_b - center_a;
    let dist_sq = delta.length_squared();
    let sum_r = radius_a + radius_b;

    if dist_sq >= sum_r * sum_r {
        return None;
    }

    let dist = dist_sq.sqrt();
    let normal = if dist > 1e-8 {
        delta * (1.0 / dist)
    } else {
        Vec2::X
    };

    let depth = sum_r - dist;
    let contact = center_a + normal * radius_a;

    Some(ContactManifold {
        body_a: BodyHandle(0),
        body_b: BodyHandle(0),
        normal,
        depth,
        points: [contact, Vec2::ZERO],
        point_count: 1,
    })
}

// ---------------------------------------------------------------------------
// Circle-AABB
// ---------------------------------------------------------------------------

fn circle_aabb(
    circle_center: Vec2,
    radius: f32,
    aabb_center: Vec2,
    half_extents: Vec2,
    aabb_rot: f32,
) -> Option<ContactManifold> {
    // Transform circle center into AABB-local space
    let delta = circle_center - aabb_center;
    let (sin, cos) = aabb_rot.sin_cos();
    let local = Vec2::new(delta.x * cos + delta.y * sin, -delta.x * sin + delta.y * cos);

    // Closest point on AABB to circle center (clamped)
    let closest_local = Vec2::new(
        local.x.clamp(-half_extents.x, half_extents.x),
        local.y.clamp(-half_extents.y, half_extents.y),
    );

    let diff = local - closest_local;
    let dist_sq = diff.length_squared();

    if dist_sq >= radius * radius {
        return None;
    }

    let dist = dist_sq.sqrt();

    // Transform normal back to world space
    let (normal_local, depth) = if dist > 1e-8 {
        (diff * (1.0 / dist), radius - dist)
    } else {
        // Circle center is inside the AABB — find minimum penetration axis
        let dx = half_extents.x - local.x.abs();
        let dy = half_extents.y - local.y.abs();
        if dx < dy {
            let sign = if local.x >= 0.0 { 1.0 } else { -1.0 };
            (Vec2::new(sign, 0.0), dx + radius)
        } else {
            let sign = if local.y >= 0.0 { 1.0 } else { -1.0 };
            (Vec2::new(0.0, sign), dy + radius)
        }
    };

    // Rotate normal back to world space
    let normal = Vec2::new(
        normal_local.x * cos - normal_local.y * sin,
        normal_local.x * sin + normal_local.y * cos,
    );

    let contact = circle_center - normal * radius;

    Some(ContactManifold {
        body_a: BodyHandle(0),
        body_b: BodyHandle(0),
        normal: -normal, // Normal from A (circle) to B (AABB) — we want A->B
        depth,
        points: [contact, Vec2::ZERO],
        point_count: 1,
    })
}

// ---------------------------------------------------------------------------
// AABB-AABB
// ---------------------------------------------------------------------------

fn aabb_aabb(
    center_a: Vec2,
    he_a: Vec2,
    rot_a: f32,
    center_b: Vec2,
    he_b: Vec2,
    rot_b: f32,
) -> Option<ContactManifold> {
    // For rotated AABBs, fall through to polygon-polygon via SAT
    if rot_a.abs() > 1e-8 || rot_b.abs() > 1e-8 {
        let verts_a = aabb_to_verts(he_a);
        let verts_b = aabb_to_verts(he_b);
        return polygon_polygon(center_a, rot_a, &verts_a, center_b, rot_b, &verts_b);
    }

    // Axis-aligned case: simple overlap test
    let min_a = center_a - he_a;
    let max_a = center_a + he_a;
    let min_b = center_b - he_b;
    let max_b = center_b + he_b;

    let overlap_x = (max_a.x.min(max_b.x)) - (min_a.x.max(min_b.x));
    let overlap_y = (max_a.y.min(max_b.y)) - (min_a.y.max(min_b.y));

    if overlap_x <= 0.0 || overlap_y <= 0.0 {
        return None;
    }

    let (normal, depth) = if overlap_x < overlap_y {
        let sign = if center_b.x > center_a.x { 1.0 } else { -1.0 };
        (Vec2::new(sign, 0.0), overlap_x)
    } else {
        let sign = if center_b.y > center_a.y { 1.0 } else { -1.0 };
        (Vec2::new(0.0, sign), overlap_y)
    };

    // Contact point: center of overlap region
    let overlap_min = Vec2::new(min_a.x.max(min_b.x), min_a.y.max(min_b.y));
    let overlap_max = Vec2::new(max_a.x.min(max_b.x), max_a.y.min(max_b.y));
    let contact = (overlap_min + overlap_max) * 0.5;

    Some(ContactManifold {
        body_a: BodyHandle(0),
        body_b: BodyHandle(0),
        normal,
        depth,
        points: [contact, Vec2::ZERO],
        point_count: 1,
    })
}

fn aabb_to_verts(he: Vec2) -> Vec<Vec2> {
    vec![
        Vec2::new(-he.x, -he.y),
        Vec2::new(he.x, -he.y),
        Vec2::new(he.x, he.y),
        Vec2::new(-he.x, he.y),
    ]
}

// ---------------------------------------------------------------------------
// Circle-Polygon
// ---------------------------------------------------------------------------

fn circle_polygon(
    circle_center: Vec2,
    radius: f32,
    poly_center: Vec2,
    poly_rot: f32,
    vertices: &[Vec2],
) -> Option<ContactManifold> {
    let world_verts = transform_verts(vertices, poly_center, poly_rot);
    let n = world_verts.len();

    // Find the closest edge/vertex to the circle center
    let mut min_dist = f32::MAX;
    let mut best_normal = Vec2::ZERO;
    let mut best_point = Vec2::ZERO;

    for i in 0..n {
        let v0 = world_verts[i];
        let v1 = world_verts[(i + 1) % n];
        let edge = v1 - v0;
        let edge_len_sq = edge.length_squared();

        let (closest, normal_candidate);
        if edge_len_sq < 1e-12 {
            closest = v0;
            let d = circle_center - v0;
            let dlen = d.length();
            normal_candidate = if dlen > 1e-8 { d * (1.0 / dlen) } else { Vec2::X };
        } else {
            let t = ((circle_center - v0).dot(edge) / edge_len_sq).clamp(0.0, 1.0);
            closest = v0 + edge * t;
            let d = circle_center - closest;
            let dlen = d.length();
            normal_candidate = if dlen > 1e-8 { d * (1.0 / dlen) } else {
                // Edge normal (outward)
                let en = Vec2::new(edge.y, -edge.x).normalize();
                en
            };
        }

        let dist = (circle_center - closest).length();
        if dist < min_dist {
            min_dist = dist;
            best_normal = normal_candidate;
            best_point = closest;
        }
    }

    // Check if circle center is inside the polygon
    let inside = point_in_convex_polygon(circle_center, &world_verts);

    if inside {
        // Circle center is inside polygon
        let depth = radius + min_dist;
        let normal = -best_normal; // Point from polygon toward circle center, then flip for A->B
        let contact = best_point;
        Some(ContactManifold {
            body_a: BodyHandle(0),
            body_b: BodyHandle(0),
            normal, // circle is A, polygon is B — normal from circle toward polygon surface
            depth,
            points: [contact, Vec2::ZERO],
            point_count: 1,
        })
    } else if min_dist <= radius {
        let depth = radius - min_dist;
        let normal = -best_normal; // From circle to polygon
        let contact = best_point;
        Some(ContactManifold {
            body_a: BodyHandle(0),
            body_b: BodyHandle(0),
            normal,
            depth,
            points: [contact, Vec2::ZERO],
            point_count: 1,
        })
    } else {
        None
    }
}

fn point_in_convex_polygon(point: Vec2, verts: &[Vec2]) -> bool {
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

// ---------------------------------------------------------------------------
// Circle-Capsule
// ---------------------------------------------------------------------------

fn circle_capsule(
    circle_center: Vec2,
    radius: f32,
    cap_center: Vec2,
    cap_rot: f32,
    half_height: f32,
    cap_radius: f32,
) -> Option<ContactManifold> {
    // Capsule = line segment + radius
    let (seg_a, seg_b) = capsule_segment(cap_center, cap_rot, half_height);
    let closest = closest_point_on_segment(circle_center, seg_a, seg_b);
    circle_circle(circle_center, radius, closest, cap_radius)
}

// ---------------------------------------------------------------------------
// Capsule-Capsule
// ---------------------------------------------------------------------------

fn capsule_capsule(
    center_a: Vec2,
    rot_a: f32,
    hh_a: f32,
    r_a: f32,
    center_b: Vec2,
    rot_b: f32,
    hh_b: f32,
    r_b: f32,
) -> Option<ContactManifold> {
    let (a0, a1) = capsule_segment(center_a, rot_a, hh_a);
    let (b0, b1) = capsule_segment(center_b, rot_b, hh_b);
    let (ca, cb) = closest_points_segments(a0, a1, b0, b1);
    circle_circle(ca, r_a, cb, r_b)
}

// ---------------------------------------------------------------------------
// AABB-Polygon (use polygon-polygon)
// ---------------------------------------------------------------------------

fn aabb_polygon(
    aabb_center: Vec2,
    half_extents: Vec2,
    aabb_rot: f32,
    poly_center: Vec2,
    poly_rot: f32,
    vertices: &[Vec2],
) -> Option<ContactManifold> {
    let aabb_verts = aabb_to_verts(half_extents);
    polygon_polygon(aabb_center, aabb_rot, &aabb_verts, poly_center, poly_rot, vertices)
}

// ---------------------------------------------------------------------------
// AABB-Capsule
// ---------------------------------------------------------------------------

fn aabb_capsule(
    aabb_center: Vec2,
    half_extents: Vec2,
    aabb_rot: f32,
    cap_center: Vec2,
    cap_rot: f32,
    half_height: f32,
    cap_radius: f32,
) -> Option<ContactManifold> {
    let aabb_verts = aabb_to_verts(half_extents);
    polygon_capsule(aabb_center, aabb_rot, &aabb_verts, cap_center, cap_rot, half_height, cap_radius)
}

// ---------------------------------------------------------------------------
// Polygon-Capsule
// ---------------------------------------------------------------------------

fn polygon_capsule(
    poly_center: Vec2,
    poly_rot: f32,
    vertices: &[Vec2],
    cap_center: Vec2,
    cap_rot: f32,
    half_height: f32,
    cap_radius: f32,
) -> Option<ContactManifold> {
    // Closest point on capsule segment to polygon, then polygon vs circle
    let world_verts = transform_verts(vertices, poly_center, poly_rot);
    let (seg_a, seg_b) = capsule_segment(cap_center, cap_rot, half_height);

    // Find closest point between polygon edges and capsule segment
    let mut min_dist_sq = f32::MAX;
    let mut best_cap_pt = seg_a;
    let mut best_poly_pt = world_verts[0];
    let n = world_verts.len();

    for i in 0..n {
        let e0 = world_verts[i];
        let e1 = world_verts[(i + 1) % n];
        let (pa, pb) = closest_points_segments(e0, e1, seg_a, seg_b);
        let d = (pa - pb).length_squared();
        if d < min_dist_sq {
            min_dist_sq = d;
            best_poly_pt = pa;
            best_cap_pt = pb;
        }
    }

    // Also check if capsule endpoints are inside the polygon
    for &seg_pt in &[seg_a, seg_b] {
        if point_in_convex_polygon(seg_pt, &world_verts) {
            // Find closest edge
            for i in 0..n {
                let e0 = world_verts[i];
                let e1 = world_verts[(i + 1) % n];
                let cp = closest_point_on_segment(seg_pt, e0, e1);
                let d = (seg_pt - cp).length_squared();
                if d < min_dist_sq {
                    min_dist_sq = d;
                    best_poly_pt = cp;
                    best_cap_pt = seg_pt;
                }
            }
        }
    }

    let dist = min_dist_sq.sqrt();
    let inside = point_in_convex_polygon(best_cap_pt, &world_verts);

    if inside {
        let normal_dir = best_poly_pt - best_cap_pt;
        let nlen = normal_dir.length();
        let normal = if nlen > 1e-8 {
            normal_dir * (1.0 / nlen)
        } else {
            Vec2::X
        };
        let depth = dist + cap_radius;
        let contact = best_poly_pt;
        Some(ContactManifold {
            body_a: BodyHandle(0),
            body_b: BodyHandle(0),
            normal,
            depth,
            points: [contact, Vec2::ZERO],
            point_count: 1,
        })
    } else if dist < cap_radius {
        let normal_dir = best_cap_pt - best_poly_pt;
        let nlen = normal_dir.length();
        let normal = if nlen > 1e-8 {
            normal_dir * (1.0 / nlen)
        } else {
            Vec2::X
        };
        let depth = cap_radius - dist;
        let contact = best_poly_pt;
        Some(ContactManifold {
            body_a: BodyHandle(0),
            body_b: BodyHandle(0),
            normal: -normal, // poly is A, capsule is B
            depth,
            points: [contact, Vec2::ZERO],
            point_count: 1,
        })
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Polygon-Polygon: SAT (Separating Axis Theorem)
// ---------------------------------------------------------------------------

fn polygon_polygon(
    center_a: Vec2,
    rot_a: f32,
    verts_a: &[Vec2],
    center_b: Vec2,
    rot_b: f32,
    verts_b: &[Vec2],
) -> Option<ContactManifold> {
    let world_a = transform_verts(verts_a, center_a, rot_a);
    let world_b = transform_verts(verts_b, center_b, rot_b);

    // SAT: test all edge normals of both polygons
    let (depth_a, normal_a, idx_a) = find_min_separation(&world_a, &world_b)?;
    let (depth_b, normal_b, idx_b) = find_min_separation(&world_b, &world_a)?;

    let (normal, depth, ref_poly, inc_poly, _ref_idx) = if depth_a > depth_b {
        // A's separation is greater (less negative) = less overlap on that axis
        (-normal_a, -depth_a, &world_a, &world_b, idx_a)
    } else {
        (normal_b, -depth_b, &world_b, &world_a, idx_b)
    };

    // Ensure normal points from A to B
    let center_dir = (center_b - center_a).dot(normal);
    let final_normal = if center_dir < 0.0 { -normal } else { normal };

    // Clip to find contact points
    let contacts = clip_polygons(ref_poly, inc_poly, final_normal);

    if contacts.is_empty() {
        // Fallback: midpoint contact
        let mid = (center_a + center_b) * 0.5;
        Some(ContactManifold {
            body_a: BodyHandle(0),
            body_b: BodyHandle(0),
            normal: final_normal,
            depth,
            points: [mid, Vec2::ZERO],
            point_count: 1,
        })
    } else {
        let mut pts = [Vec2::ZERO; 2];
        let count = contacts.len().min(2);
        for i in 0..count {
            pts[i] = contacts[i];
        }
        Some(ContactManifold {
            body_a: BodyHandle(0),
            body_b: BodyHandle(0),
            normal: final_normal,
            depth,
            points: pts,
            point_count: count,
        })
    }
}

/// Find minimum separation (most-positive = least-penetrating) across edge normals of `poly`.
/// Returns None if a separating axis is found (no collision).
fn find_min_separation(poly: &[Vec2], other: &[Vec2]) -> Option<(f32, Vec2, usize)> {
    let n = poly.len();
    let mut max_sep = f32::MIN;
    let mut best_normal = Vec2::ZERO;
    let mut best_idx = 0;

    for i in 0..n {
        let v0 = poly[i];
        let v1 = poly[(i + 1) % n];
        let edge = v1 - v0;
        // Outward normal (assumes CCW winding; we check both)
        let normal = Vec2::new(edge.y, -edge.x).normalize();

        // Find the minimum projection of other polygon onto this normal relative to v0
        let mut min_proj = f32::MAX;
        for &ov in other {
            let proj = (ov - v0).dot(normal);
            if proj < min_proj {
                min_proj = proj;
            }
        }

        if min_proj > 0.0 {
            // Separating axis found
            return None;
        }

        if min_proj > max_sep {
            max_sep = min_proj;
            best_normal = normal;
            best_idx = i;
        }
    }

    Some((max_sep, best_normal, best_idx))
}

/// Sutherland-Hodgman-style contact clipping for polygon-polygon contacts.
fn clip_polygons(ref_poly: &[Vec2], inc_poly: &[Vec2], normal: Vec2) -> Vec<Vec2> {
    // Find the reference face: the edge most aligned with the normal
    let rn = ref_poly.len();
    let mut best_dot = f32::MIN;
    let mut ref_idx = 0;
    for i in 0..rn {
        let v0 = ref_poly[i];
        let v1 = ref_poly[(i + 1) % rn];
        let edge = v1 - v0;
        let face_normal = Vec2::new(edge.y, -edge.x).normalize();
        let d = face_normal.dot(normal);
        if d > best_dot {
            best_dot = d;
            ref_idx = i;
        }
    }

    let ref_v0 = ref_poly[ref_idx];
    let ref_v1 = ref_poly[(ref_idx + 1) % rn];
    let ref_edge = ref_v1 - ref_v0;
    let ref_normal = Vec2::new(ref_edge.y, -ref_edge.x).normalize();

    // Find incident face: edge of inc_poly most anti-aligned with ref_normal
    let in_ = inc_poly.len();
    let mut min_dot = f32::MAX;
    let mut inc_idx = 0;
    for i in 0..in_ {
        let v0 = inc_poly[i];
        let v1 = inc_poly[(i + 1) % in_];
        let edge = v1 - v0;
        let face_normal = Vec2::new(edge.y, -edge.x).normalize();
        let d = face_normal.dot(ref_normal);
        if d < min_dot {
            min_dot = d;
            inc_idx = i;
        }
    }

    let mut clip_pts = vec![inc_poly[inc_idx], inc_poly[(inc_idx + 1) % in_]];

    // Clip against side planes of reference face
    let tangent = ref_edge.normalize();

    // Side plane 1: points must be on the positive side of ref_v0 along tangent
    let d1 = tangent.dot(ref_v0);
    clip_pts = clip_segment_to_line(&clip_pts, tangent, d1);
    if clip_pts.len() < 2 {
        return clip_pts;
    }

    // Side plane 2: points must be on the negative side of ref_v1 along tangent
    let d2 = tangent.dot(ref_v1);
    clip_pts = clip_segment_to_line(&clip_pts, -tangent, -d2);
    if clip_pts.is_empty() {
        return clip_pts;
    }

    // Keep only points below the reference face
    let ref_d = ref_normal.dot(ref_v0);
    let mut contacts = Vec::new();
    for &p in &clip_pts {
        let sep = ref_normal.dot(p) - ref_d;
        if sep <= 0.0 {
            contacts.push(p);
        }
    }

    contacts
}

fn clip_segment_to_line(points: &[Vec2], normal: Vec2, d: f32) -> Vec<Vec2> {
    let mut out = Vec::new();
    let n = points.len();
    if n < 2 {
        return out;
    }

    let d0 = normal.dot(points[0]) - d;
    let d1 = normal.dot(points[1]) - d;

    if d0 >= 0.0 {
        out.push(points[0]);
    }
    if d1 >= 0.0 {
        out.push(points[1]);
    }

    if d0 * d1 < 0.0 {
        let t = d0 / (d0 - d1);
        out.push(points[0] + (points[1] - points[0]) * t);
    }

    out
}

// ---------------------------------------------------------------------------
// GJK / EPA (for general convex shapes and benchmark compliance)
// ---------------------------------------------------------------------------

/// GJK support function for a convex polygon.
pub fn support_polygon(vertices: &[Vec2], direction: Vec2) -> Vec2 {
    let mut best = vertices[0];
    let mut best_dot = best.dot(direction);
    for &v in &vertices[1..] {
        let d = v.dot(direction);
        if d > best_dot {
            best_dot = d;
            best = v;
        }
    }
    best
}

/// GJK support function for a circle.
#[inline]
pub fn support_circle(center: Vec2, radius: f32, direction: Vec2) -> Vec2 {
    let len = direction.length();
    if len > 1e-8 {
        center + direction * (radius / len)
    } else {
        center + Vec2::new(radius, 0.0)
    }
}

/// Minkowski difference support for two convex shapes.
#[inline]
fn support_diff(
    verts_a: &[Vec2],
    verts_b: &[Vec2],
    direction: Vec2,
) -> Vec2 {
    support_polygon(verts_a, direction) - support_polygon(verts_b, -direction)
}

/// GJK intersection test. Returns true if shapes overlap plus the simplex.
pub fn gjk_intersect(verts_a: &[Vec2], verts_b: &[Vec2]) -> (bool, Vec<Vec2>) {
    let mut direction = Vec2::new(1.0, 0.0);
    let mut simplex = Vec::new();

    let a = support_diff(verts_a, verts_b, direction);
    simplex.push(a);
    direction = -a;

    loop {
        let a = support_diff(verts_a, verts_b, direction);
        if a.dot(direction) < 0.0 {
            return (false, simplex);
        }
        simplex.push(a);

        match simplex.len() {
            2 => {
                // Line case
                let b = simplex[0];
                let a = simplex[1];
                let ab = b - a;
                let ao = -a;
                if ab.dot(ao) > 0.0 {
                    direction = triple_product(ab, ao, ab);
                    if direction.length_squared() < 1e-12 {
                        // Origin is on the line segment
                        direction = Vec2::new(-ab.y, ab.x);
                    }
                } else {
                    simplex = vec![a];
                    direction = ao;
                }
            }
            3 => {
                // Triangle case
                let c = simplex[0];
                let b = simplex[1];
                let a = simplex[2];
                let ab = b - a;
                let ac = c - a;
                let ao = -a;

                let ab_perp = triple_product(ac, ab, ab);
                let ac_perp = triple_product(ab, ac, ac);

                if ab_perp.dot(ao) > 0.0 {
                    // Region AB
                    simplex = vec![b, a];
                    direction = ab_perp;
                } else if ac_perp.dot(ao) > 0.0 {
                    // Region AC
                    simplex = vec![c, a];
                    direction = ac_perp;
                } else {
                    // Origin is inside the triangle
                    return (true, simplex);
                }
            }
            _ => unreachable!(),
        }
    }
}

/// EPA: expand the simplex to find the minimum penetration vector.
pub fn epa(
    verts_a: &[Vec2],
    verts_b: &[Vec2],
    simplex: &[Vec2],
) -> (Vec2, f32) {
    let mut polytope: Vec<Vec2> = simplex.to_vec();

    // Ensure CCW winding
    if polytope.len() >= 3 {
        let cross = (polytope[1] - polytope[0]).cross(polytope[2] - polytope[0]);
        if cross < 0.0 {
            polytope.swap(0, 1);
        }
    }

    const MAX_ITERATIONS: usize = 64;
    const TOLERANCE: f32 = 1e-6;

    for _ in 0..MAX_ITERATIONS {
        // Find closest edge to origin
        let n = polytope.len();
        let mut min_dist = f32::MAX;
        let mut min_idx = 0;
        let mut min_normal = Vec2::ZERO;

        for i in 0..n {
            let j = (i + 1) % n;
            let a = polytope[i];
            let b = polytope[j];
            let edge = b - a;
            // Outward normal (CCW winding)
            let normal = Vec2::new(edge.y, -edge.x).normalize();
            let dist = normal.dot(a);

            if dist < min_dist {
                min_dist = dist;
                min_idx = j;
                min_normal = normal;
            }
        }

        // Get new support point in direction of closest edge normal
        let support = support_diff(verts_a, verts_b, min_normal);
        let d = support.dot(min_normal);

        if d - min_dist < TOLERANCE {
            return (min_normal, min_dist);
        }

        polytope.insert(min_idx, support);
    }

    // Fallback
    let n = polytope.len();
    let mut min_dist = f32::MAX;
    let mut min_normal = Vec2::X;
    for i in 0..n {
        let j = (i + 1) % n;
        let edge = polytope[j] - polytope[i];
        let normal = Vec2::new(edge.y, -edge.x).normalize();
        let dist = normal.dot(polytope[i]);
        if dist < min_dist {
            min_dist = dist;
            min_normal = normal;
        }
    }
    (min_normal, min_dist)
}

/// GJK + EPA based collision for two convex polygons (used for benchmarks).
pub fn gjk_epa_collision(
    verts_a: &[Vec2],
    verts_b: &[Vec2],
) -> Option<(Vec2, f32)> {
    let (intersects, simplex) = gjk_intersect(verts_a, verts_b);
    if !intersects {
        return None;
    }
    let (normal, depth) = epa(verts_a, verts_b, &simplex);
    Some((normal, depth))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn transform_verts(verts: &[Vec2], center: Vec2, rotation: f32) -> Vec<Vec2> {
    let (sin, cos) = rotation.sin_cos();
    verts
        .iter()
        .map(|v| {
            let rotated = Vec2::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos);
            center + rotated
        })
        .collect()
}

/// Triple product: (a × b) × c = b(a·c) - a(b·c) (in 2D, returns a Vec2).
#[inline]
fn triple_product(a: Vec2, b: Vec2, c: Vec2) -> Vec2 {
    let ac = a.dot(c);
    let bc = b.dot(c);
    b * ac - a * bc
}

fn capsule_segment(center: Vec2, rotation: f32, half_height: f32) -> (Vec2, Vec2) {
    let dir = Vec2::new(0.0, half_height).rotate(rotation);
    (center - dir, center + dir)
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

fn closest_points_segments(a0: Vec2, a1: Vec2, b0: Vec2, b1: Vec2) -> (Vec2, Vec2) {
    let d1 = a1 - a0;
    let d2 = b1 - b0;
    let r = a0 - b0;

    let a = d1.dot(d1);
    let e = d2.dot(d2);
    let f = d2.dot(r);

    if a < 1e-12 && e < 1e-12 {
        return (a0, b0);
    }

    let (s, t);
    if a < 1e-12 {
        s = 0.0;
        t = (f / e).clamp(0.0, 1.0);
    } else {
        let c = d1.dot(r);
        if e < 1e-12 {
            t = 0.0;
            s = (-c / a).clamp(0.0, 1.0);
        } else {
            let b_val = d1.dot(d2);
            let denom = a * e - b_val * b_val;

            s = if denom.abs() > 1e-12 {
                ((b_val * f - c * e) / denom).clamp(0.0, 1.0)
            } else {
                0.0
            };

            t = ((b_val * s + f) / e).clamp(0.0, 1.0);
        }
    }

    (a0 + d1 * s, b0 + d2 * t)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rigid_body::BodyHandle;

    #[test]
    fn circle_circle_overlapping() {
        let m = circle_circle(Vec2::ZERO, 1.0, Vec2::new(1.5, 0.0), 1.0);
        let m = m.expect("should collide");
        assert!((m.depth - 0.5).abs() < 1e-4);
        assert!((m.normal.x - 1.0).abs() < 1e-4);
        assert!((m.normal.y).abs() < 1e-4);
    }

    #[test]
    fn circle_circle_touching() {
        let m = circle_circle(Vec2::ZERO, 1.0, Vec2::new(2.0, 0.0), 1.0);
        // Exactly touching — depth 0
        assert!(m.is_none() || m.unwrap().depth.abs() < 1e-4);
    }

    #[test]
    fn circle_circle_separated() {
        let m = circle_circle(Vec2::ZERO, 1.0, Vec2::new(5.0, 0.0), 1.0);
        assert!(m.is_none());
    }

    #[test]
    fn aabb_aabb_overlap() {
        let m = aabb_aabb(
            Vec2::ZERO,
            Vec2::new(2.0, 2.0),
            0.0,
            Vec2::new(3.0, 0.0),
            Vec2::new(2.0, 2.0),
            0.0,
        );
        let m = m.expect("should collide");
        assert!((m.depth - 1.0).abs() < 1e-4);
        assert!((m.normal.x - 1.0).abs() < 1e-4);
    }

    #[test]
    fn aabb_aabb_separated() {
        let m = aabb_aabb(
            Vec2::ZERO,
            Vec2::new(1.0, 1.0),
            0.0,
            Vec2::new(10.0, 0.0),
            Vec2::new(1.0, 1.0),
            0.0,
        );
        assert!(m.is_none());
    }

    #[test]
    fn aabb_aabb_corner_touch() {
        let m = aabb_aabb(
            Vec2::ZERO,
            Vec2::new(1.0, 1.0),
            0.0,
            Vec2::new(2.0, 2.0),
            Vec2::new(1.0, 1.0),
            0.0,
        );
        // Exactly touching at corner — depth 0 or no collision
        assert!(m.is_none() || m.unwrap().depth.abs() < 1e-4);
    }

    #[test]
    fn polygon_polygon_gjk_epa() {
        // Two overlapping squares
        let sq_a = vec![
            Vec2::new(-1.0, -1.0),
            Vec2::new(1.0, -1.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(-1.0, 1.0),
        ];
        let sq_b = vec![
            Vec2::new(-1.0, -1.0),
            Vec2::new(1.0, -1.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(-1.0, 1.0),
        ];

        // Transform B to be offset by 1.5 in x (overlap of 0.5 on x axis)
        let world_a: Vec<Vec2> = sq_a.iter().map(|v| *v).collect();
        let world_b: Vec<Vec2> = sq_b.iter().map(|v| *v + Vec2::new(1.5, 0.0)).collect();

        let result = gjk_epa_collision(&world_a, &world_b);
        let (normal, depth) = result.expect("should collide");
        assert!((depth - 0.5).abs() < 1e-2, "depth={}", depth);
        // Normal should be roughly along x axis
        assert!(normal.x.abs() > 0.9, "normal={:?}", normal);
    }

    #[test]
    fn polygon_polygon_separated() {
        let sq_a = vec![
            Vec2::new(-1.0, -1.0),
            Vec2::new(1.0, -1.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(-1.0, 1.0),
        ];
        let sq_b: Vec<Vec2> = sq_a.iter().map(|v| *v + Vec2::new(10.0, 0.0)).collect();
        let result = gjk_epa_collision(&sq_a, &sq_b);
        assert!(result.is_none());
    }

    #[test]
    fn polygon_polygon_sat_overlap() {
        let m = polygon_polygon(
            Vec2::ZERO,
            0.0,
            &[
                Vec2::new(-1.0, -1.0),
                Vec2::new(1.0, -1.0),
                Vec2::new(1.0, 1.0),
                Vec2::new(-1.0, 1.0),
            ],
            Vec2::new(1.5, 0.0),
            0.0,
            &[
                Vec2::new(-1.0, -1.0),
                Vec2::new(1.0, -1.0),
                Vec2::new(1.0, 1.0),
                Vec2::new(-1.0, 1.0),
            ],
        );
        let m = m.expect("should collide");
        assert!((m.depth - 0.5).abs() < 1e-2, "depth={}", m.depth);
    }

    #[test]
    fn circle_aabb_collision() {
        let m = test_collision(
            BodyHandle(0),
            &ColliderShape::Circle { radius: 1.0 },
            Vec2::ZERO,
            0.0,
            Vec2::ZERO,
            BodyHandle(1),
            &ColliderShape::AABB {
                half_extents: Vec2::new(1.0, 1.0),
            },
            Vec2::new(1.5, 0.0),
            0.0,
            Vec2::ZERO,
        );
        assert!(m.is_some());
    }

    #[test]
    fn test_collision_all_shape_pairs_separated() {
        let shapes: Vec<ColliderShape> = vec![
            ColliderShape::Circle { radius: 0.5 },
            ColliderShape::AABB {
                half_extents: Vec2::new(0.5, 0.5),
            },
            ColliderShape::Polygon {
                vertices: vec![
                    Vec2::new(-0.5, -0.5),
                    Vec2::new(0.5, -0.5),
                    Vec2::new(0.5, 0.5),
                    Vec2::new(-0.5, 0.5),
                ],
            },
            ColliderShape::Capsule {
                half_height: 0.5,
                radius: 0.3,
            },
        ];

        // All pairs far apart -> no collision
        for (i, sa) in shapes.iter().enumerate() {
            for (j, sb) in shapes.iter().enumerate() {
                let m = test_collision(
                    BodyHandle(i as u32),
                    sa,
                    Vec2::ZERO,
                    0.0,
                    Vec2::ZERO,
                    BodyHandle(j as u32),
                    sb,
                    Vec2::new(100.0, 100.0),
                    0.0,
                    Vec2::ZERO,
                );
                assert!(
                    m.is_none(),
                    "Expected no collision for shapes {} and {} at distance",
                    i,
                    j
                );
            }
        }
    }

    #[test]
    fn bench_gjk_polygon_polygon() {
        use std::hint::black_box;
        use std::time::Instant;

        let hex_a: Vec<Vec2> = (0..6)
            .map(|i| {
                let angle = i as f32 * std::f32::consts::TAU / 6.0;
                Vec2::new(angle.cos(), angle.sin())
            })
            .collect();
        let hex_b: Vec<Vec2> = hex_a.iter().map(|v| *v + Vec2::new(1.5, 0.0)).collect();

        let iterations = 100_000u64;
        let start = Instant::now();
        for _ in 0..iterations {
            let _ = black_box(gjk_epa_collision(
                black_box(&hex_a),
                black_box(&hex_b),
            ));
        }
        let elapsed = start.elapsed();
        let ops_per_sec = iterations as f64 / elapsed.as_secs_f64();
        eprintln!(
            "GJK polygon-polygon: {:.0}K ops/sec ({} iterations in {:.3}ms)",
            ops_per_sec / 1_000.0,
            iterations,
            elapsed.as_secs_f64() * 1000.0
        );
        assert!(
            ops_per_sec >= 500_000.0,
            "GJK throughput {:.0}K < 500K ops/sec",
            ops_per_sec / 1_000.0
        );
    }
}
