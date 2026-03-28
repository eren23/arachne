use arachne_ecs::Entity;
use arachne_math::{Mat4, Vec3, Vec4};
use std::collections::HashMap;

use crate::graph::SceneGraph;

// ---------------------------------------------------------------------------
// Visibility
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Visibility {
    Visible,
    Hidden,
    Inherited,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ComputedVisibility {
    pub visible: bool,
    pub layer: u32,
}

impl ComputedVisibility {
    #[inline]
    pub fn new(visible: bool, layer: u32) -> Self {
        Self { visible, layer }
    }
}

// ---------------------------------------------------------------------------
// VisibilitySystem
// ---------------------------------------------------------------------------

pub struct VisibilitySystem {
    visibility: HashMap<Entity, Visibility>,
    computed: HashMap<Entity, ComputedVisibility>,
    layers: HashMap<Entity, u32>,
}

impl VisibilitySystem {
    #[inline]
    pub fn new() -> Self {
        Self {
            visibility: HashMap::new(),
            computed: HashMap::new(),
            layers: HashMap::new(),
        }
    }

    #[inline]
    pub fn set_visibility(&mut self, entity: Entity, vis: Visibility) {
        self.visibility.insert(entity, vis);
    }

    #[inline]
    pub fn get_visibility(&self, entity: Entity) -> Visibility {
        self.visibility.get(&entity).copied().unwrap_or(Visibility::Inherited)
    }

    #[inline]
    pub fn set_layer(&mut self, entity: Entity, layer: u32) {
        self.layers.insert(entity, layer);
    }

    #[inline]
    pub fn get_layer(&self, entity: Entity) -> u32 {
        self.layers.get(&entity).copied().unwrap_or(0xFFFF_FFFF)
    }

    #[inline]
    pub fn get_computed(&self, entity: Entity) -> Option<&ComputedVisibility> {
        self.computed.get(&entity)
    }

    /// Is this entity visible after propagation?
    pub fn is_visible(&self, entity: Entity) -> bool {
        self.computed.get(&entity).map_or(false, |c| c.visible)
    }

    /// Remove an entity from the visibility system.
    pub fn remove(&mut self, entity: Entity) {
        self.visibility.remove(&entity);
        self.computed.remove(&entity);
        self.layers.remove(&entity);
    }

    pub fn resolve(&mut self, graph: &SceneGraph) {
        let roots = graph.roots();
        for root in roots {
            self.resolve_entity(root, true, graph);
        }
    }

    fn resolve_entity(&mut self, entity: Entity, parent_visible: bool, graph: &SceneGraph) {
        let vis = self.visibility.get(&entity).copied().unwrap_or(Visibility::Inherited);
        let layer = self.layers.get(&entity).copied().unwrap_or(0xFFFF_FFFF);

        let visible = match vis {
            Visibility::Visible => true,
            Visibility::Hidden => false,
            Visibility::Inherited => parent_visible,
        };

        self.computed.insert(entity, ComputedVisibility { visible, layer });

        let children: Vec<Entity> = graph.children_of(entity).to_vec();
        for child in children {
            self.resolve_entity(child, visible, graph);
        }
    }

    /// Count of entities that are computed as visible.
    pub fn visible_count(&self) -> usize {
        self.computed.values().filter(|c| c.visible).count()
    }

    /// Count of entities that are computed as hidden.
    pub fn hidden_count(&self) -> usize {
        self.computed.values().filter(|c| !c.visible).count()
    }

    /// Get all visible entities.
    pub fn visible_entities(&self) -> Vec<Entity> {
        self.computed
            .iter()
            .filter(|(_, c)| c.visible)
            .map(|(&e, _)| e)
            .collect()
    }

    /// Get all visible entities on a specific layer.
    pub fn visible_on_layer(&self, layer_mask: u32) -> Vec<Entity> {
        self.computed
            .iter()
            .filter(|(_, c)| c.visible && (c.layer & layer_mask) != 0)
            .map(|(&e, _)| e)
            .collect()
    }
}

impl Default for VisibilitySystem {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Aabb
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    #[inline]
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    /// Create an AABB from center and half-extents.
    #[inline]
    pub fn from_center_half(center: Vec3, half: Vec3) -> Self {
        Self {
            min: Vec3::new(center.x - half.x, center.y - half.y, center.z - half.z),
            max: Vec3::new(center.x + half.x, center.y + half.y, center.z + half.z),
        }
    }

    /// Create the smallest AABB containing all given points.
    pub fn from_points(points: &[Vec3]) -> Self {
        if points.is_empty() {
            return Self::new(Vec3::ZERO, Vec3::ZERO);
        }
        let mut min = points[0];
        let mut max = points[0];
        for p in &points[1..] {
            if p.x < min.x { min.x = p.x; }
            if p.y < min.y { min.y = p.y; }
            if p.z < min.z { min.z = p.z; }
            if p.x > max.x { max.x = p.x; }
            if p.y > max.y { max.y = p.y; }
            if p.z > max.z { max.z = p.z; }
        }
        Self { min, max }
    }

    /// Merge two AABBs into one that contains both.
    pub fn merge(&self, other: &Aabb) -> Aabb {
        Aabb {
            min: Vec3::new(
                self.min.x.min(other.min.x),
                self.min.y.min(other.min.y),
                self.min.z.min(other.min.z),
            ),
            max: Vec3::new(
                self.max.x.max(other.max.x),
                self.max.y.max(other.max.y),
                self.max.z.max(other.max.z),
            ),
        }
    }

    /// Expand this AABB by a margin on all sides.
    pub fn expand(&self, margin: f32) -> Aabb {
        Aabb {
            min: Vec3::new(self.min.x - margin, self.min.y - margin, self.min.z - margin),
            max: Vec3::new(self.max.x + margin, self.max.y + margin, self.max.z + margin),
        }
    }

    #[inline]
    pub fn contains_point(&self, point: Vec3) -> bool {
        point.x >= self.min.x && point.x <= self.max.x
            && point.y >= self.min.y && point.y <= self.max.y
            && point.z >= self.min.z && point.z <= self.max.z
    }

    #[inline]
    pub fn intersects(&self, other: &Aabb) -> bool {
        self.min.x <= other.max.x && self.max.x >= other.min.x
            && self.min.y <= other.max.y && self.max.y >= other.min.y
            && self.min.z <= other.max.z && self.max.z >= other.min.z
    }

    /// Test if this AABB fully contains another.
    pub fn contains_aabb(&self, other: &Aabb) -> bool {
        self.min.x <= other.min.x && self.max.x >= other.max.x
            && self.min.y <= other.min.y && self.max.y >= other.max.y
            && self.min.z <= other.min.z && self.max.z >= other.max.z
    }

    #[inline]
    pub fn center(&self) -> Vec3 {
        Vec3::new(
            (self.min.x + self.max.x) * 0.5,
            (self.min.y + self.max.y) * 0.5,
            (self.min.z + self.max.z) * 0.5,
        )
    }

    #[inline]
    pub fn half_extents(&self) -> Vec3 {
        Vec3::new(
            (self.max.x - self.min.x) * 0.5,
            (self.max.y - self.min.y) * 0.5,
            (self.max.z - self.min.z) * 0.5,
        )
    }

    /// Surface area of the AABB (useful for BVH heuristics).
    pub fn surface_area(&self) -> f32 {
        let d = Vec3::new(
            self.max.x - self.min.x,
            self.max.y - self.min.y,
            self.max.z - self.min.z,
        );
        2.0 * (d.x * d.y + d.y * d.z + d.z * d.x)
    }

    /// Volume of the AABB.
    pub fn volume(&self) -> f32 {
        let d = Vec3::new(
            self.max.x - self.min.x,
            self.max.y - self.min.y,
            self.max.z - self.min.z,
        );
        d.x * d.y * d.z
    }

    /// Transform an AABB by a matrix, producing a new (larger) AABB that
    /// contains the transformed original.
    pub fn transform(&self, matrix: &Mat4) -> Aabb {
        let corners = [
            Vec3::new(self.min.x, self.min.y, self.min.z),
            Vec3::new(self.max.x, self.min.y, self.min.z),
            Vec3::new(self.min.x, self.max.y, self.min.z),
            Vec3::new(self.max.x, self.max.y, self.min.z),
            Vec3::new(self.min.x, self.min.y, self.max.z),
            Vec3::new(self.max.x, self.min.y, self.max.z),
            Vec3::new(self.min.x, self.max.y, self.max.z),
            Vec3::new(self.max.x, self.max.y, self.max.z),
        ];

        let transformed: Vec<Vec3> = corners
            .iter()
            .map(|c| {
                let v = matrix.mul_vec4(Vec4::new(c.x, c.y, c.z, 1.0));
                Vec3::new(v.x, v.y, v.z)
            })
            .collect();

        Aabb::from_points(&transformed)
    }
}

// ---------------------------------------------------------------------------
// Frustum
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frustum {
    pub planes: [Vec4; 6],
}

impl Frustum {
    #[inline]
    pub fn new(planes: [Vec4; 6]) -> Self {
        Self { planes }
    }

    pub fn test_aabb(&self, aabb: &Aabb) -> bool {
        let center = aabb.center();
        let half = aabb.half_extents();

        for plane in &self.planes {
            let normal = Vec3::new(plane.x, plane.y, plane.z);
            let d = plane.w;

            let r = half.x * normal.x.abs() + half.y * normal.y.abs() + half.z * normal.z.abs();
            let dist = normal.x * center.x + normal.y * center.y + normal.z * center.z + d;

            if dist < -r {
                return false;
            }
        }
        true
    }

    /// Test a sphere against the frustum.
    pub fn test_sphere(&self, center: Vec3, radius: f32) -> bool {
        for plane in &self.planes {
            let dist = plane.x * center.x + plane.y * center.y + plane.z * center.z + plane.w;
            if dist < -radius {
                return false;
            }
        }
        true
    }

    /// Test a point against the frustum.
    pub fn test_point(&self, point: Vec3) -> bool {
        for plane in &self.planes {
            let dist = plane.x * point.x + plane.y * point.y + plane.z * point.z + plane.w;
            if dist < 0.0 {
                return false;
            }
        }
        true
    }
}

pub fn extract_frustum_planes(view_proj: Mat4) -> Frustum {
    let m = &view_proj.cols;

    let row0 = Vec4::new(m[0][0], m[1][0], m[2][0], m[3][0]);
    let row1 = Vec4::new(m[0][1], m[1][1], m[2][1], m[3][1]);
    let row2 = Vec4::new(m[0][2], m[1][2], m[2][2], m[3][2]);
    let row3 = Vec4::new(m[0][3], m[1][3], m[2][3], m[3][3]);

    let mut planes = [
        Vec4::new(row3.x + row0.x, row3.y + row0.y, row3.z + row0.z, row3.w + row0.w),
        Vec4::new(row3.x - row0.x, row3.y - row0.y, row3.z - row0.z, row3.w - row0.w),
        Vec4::new(row3.x + row1.x, row3.y + row1.y, row3.z + row1.z, row3.w + row1.w),
        Vec4::new(row3.x - row1.x, row3.y - row1.y, row3.z - row1.z, row3.w - row1.w),
        Vec4::new(row3.x + row2.x, row3.y + row2.y, row3.z + row2.z, row3.w + row2.w),
        Vec4::new(row3.x - row2.x, row3.y - row2.y, row3.z - row2.z, row3.w - row2.w),
    ];

    for plane in &mut planes {
        let len = (plane.x * plane.x + plane.y * plane.y + plane.z * plane.z).sqrt();
        if len > 0.0 {
            let inv = 1.0 / len;
            plane.x *= inv;
            plane.y *= inv;
            plane.z *= inv;
            plane.w *= inv;
        }
    }

    Frustum { planes }
}

// ---------------------------------------------------------------------------
// FrustumCuller
// ---------------------------------------------------------------------------

pub struct FrustumCuller;

impl FrustumCuller {
    pub fn cull_2d(entities_with_aabb: &[(Entity, Aabb)], camera_aabb: &Aabb) -> Vec<Entity> {
        let mut visible = Vec::new();
        for &(entity, ref aabb) in entities_with_aabb {
            if camera_aabb.intersects(aabb) {
                visible.push(entity);
            }
        }
        visible
    }

    pub fn cull_3d(entities_with_aabb: &[(Entity, Aabb)], frustum: &Frustum) -> Vec<Entity> {
        let mut visible = Vec::new();
        for &(entity, ref aabb) in entities_with_aabb {
            if frustum.test_aabb(aabb) {
                visible.push(entity);
            }
        }
        visible
    }

    /// Cull using sphere bounds (faster but less precise).
    pub fn cull_3d_sphere(
        entities_with_sphere: &[(Entity, Vec3, f32)],
        frustum: &Frustum,
    ) -> Vec<Entity> {
        let mut visible = Vec::new();
        for &(entity, center, radius) in entities_with_sphere {
            if frustum.test_sphere(center, radius) {
                visible.push(entity);
            }
        }
        visible
    }

    pub fn filter_by_layer(
        entities: &[Entity],
        layers: &HashMap<Entity, u32>,
        camera_layer_mask: u32,
    ) -> Vec<Entity> {
        let mut result = Vec::new();
        for &entity in entities {
            let entity_layer = layers.get(&entity).copied().unwrap_or(0xFFFF_FFFF);
            if entity_layer & camera_layer_mask != 0 {
                result.push(entity);
            }
        }
        result
    }

    /// Combined frustum + visibility + layer culling.
    pub fn cull_visible(
        entities_with_aabb: &[(Entity, Aabb)],
        frustum: &Frustum,
        vis_system: &VisibilitySystem,
        camera_layer_mask: u32,
    ) -> Vec<Entity> {
        let mut visible = Vec::new();
        for &(entity, ref aabb) in entities_with_aabb {
            // Check visibility
            if !vis_system.is_visible(entity) {
                continue;
            }
            // Check layer
            let layer = vis_system.get_layer(entity);
            if layer & camera_layer_mask == 0 {
                continue;
            }
            // Check frustum
            if !frustum.test_aabb(aabb) {
                continue;
            }
            visible.push(entity);
        }
        visible
    }
}

// ---------------------------------------------------------------------------
// OcclusionResult – for future occlusion culling extension
// ---------------------------------------------------------------------------

/// Result of an occlusion query (placeholder for future extension).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OcclusionResult {
    Visible,
    Occluded,
    Unknown,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn e(index: u32) -> Entity {
        Entity::from_raw(index, 0)
    }

    #[test]
    fn hide_parent_hides_inherited_children() {
        let mut graph = SceneGraph::new();
        let parent = e(0);
        let child = e(1);
        graph.add_child(parent, child);

        let mut vis_sys = VisibilitySystem::new();
        vis_sys.set_visibility(parent, Visibility::Hidden);
        vis_sys.set_visibility(child, Visibility::Inherited);
        vis_sys.resolve(&graph);

        assert!(!vis_sys.get_computed(child).unwrap().visible);
    }

    #[test]
    fn show_child_inherited_still_hidden_if_parent_hidden() {
        let mut graph = SceneGraph::new();
        let parent = e(0);
        let child = e(1);
        graph.add_child(parent, child);

        let mut vis_sys = VisibilitySystem::new();
        vis_sys.set_visibility(parent, Visibility::Hidden);
        vis_sys.set_visibility(child, Visibility::Inherited);
        vis_sys.resolve(&graph);

        assert!(!vis_sys.get_computed(child).unwrap().visible);
    }

    #[test]
    fn child_visible_overrides_hidden_parent() {
        let mut graph = SceneGraph::new();
        let parent = e(0);
        let child = e(1);
        graph.add_child(parent, child);

        let mut vis_sys = VisibilitySystem::new();
        vis_sys.set_visibility(parent, Visibility::Hidden);
        vis_sys.set_visibility(child, Visibility::Visible);
        vis_sys.resolve(&graph);

        assert!(!vis_sys.get_computed(parent).unwrap().visible);
        assert!(vis_sys.get_computed(child).unwrap().visible);
    }

    #[test]
    fn frustum_culling_1000_entities() {
        let view = Mat4::look_at(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO, Vec3::Y);
        let proj = Mat4::perspective(core::f32::consts::FRAC_PI_2, 1.0, 0.1, 100.0);
        let view_proj = proj * view;
        let frustum = extract_frustum_planes(view_proj);

        let mut entities_with_aabb = Vec::new();
        for i in 0..1000u32 {
            let x = (i % 32) as f32 * 3.125 - 50.0;
            let z = (i / 32) as f32 * 3.125 - 50.0;
            let aabb = Aabb::new(
                Vec3::new(x - 0.5, -0.5, z - 0.5),
                Vec3::new(x + 0.5, 0.5, z + 0.5),
            );
            entities_with_aabb.push((e(i), aabb));
        }

        let visible = FrustumCuller::cull_3d(&entities_with_aabb, &frustum);
        assert!(visible.len() < entities_with_aabb.len());
        assert!(!visible.is_empty());
    }

    #[test]
    fn layer_mask_filtering() {
        let mut layers = HashMap::new();
        let e0 = e(0);
        let e1 = e(1);
        let e2 = e(2);

        layers.insert(e0, 0b0001);
        layers.insert(e1, 0b0010);
        layers.insert(e2, 0b0011);

        let entities = vec![e0, e1, e2];

        let visible = FrustumCuller::filter_by_layer(&entities, &layers, 0b0001);
        assert_eq!(visible, vec![e0, e2]);

        let visible = FrustumCuller::filter_by_layer(&entities, &layers, 0b0010);
        assert_eq!(visible, vec![e1, e2]);

        let visible = FrustumCuller::filter_by_layer(&entities, &layers, 0b0011);
        assert_eq!(visible, vec![e0, e1, e2]);

        let visible = FrustumCuller::filter_by_layer(&entities, &layers, 0b0100);
        assert!(visible.is_empty());
    }

    #[test]
    fn aabb_contains_and_intersects() {
        let a = Aabb::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(2.0, 2.0, 2.0));
        assert!(a.contains_point(Vec3::new(1.0, 1.0, 1.0)));
        assert!(!a.contains_point(Vec3::new(3.0, 1.0, 1.0)));

        let b = Aabb::new(Vec3::new(1.0, 1.0, 1.0), Vec3::new(3.0, 3.0, 3.0));
        assert!(a.intersects(&b));

        let c = Aabb::new(Vec3::new(5.0, 5.0, 5.0), Vec3::new(6.0, 6.0, 6.0));
        assert!(!a.intersects(&c));
    }

    #[test]
    fn cull_2d_basic() {
        let camera = Aabb::new(Vec3::new(-5.0, -5.0, 0.0), Vec3::new(5.0, 5.0, 0.0));

        let entities = vec![
            (e(0), Aabb::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 0.0))),
            (e(1), Aabb::new(Vec3::new(10.0, 10.0, 0.0), Vec3::new(11.0, 11.0, 0.0))),
            (e(2), Aabb::new(Vec3::new(4.0, 4.0, 0.0), Vec3::new(6.0, 6.0, 0.0))),
        ];

        let visible = FrustumCuller::cull_2d(&entities, &camera);
        assert_eq!(visible, vec![e(0), e(2)]);
    }

    // -- New tests --------------------------------------------------------

    #[test]
    fn aabb_from_center_half() {
        let aabb = Aabb::from_center_half(Vec3::new(5.0, 5.0, 5.0), Vec3::new(1.0, 1.0, 1.0));
        assert_eq!(aabb.min, Vec3::new(4.0, 4.0, 4.0));
        assert_eq!(aabb.max, Vec3::new(6.0, 6.0, 6.0));
    }

    #[test]
    fn aabb_from_points() {
        let points = vec![
            Vec3::new(-1.0, 0.0, 0.0),
            Vec3::new(3.0, 2.0, 1.0),
            Vec3::new(0.0, -1.0, 5.0),
        ];
        let aabb = Aabb::from_points(&points);
        assert_eq!(aabb.min, Vec3::new(-1.0, -1.0, 0.0));
        assert_eq!(aabb.max, Vec3::new(3.0, 2.0, 5.0));
    }

    #[test]
    fn aabb_merge() {
        let a = Aabb::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 1.0));
        let b = Aabb::new(Vec3::new(-1.0, 2.0, 0.0), Vec3::new(0.5, 3.0, 0.5));
        let merged = a.merge(&b);
        assert_eq!(merged.min, Vec3::new(-1.0, 0.0, 0.0));
        assert_eq!(merged.max, Vec3::new(1.0, 3.0, 1.0));
    }

    #[test]
    fn aabb_expand() {
        let a = Aabb::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 1.0));
        let expanded = a.expand(0.5);
        assert!((expanded.min.x - (-0.5)).abs() < 1e-6);
        assert!((expanded.max.x - 1.5).abs() < 1e-6);
    }

    #[test]
    fn aabb_contains_aabb() {
        let outer = Aabb::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 10.0, 10.0));
        let inner = Aabb::new(Vec3::new(2.0, 2.0, 2.0), Vec3::new(8.0, 8.0, 8.0));
        let outside = Aabb::new(Vec3::new(11.0, 0.0, 0.0), Vec3::new(12.0, 1.0, 1.0));

        assert!(outer.contains_aabb(&inner));
        assert!(!outer.contains_aabb(&outside));
        assert!(!inner.contains_aabb(&outer));
    }

    #[test]
    fn aabb_surface_area_and_volume() {
        let a = Aabb::new(Vec3::ZERO, Vec3::new(2.0, 3.0, 4.0));
        assert!((a.volume() - 24.0).abs() < 1e-6);
        // SA = 2*(2*3 + 3*4 + 4*2) = 2*(6+12+8) = 52
        assert!((a.surface_area() - 52.0).abs() < 1e-6);
    }

    #[test]
    fn aabb_transform_identity() {
        let aabb = Aabb::new(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0));
        let transformed = aabb.transform(&Mat4::IDENTITY);
        assert!((transformed.min.x - (-1.0)).abs() < 1e-4);
        assert!((transformed.max.x - 1.0).abs() < 1e-4);
    }

    #[test]
    fn frustum_test_sphere() {
        let view = Mat4::look_at(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO, Vec3::Y);
        let proj = Mat4::perspective(core::f32::consts::FRAC_PI_2, 1.0, 0.1, 100.0);
        let frustum = extract_frustum_planes(proj * view);

        // Sphere at origin should be visible
        assert!(frustum.test_sphere(Vec3::ZERO, 1.0));
        // Sphere far away should not be visible
        assert!(!frustum.test_sphere(Vec3::new(1000.0, 0.0, 0.0), 1.0));
    }

    #[test]
    fn frustum_test_point() {
        let view = Mat4::look_at(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO, Vec3::Y);
        let proj = Mat4::perspective(core::f32::consts::FRAC_PI_2, 1.0, 0.1, 100.0);
        let frustum = extract_frustum_planes(proj * view);

        assert!(frustum.test_point(Vec3::ZERO));
        assert!(!frustum.test_point(Vec3::new(1000.0, 0.0, 0.0)));
    }

    #[test]
    fn visibility_system_counts() {
        let mut graph = SceneGraph::new();
        let a = e(0);
        let b = e(1);
        let c = e(2);
        graph.add_child(a, b);
        graph.add_child(a, c);

        let mut vis = VisibilitySystem::new();
        vis.set_visibility(a, Visibility::Visible);
        vis.set_visibility(b, Visibility::Inherited);
        vis.set_visibility(c, Visibility::Hidden);
        vis.resolve(&graph);

        assert_eq!(vis.visible_count(), 2); // a and b
        assert_eq!(vis.hidden_count(), 1); // c
    }

    #[test]
    fn visibility_system_visible_entities() {
        let mut graph = SceneGraph::new();
        let a = e(0);
        let b = e(1);
        graph.add_child(a, b);

        let mut vis = VisibilitySystem::new();
        vis.set_visibility(a, Visibility::Visible);
        vis.set_visibility(b, Visibility::Visible);
        vis.resolve(&graph);

        let visible = vis.visible_entities();
        assert_eq!(visible.len(), 2);
    }

    #[test]
    fn visibility_system_remove_entity() {
        let mut vis = VisibilitySystem::new();
        vis.set_visibility(e(0), Visibility::Visible);
        vis.set_layer(e(0), 1);
        vis.remove(e(0));

        assert_eq!(vis.get_visibility(e(0)), Visibility::Inherited);
        assert!(vis.get_computed(e(0)).is_none());
    }

    #[test]
    fn cull_visible_combined() {
        let view = Mat4::look_at(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO, Vec3::Y);
        let proj = Mat4::perspective(core::f32::consts::FRAC_PI_2, 1.0, 0.1, 100.0);
        let frustum = extract_frustum_planes(proj * view);

        let mut graph = SceneGraph::new();
        for i in 0..3u32 {
            // All roots
            graph.add_child(e(100), e(i)); // parent=100
        }

        let mut vis = VisibilitySystem::new();
        vis.set_visibility(e(100), Visibility::Visible);
        vis.set_visibility(e(0), Visibility::Visible);
        vis.set_visibility(e(1), Visibility::Hidden); // hidden
        vis.set_visibility(e(2), Visibility::Visible);
        vis.set_layer(e(0), 1);
        vis.set_layer(e(1), 1);
        vis.set_layer(e(2), 1);
        vis.resolve(&graph);

        let entities = vec![
            (e(0), Aabb::new(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0))),
            (e(1), Aabb::new(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0))),
            (e(2), Aabb::new(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0))),
        ];

        let visible = FrustumCuller::cull_visible(&entities, &frustum, &vis, 1);
        // e(0) and e(2) are visible, e(1) is hidden
        assert_eq!(visible.len(), 2);
        assert!(visible.contains(&e(0)));
        assert!(visible.contains(&e(2)));
        assert!(!visible.contains(&e(1)));
    }

    #[test]
    fn sphere_culling() {
        let view = Mat4::look_at(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO, Vec3::Y);
        let proj = Mat4::perspective(core::f32::consts::FRAC_PI_2, 1.0, 0.1, 100.0);
        let frustum = extract_frustum_planes(proj * view);

        let entities = vec![
            (e(0), Vec3::ZERO, 1.0),                     // visible
            (e(1), Vec3::new(1000.0, 0.0, 0.0), 1.0),   // not visible
        ];

        let visible = FrustumCuller::cull_3d_sphere(&entities, &frustum);
        assert_eq!(visible, vec![e(0)]);
    }
}
