use arachne_math::{Mat4, Vec2, Vec3, Vec4, Rect};

// ---------------------------------------------------------------------------
// Camera2d
// ---------------------------------------------------------------------------

/// 2D orthographic camera.
pub struct Camera2d {
    pub position: Vec2,
    pub zoom: f32,
    pub rotation: f32,
    pub viewport_size: Vec2,
}

impl Camera2d {
    pub fn new(viewport_width: f32, viewport_height: f32) -> Self {
        Self {
            position: Vec2::ZERO,
            zoom: 1.0,
            rotation: 0.0,
            viewport_size: Vec2::new(viewport_width, viewport_height),
        }
    }

    /// Compute the view-projection matrix for this camera.
    pub fn view_projection(&self) -> Mat4 {
        let half_w = self.viewport_size.x / (2.0 * self.zoom);
        let half_h = self.viewport_size.y / (2.0 * self.zoom);

        // wgpu/Vulkan clip space: Y = -1 is TOP, Y = 1 is BOTTOM.
        // Swap bottom/top so world Y-up maps to screen Y-up.
        let projection = Mat4::orthographic(
            -half_w, half_w,
            half_h, -half_h,
            -1.0, 1.0,
        );

        let translation = Mat4::from_translation(Vec3::new(-self.position.x, -self.position.y, 0.0));

        if self.rotation.abs() > 1e-6 {
            let rotation = Mat4::from_rotation_z(-self.rotation);
            projection * rotation * translation
        } else {
            projection * translation
        }
    }

    /// Convert screen coordinates (pixels, origin top-left) to world coordinates.
    pub fn screen_to_world(&self, screen_pos: Vec2) -> Vec2 {
        // Screen coords: (0,0) = top-left, (width, height) = bottom-right
        // NDC: (-1,-1) = bottom-left, (1,1) = top-right
        let ndc_x = (screen_pos.x / self.viewport_size.x) * 2.0 - 1.0;
        let ndc_y = 1.0 - (screen_pos.y / self.viewport_size.y) * 2.0;

        let half_w = self.viewport_size.x / (2.0 * self.zoom);
        let half_h = self.viewport_size.y / (2.0 * self.zoom);

        if self.rotation.abs() > 1e-6 {
            let cos = self.rotation.cos();
            let sin = self.rotation.sin();
            let local_x = ndc_x * half_w;
            let local_y = ndc_y * half_h;
            Vec2::new(
                cos * local_x - sin * local_y + self.position.x,
                sin * local_x + cos * local_y + self.position.y,
            )
        } else {
            Vec2::new(
                ndc_x * half_w + self.position.x,
                ndc_y * half_h + self.position.y,
            )
        }
    }

    /// Convert world coordinates to screen coordinates (pixels, origin top-left).
    pub fn world_to_screen(&self, world_pos: Vec2) -> Vec2 {
        let dx = world_pos.x - self.position.x;
        let dy = world_pos.y - self.position.y;

        let (local_x, local_y) = if self.rotation.abs() > 1e-6 {
            let cos = (-self.rotation).cos();
            let sin = (-self.rotation).sin();
            (cos * dx - sin * dy, sin * dx + cos * dy)
        } else {
            (dx, dy)
        };

        let half_w = self.viewport_size.x / (2.0 * self.zoom);
        let half_h = self.viewport_size.y / (2.0 * self.zoom);

        let ndc_x = local_x / half_w;
        let ndc_y = local_y / half_h;

        Vec2::new(
            (ndc_x + 1.0) * 0.5 * self.viewport_size.x,
            (1.0 - ndc_y) * 0.5 * self.viewport_size.y,
        )
    }

    /// Get the visible world-space bounding rect.
    pub fn visible_rect(&self) -> Rect {
        let half_w = self.viewport_size.x / (2.0 * self.zoom);
        let half_h = self.viewport_size.y / (2.0 * self.zoom);
        Rect::from_center_size(
            self.position,
            Vec2::new(half_w * 2.0, half_h * 2.0),
        )
    }
}

// ---------------------------------------------------------------------------
// Camera3d
// ---------------------------------------------------------------------------

