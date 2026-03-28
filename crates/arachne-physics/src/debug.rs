use arachne_math::{Color, Vec2};

use crate::broadphase::SpatialHashGrid;
use crate::collider::{Collider, ColliderShape};
use crate::narrowphase::ContactManifold;
use crate::rigid_body::RigidBodyData;

/// Line segment for debug drawing.
#[derive(Clone, Debug)]
pub struct DebugLine {
    pub start: Vec2,
    pub end: Vec2,
    pub color: Color,
}

/// All debug draw data for the current physics state.
#[derive(Clone, Debug, Default)]
pub struct DebugDrawData {
    pub lines: Vec<DebugLine>,
}

impl DebugDrawData {
    pub fn new() -> Self {
        Self { lines: Vec::new() }
    }

    pub fn clear(&mut self) {
        self.lines.clear();
    }

    /// Generates debug lines for all collider outlines.
    pub fn draw_colliders(
        &mut self,
        bodies: &[RigidBodyData],
        colliders: &[Option<Collider>],
    ) {
        let color = Color::GREEN;
        for (body, collider) in bodies.iter().zip(colliders.iter()) {
            if let Some(col) = collider {
                self.draw_collider_shape(body.position, body.rotation, col, color);
            }
        }
    }

    fn draw_collider_shape(
        &mut self,
        body_pos: Vec2,
        body_rot: f32,
        collider: &Collider,
        color: Color,
    ) {
        let center = collider.world_center(body_pos, body_rot);
        match &collider.shape {
            ColliderShape::Circle { radius } => {
                self.draw_circle(center, *radius, color, 16);
            }
            ColliderShape::AABB { half_extents: _ } => {
                let verts = collider.world_vertices(body_pos, body_rot);
                self.draw_polygon(&verts, color);
            }
            ColliderShape::Polygon { vertices: _ } => {
                let verts = collider.world_vertices(body_pos, body_rot);
                self.draw_polygon(&verts, color);
            }
            ColliderShape::Capsule {
                half_height,
                radius,
            } => {
                let dir = Vec2::new(0.0, *half_height).rotate(body_rot);
                let a = center - dir;
                let b = center + dir;
                // Draw semicircles at ends
                self.draw_semicircle(a, *radius, body_rot + std::f32::consts::PI, color, 8);
                self.draw_semicircle(b, *radius, body_rot, color, 8);
                // Connect with lines
                let perp = Vec2::new(-dir.y, dir.x).normalize() * *radius;
                self.lines.push(DebugLine {
                    start: a + perp,
                    end: b + perp,
                    color,
                });
                self.lines.push(DebugLine {
                    start: a - perp,
                    end: b - perp,
                    color,
                });
            }
        }
    }

    fn draw_circle(&mut self, center: Vec2, radius: f32, color: Color, segments: usize) {
        for i in 0..segments {
            let a0 = (i as f32 / segments as f32) * std::f32::consts::TAU;
            let a1 = ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU;
            self.lines.push(DebugLine {
                start: center + Vec2::new(a0.cos(), a0.sin()) * radius,
                end: center + Vec2::new(a1.cos(), a1.sin()) * radius,
                color,
            });
        }
    }

    fn draw_semicircle(
        &mut self,
        center: Vec2,
        radius: f32,
        start_angle: f32,
        color: Color,
        segments: usize,
    ) {
        for i in 0..segments {
            let a0 = start_angle + (i as f32 / segments as f32) * std::f32::consts::PI;
            let a1 = start_angle + ((i + 1) as f32 / segments as f32) * std::f32::consts::PI;
            self.lines.push(DebugLine {
                start: center + Vec2::new(a0.cos(), a0.sin()) * radius,
                end: center + Vec2::new(a1.cos(), a1.sin()) * radius,
                color,
            });
        }
    }

    fn draw_polygon(&mut self, verts: &[Vec2], color: Color) {
        let n = verts.len();
        for i in 0..n {
            self.lines.push(DebugLine {
                start: verts[i],
                end: verts[(i + 1) % n],
                color,
            });
        }
    }

