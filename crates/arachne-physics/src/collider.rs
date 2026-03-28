use arachne_math::{Rect, Vec2};
use crate::material::PhysicsMaterial;

/// Maximum number of vertices for a convex polygon collider.
pub const MAX_POLYGON_VERTICES: usize = 8;

/// The geometric shape of a collider.
#[derive(Clone, Debug, PartialEq)]
pub enum ColliderShape {
    Circle { radius: f32 },
    AABB { half_extents: Vec2 },
    Polygon { vertices: Vec<Vec2> },
    Capsule { half_height: f32, radius: f32 },
}

/// A collider attached to a rigid body: shape + offset + material.
#[derive(Clone, Debug)]
pub struct Collider {
    pub shape: ColliderShape,
    /// Local offset from the body's center of mass.
    pub offset: Vec2,
    pub material: PhysicsMaterial,
}

impl Collider {
    pub fn new(shape: ColliderShape, material: PhysicsMaterial) -> Self {
        Self {
            shape,
            offset: Vec2::ZERO,
            material,
        }
    }

    pub fn with_offset(mut self, offset: Vec2) -> Self {
        self.offset = offset;
        self
    }

    pub fn circle(radius: f32) -> Self {
        Self::new(ColliderShape::Circle { radius }, PhysicsMaterial::default())
    }

    pub fn aabb(half_extents: Vec2) -> Self {
        Self::new(
            ColliderShape::AABB { half_extents },
            PhysicsMaterial::default(),
        )
    }

    pub fn polygon(vertices: Vec<Vec2>) -> Self {
        assert!(vertices.len() >= 3, "Polygon must have at least 3 vertices");
        assert!(
            vertices.len() <= MAX_POLYGON_VERTICES,
            "Polygon cannot have more than {} vertices",
            MAX_POLYGON_VERTICES
        );
        Self::new(
            ColliderShape::Polygon { vertices },
            PhysicsMaterial::default(),
        )
    }

    pub fn capsule(half_height: f32, radius: f32) -> Self {
        Self::new(
            ColliderShape::Capsule {
                half_height,
                radius,
            },
            PhysicsMaterial::default(),
        )
    }

    /// Computes the world-space AABB for this collider given body position and rotation.
    pub fn compute_aabb(&self, body_pos: Vec2, body_rotation: f32) -> Rect {
        let center = body_pos + self.offset.rotate(body_rotation);
        match &self.shape {
            ColliderShape::Circle { radius } => {
                let r = Vec2::splat(*radius);
                Rect::new(center - r, center + r)
            }
            ColliderShape::AABB { half_extents } => {
                if body_rotation.abs() < 1e-8 {
                    Rect::new(center - *half_extents, center + *half_extents)
                } else {
                    // Rotated AABB: compute bounding box of the rotated rectangle
                    let (sin, cos) = body_rotation.sin_cos();
                    let abs_cos = cos.abs();
                    let abs_sin = sin.abs();
                    let new_hx = half_extents.x * abs_cos + half_extents.y * abs_sin;
                    let new_hy = half_extents.x * abs_sin + half_extents.y * abs_cos;
                    let new_half = Vec2::new(new_hx, new_hy);
                    Rect::new(center - new_half, center + new_half)
                }
            }
            ColliderShape::Polygon { vertices } => {
                let (sin, cos) = body_rotation.sin_cos();
                let mut min = Vec2::new(f32::MAX, f32::MAX);
                let mut max = Vec2::new(f32::MIN, f32::MIN);
                for v in vertices {
                    let rotated = Vec2::new(
                        v.x * cos - v.y * sin,
                        v.x * sin + v.y * cos,
                    );
                    let world = center + rotated;
                    min = min.min(world);
                    max = max.max(world);
                }
                Rect::new(min, max)
            }
            ColliderShape::Capsule {
                half_height,
                radius,
            } => {
                // Capsule is a line segment from (0, -half_height) to (0, half_height)
                // expanded by radius. When rotated:
                let (sin, cos) = body_rotation.sin_cos();
                let endpoint = Vec2::new(-(*half_height) * sin, *half_height * cos);
                let abs_end = endpoint.abs();
                let half = Vec2::new(abs_end.x + radius, abs_end.y + radius);
                Rect::new(center - half, center + half)
            }
        }
    }