/// 3D perspective camera.
pub struct Camera3d {
    pub position: Vec3,
    pub target: Vec3,
    pub up: Vec3,
    pub fov: f32,
    pub near: f32,
    pub far: f32,
    pub aspect: f32,
}

impl Camera3d {
    pub fn new(aspect: f32) -> Self {
        Self {
            position: Vec3::new(0.0, 0.0, 5.0),
            target: Vec3::ZERO,
            up: Vec3::Y,
            fov: std::f32::consts::FRAC_PI_4, // 45 degrees
            near: 0.1,
            far: 100.0,
            aspect,
        }
    }

    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_at(self.position, self.target, self.up)
    }

    pub fn projection_matrix(&self) -> Mat4 {
        Mat4::perspective(self.fov, self.aspect, self.near, self.far)
    }

    pub fn view_projection(&self) -> Mat4 {
        self.projection_matrix() * self.view_matrix()
    }

    /// Convert screen coordinates to a world-space ray (origin, direction).
    pub fn screen_to_world_ray(
        &self,
        screen_pos: Vec2,
        viewport_size: Vec2,
    ) -> (Vec3, Vec3) {
        // Screen -> NDC
        let ndc_x = (screen_pos.x / viewport_size.x) * 2.0 - 1.0;
        let ndc_y = 1.0 - (screen_pos.y / viewport_size.y) * 2.0;

        let vp = self.view_projection();
        let inv_vp = vp.inverse().unwrap_or(Mat4::IDENTITY);

        let near_point = inv_vp.mul_vec4(Vec4::new(ndc_x, ndc_y, 0.0, 1.0));
        let far_point = inv_vp.mul_vec4(Vec4::new(ndc_x, ndc_y, 1.0, 1.0));

        let near_world = Vec3::new(
            near_point.x / near_point.w,
            near_point.y / near_point.w,
            near_point.z / near_point.w,
        );
        let far_world = Vec3::new(
            far_point.x / far_point.w,
            far_point.y / far_point.w,
            far_point.z / far_point.w,
        );

        let direction = (far_world - near_world).normalize();
        (near_world, direction)
    }

    /// Convert a world-space point to screen coordinates.
    pub fn world_to_screen(&self, world_pos: Vec3, viewport_size: Vec2) -> Vec2 {
        let clip = self.view_projection().mul_vec4(Vec4::new(
            world_pos.x,
            world_pos.y,
            world_pos.z,
            1.0,
        ));
        let ndc_x = clip.x / clip.w;
        let ndc_y = clip.y / clip.w;

        Vec2::new(
            (ndc_x + 1.0) * 0.5 * viewport_size.x,
            (1.0 - ndc_y) * 0.5 * viewport_size.y,
        )
    }
}

// ---------------------------------------------------------------------------
// GPU-compatible camera uniform
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
}