    /// Generates debug lines for contact points and normals.
    pub fn draw_contacts(&mut self, manifolds: &[ContactManifold]) {
        let point_color = Color::RED;
        let normal_color = Color::BLUE;
        let normal_len = 0.3;

        for manifold in manifolds {
            for i in 0..manifold.point_count {
                let point = manifold.points[i];
                // Small cross at contact point
                let size = 0.1;
                self.lines.push(DebugLine {
                    start: point - Vec2::new(size, 0.0),
                    end: point + Vec2::new(size, 0.0),
                    color: point_color,
                });
                self.lines.push(DebugLine {
                    start: point - Vec2::new(0.0, size),
                    end: point + Vec2::new(0.0, size),
                    color: point_color,
                });
                // Normal arrow
                self.lines.push(DebugLine {
                    start: point,
                    end: point + manifold.normal * normal_len,
                    color: normal_color,
                });
            }
        }
    }

    /// Generates debug lines for the broadphase grid cells.
    pub fn draw_broadphase_grid(&mut self, grid: &SpatialHashGrid) {
        let color = Color::new(0.3, 0.3, 0.3, 0.5);
        for cell_rect in grid.debug_cells() {
            let min = cell_rect.min;
            let max = cell_rect.max;
            self.lines.push(DebugLine {
                start: Vec2::new(min.x, min.y),
                end: Vec2::new(max.x, min.y),
                color,
            });
            self.lines.push(DebugLine {
                start: Vec2::new(max.x, min.y),
                end: Vec2::new(max.x, max.y),
                color,
            });
            self.lines.push(DebugLine {
                start: Vec2::new(max.x, max.y),
                end: Vec2::new(min.x, max.y),
                color,
            });
            self.lines.push(DebugLine {
                start: Vec2::new(min.x, max.y),
                end: Vec2::new(min.x, min.y),
                color,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rigid_body::RigidBodyData;

    #[test]
    fn draw_circle_collider() {
        let mut draw = DebugDrawData::new();
        let bodies = vec![RigidBodyData::new_dynamic(Vec2::ZERO, 1.0, 1.0)];
        let colliders: Vec<Option<Collider>> = vec![Some(Collider::circle(1.0))];
        draw.draw_colliders(&bodies, &colliders);
        assert_eq!(draw.lines.len(), 16); // 16 segments for circle
    }

    #[test]
    fn draw_polygon_collider() {
        let mut draw = DebugDrawData::new();
        let bodies = vec![RigidBodyData::new_dynamic(Vec2::ZERO, 1.0, 1.0)];
        let colliders: Vec<Option<Collider>> = vec![Some(Collider::polygon(vec![
            Vec2::new(-1.0, -1.0),
            Vec2::new(1.0, -1.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(-1.0, 1.0),
        ]))];
        draw.draw_colliders(&bodies, &colliders);
        assert_eq!(draw.lines.len(), 4); // 4 edges for square
    }

    #[test]
    fn draw_contacts() {
        let mut draw = DebugDrawData::new();
        let manifold = ContactManifold {
            body_a: crate::rigid_body::BodyHandle(0),
            body_b: crate::rigid_body::BodyHandle(1),
            normal: Vec2::new(1.0, 0.0),
            depth: 0.5,
            points: [Vec2::new(1.0, 0.0), Vec2::ZERO],
            point_count: 1,
        };
        draw.draw_contacts(&[manifold]);
        // 2 cross lines + 1 normal line = 3 lines per contact
        assert_eq!(draw.lines.len(), 3);
    }

    #[test]
    fn clear_empties_draw_data() {
        let mut draw = DebugDrawData::new();
        draw.lines.push(DebugLine {
            start: Vec2::ZERO,
            end: Vec2::X,
            color: Color::RED,
        });
        draw.clear();
        assert!(draw.lines.is_empty());
    }
}