    /// Returns world-space vertices for the polygon shape, rotated by body_rotation.
    pub fn world_vertices(&self, body_pos: Vec2, body_rotation: f32) -> Vec<Vec2> {
        let center = body_pos + self.offset.rotate(body_rotation);
        match &self.shape {
            ColliderShape::Polygon { vertices } => {
                let (sin, cos) = body_rotation.sin_cos();
                vertices
                    .iter()
                    .map(|v| {
                        let rotated = Vec2::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos);
                        center + rotated
                    })
                    .collect()
            }
            ColliderShape::AABB { half_extents } => {
                let hx = half_extents.x;
                let hy = half_extents.y;
                let local = [
                    Vec2::new(-hx, -hy),
                    Vec2::new(hx, -hy),
                    Vec2::new(hx, hy),
                    Vec2::new(-hx, hy),
                ];
                let (sin, cos) = body_rotation.sin_cos();
                local
                    .iter()
                    .map(|v| {
                        let rotated = Vec2::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos);
                        center + rotated
                    })
                    .collect()
            }
            _ => vec![],
        }
    }

    /// Returns the world-space center of this collider given body position and rotation.
    #[inline]
    pub fn world_center(&self, body_pos: Vec2, body_rotation: f32) -> Vec2 {
        body_pos + self.offset.rotate(body_rotation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn circle_aabb_at_origin() {
        let c = Collider::circle(2.0);
        let aabb = c.compute_aabb(Vec2::ZERO, 0.0);
        assert!((aabb.min.x - (-2.0)).abs() < 1e-6);
        assert!((aabb.min.y - (-2.0)).abs() < 1e-6);
        assert!((aabb.max.x - 2.0).abs() < 1e-6);
        assert!((aabb.max.y - 2.0).abs() < 1e-6);
    }

    #[test]
    fn circle_aabb_offset() {
        let c = Collider::circle(2.0);
        let aabb = c.compute_aabb(Vec2::new(5.0, 5.0), 0.0);
        assert!((aabb.min.x - 3.0).abs() < 1e-6);
        assert!((aabb.min.y - 3.0).abs() < 1e-6);
        assert!((aabb.max.x - 7.0).abs() < 1e-6);
        assert!((aabb.max.y - 7.0).abs() < 1e-6);
    }

    #[test]
    fn aabb_shape_no_rotation() {
        let c = Collider::aabb(Vec2::new(3.0, 2.0));
        let aabb = c.compute_aabb(Vec2::new(1.0, 1.0), 0.0);
        assert!((aabb.min.x - (-2.0)).abs() < 1e-6);
        assert!((aabb.min.y - (-1.0)).abs() < 1e-6);
        assert!((aabb.max.x - 4.0).abs() < 1e-6);
        assert!((aabb.max.y - 3.0).abs() < 1e-6);
    }

    #[test]
    fn aabb_shape_with_rotation() {
        // 2x2 AABB rotated 45 degrees should be larger
        let c = Collider::aabb(Vec2::new(1.0, 1.0));
        let aabb = c.compute_aabb(Vec2::ZERO, std::f32::consts::FRAC_PI_4);
        let sqrt2 = std::f32::consts::SQRT_2;
        assert!((aabb.max.x - sqrt2).abs() < 1e-4);
        assert!((aabb.max.y - sqrt2).abs() < 1e-4);
    }

    #[test]
    fn polygon_aabb() {
        // Triangle: (0,0), (2,0), (1,2)
        let c = Collider::polygon(vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(2.0, 0.0),
            Vec2::new(1.0, 2.0),
        ]);
        let aabb = c.compute_aabb(Vec2::ZERO, 0.0);
        assert!((aabb.min.x - 0.0).abs() < 1e-6);
        assert!((aabb.min.y - 0.0).abs() < 1e-6);
        assert!((aabb.max.x - 2.0).abs() < 1e-6);
        assert!((aabb.max.y - 2.0).abs() < 1e-6);
    }

    #[test]
    fn capsule_aabb() {
        let c = Collider::capsule(1.0, 0.5);
        let aabb = c.compute_aabb(Vec2::ZERO, 0.0);
        // Vertical capsule: half_height=1 + radius=0.5
        assert!((aabb.min.x - (-0.5)).abs() < 1e-6);
        assert!((aabb.min.y - (-1.5)).abs() < 1e-6);
        assert!((aabb.max.x - 0.5).abs() < 1e-6);
        assert!((aabb.max.y - 1.5).abs() < 1e-6);
    }

    #[test]
    fn world_vertices_polygon() {
        let c = Collider::polygon(vec![
            Vec2::new(-1.0, -1.0),
            Vec2::new(1.0, -1.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(-1.0, 1.0),
        ]);
        let verts = c.world_vertices(Vec2::new(5.0, 5.0), 0.0);
        assert_eq!(verts.len(), 4);
        assert!((verts[0].x - 4.0).abs() < 1e-6);
        assert!((verts[0].y - 4.0).abs() < 1e-6);
    }

    #[test]
    fn collider_with_offset() {
        let c = Collider::circle(1.0).with_offset(Vec2::new(3.0, 0.0));
        let aabb = c.compute_aabb(Vec2::ZERO, 0.0);
        assert!((aabb.min.x - 2.0).abs() < 1e-6);
        assert!((aabb.max.x - 4.0).abs() < 1e-6);
    }
}