impl CameraUniform {
    pub fn from_mat4(m: &Mat4) -> Self {
        Self {
            view_proj: m.cols,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq_f32(a: f32, b: f32, eps: f32) -> bool {
        (a - b).abs() < eps
    }

    fn approx_eq_vec2(a: Vec2, b: Vec2, eps: f32) -> bool {
        approx_eq_f32(a.x, b.x, eps) && approx_eq_f32(a.y, b.y, eps)
    }

    #[test]
    fn camera2d_screen_corners_to_world() {
        let cam = Camera2d::new(800.0, 600.0);
        // Camera at origin, zoom 1.0, no rotation
        // Screen (0,0) = top-left -> world (-400, 300)
        let tl = cam.screen_to_world(Vec2::new(0.0, 0.0));
        assert!(
            approx_eq_vec2(tl, Vec2::new(-400.0, 300.0), 0.1),
            "top-left: {:?}",
            tl
        );

        // Screen (800,600) = bottom-right -> world (400, -300)
        let br = cam.screen_to_world(Vec2::new(800.0, 600.0));
        assert!(
            approx_eq_vec2(br, Vec2::new(400.0, -300.0), 0.1),
            "bottom-right: {:?}",
            br
        );

        // Screen center -> world origin
        let center = cam.screen_to_world(Vec2::new(400.0, 300.0));
        assert!(
            approx_eq_vec2(center, Vec2::ZERO, 0.1),
            "center: {:?}",
            center
        );
    }

    #[test]
    fn camera2d_zoom() {
        let mut cam = Camera2d::new(800.0, 600.0);
        cam.zoom = 2.0;

        // At zoom 2, visible area is halved
        let tl = cam.screen_to_world(Vec2::new(0.0, 0.0));
        assert!(
            approx_eq_vec2(tl, Vec2::new(-200.0, 150.0), 0.1),
            "zoomed top-left: {:?}",
            tl
        );
    }

    #[test]
    fn camera2d_position_offset() {
        let mut cam = Camera2d::new(800.0, 600.0);
        cam.position = Vec2::new(100.0, 50.0);

        let center = cam.screen_to_world(Vec2::new(400.0, 300.0));
        assert!(
            approx_eq_vec2(center, Vec2::new(100.0, 50.0), 0.1),
            "offset center: {:?}",
            center
        );
    }

    #[test]
    fn camera2d_roundtrip() {
        let mut cam = Camera2d::new(800.0, 600.0);
        cam.position = Vec2::new(50.0, -30.0);
        cam.zoom = 1.5;

        let world = Vec2::new(100.0, 200.0);
        let screen = cam.world_to_screen(world);
        let back = cam.screen_to_world(screen);
        assert!(
            approx_eq_vec2(back, world, 0.5),
            "roundtrip failed: {:?} -> {:?} -> {:?}",
            world,
            screen,
            back
        );
    }

    #[test]
    fn camera2d_view_projection_identity_at_origin() {
        let cam = Camera2d::new(2.0, 2.0);
        let vp = cam.view_projection();

        // A point at origin should map to clip (0,0,z,1)
        let p = Vec4::new(0.0, 0.0, 0.0, 1.0);
        let clip = vp.mul_vec4(p);
        assert!(
            approx_eq_f32(clip.x, 0.0, 0.01) && approx_eq_f32(clip.y, 0.0, 0.01),
            "origin in clip space: {:?}",
            clip
        );
    }

    #[test]
    fn camera3d_perspective_basic() {
        let cam = Camera3d::new(1.0);
        let proj = cam.projection_matrix();

        // A point on the near plane center should map to z=0 after perspective divide
        let p = Vec4::new(0.0, 0.0, -0.1, 1.0);
        let clip = proj.mul_vec4(p);
        let ndc_z = clip.z / clip.w;
        assert!(
            ndc_z.abs() < 0.01,
            "near plane should map to ndc_z≈0, got {}",
            ndc_z
        );
    }

    #[test]
    fn camera3d_view_matrix() {
        let cam = Camera3d {
            position: Vec3::new(0.0, 0.0, 5.0),
            target: Vec3::ZERO,
            up: Vec3::Y,
            fov: std::f32::consts::FRAC_PI_4,
            near: 0.1,
            far: 100.0,
            aspect: 1.0,
        };

        let view = cam.view_matrix();
        // Origin in world -> (0, 0, -5) in view space
        let origin = view.mul_vec4(Vec4::new(0.0, 0.0, 0.0, 1.0));
        assert!(
            approx_eq_f32(origin.z, -5.0, 0.01),
            "origin.z in view: {}",
            origin.z
        );
    }

    #[test]
    fn camera3d_world_to_screen_center() {
        let cam = Camera3d::new(1.0);
        let vp_size = Vec2::new(800.0, 600.0);

        // Target point should project to screen center
        let screen = cam.world_to_screen(cam.target, vp_size);
        assert!(
            approx_eq_vec2(screen, Vec2::new(400.0, 300.0), 1.0),
            "target on screen: {:?}",
            screen
        );
    }

    #[test]
    fn camera_uniform_from_mat4() {
        let m = Mat4::IDENTITY;
        let u = CameraUniform::from_mat4(&m);
        assert_eq!(u.view_proj, Mat4::IDENTITY.cols);
    }
}
