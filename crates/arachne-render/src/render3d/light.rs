use arachne_math::{Color, Vec3};

// ---------------------------------------------------------------------------
// CPU-side light types
// ---------------------------------------------------------------------------

/// A point light with position, color, intensity, and range (attenuation).
#[derive(Clone, Debug)]
pub struct PointLight {
    pub position: Vec3,
    pub color: Color,
    pub intensity: f32,
    pub range: f32,
}

impl PointLight {
    pub fn new(position: Vec3, color: Color, intensity: f32, range: f32) -> Self {
        Self { position, color, intensity, range }
    }
}

/// A directional light with direction, color, and intensity.
#[derive(Clone, Debug)]
pub struct DirectionalLight {
    pub direction: Vec3,
    pub color: Color,
    pub intensity: f32,
}

impl DirectionalLight {
    pub fn new(direction: Vec3, color: Color, intensity: f32) -> Self {
        Self {
            direction: direction.normalize(),
            color,
            intensity,
        }
    }
}

/// A spot light with position, direction, inner/outer cone angles, and range.
#[derive(Clone, Debug)]
pub struct SpotLight {
    pub position: Vec3,
    pub direction: Vec3,
    pub color: Color,
    pub intensity: f32,
    pub inner_angle: f32,
    pub outer_angle: f32,
    pub range: f32,
}

impl SpotLight {
    pub fn new(
        position: Vec3,
        direction: Vec3,
        color: Color,
        intensity: f32,
        inner_angle: f32,
        outer_angle: f32,
        range: f32,
    ) -> Self {
        Self {
            position,
            direction: direction.normalize(),
            color,
            intensity,
            inner_angle,
            outer_angle,
            range,
        }
    }
}

// ---------------------------------------------------------------------------
// GPU-side structs (Pod/Zeroable, std140-compatible)
// ---------------------------------------------------------------------------

/// Matches `Light` in mesh_pbr.wgsl.
///
/// Layout:
///   position_type:     vec4  (xyz = position, w = type: 0=dir, 1=point, 2=spot)
///   direction_range:   vec4  (xyz = direction, w = range)
///   color_intensity:   vec4  (xyz = color, w = intensity)
///   spot_params:       vec4  (x = inner_angle_cos, y = outer_angle_cos, zw = 0)
///
/// Size: 64 bytes per light.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuLight {
    pub position_type: [f32; 4],
    pub direction_range: [f32; 4],
    pub color_intensity: [f32; 4],
    pub spot_params: [f32; 4],
}

impl Default for GpuLight {
    fn default() -> Self {
        Self {
            position_type: [0.0; 4],
            direction_range: [0.0; 4],
            color_intensity: [0.0; 4],
            spot_params: [0.0; 4],
        }
    }
}

impl GpuLight {
    pub fn from_directional(light: &DirectionalLight) -> Self {
        let d = light.direction.normalize();
        Self {
            position_type: [0.0, 0.0, 0.0, 0.0], // type 0 = directional
            direction_range: [d.x, d.y, d.z, 0.0],
            color_intensity: [light.color.r, light.color.g, light.color.b, light.intensity],
            spot_params: [0.0; 4],
        }
    }

    pub fn from_point(light: &PointLight) -> Self {
        Self {
            position_type: [light.position.x, light.position.y, light.position.z, 1.0],
            direction_range: [0.0, 0.0, 0.0, light.range],
            color_intensity: [light.color.r, light.color.g, light.color.b, light.intensity],
            spot_params: [0.0; 4],
        }
    }

    pub fn from_spot(light: &SpotLight) -> Self {
        let d = light.direction.normalize();
        Self {
            position_type: [light.position.x, light.position.y, light.position.z, 2.0],
            direction_range: [d.x, d.y, d.z, light.range],
            color_intensity: [light.color.r, light.color.g, light.color.b, light.intensity],
            spot_params: [
                light.inner_angle.cos(),
                light.outer_angle.cos(),
                0.0,
                0.0,
            ],
        }
    }
}

/// Matches `LightUniforms` in mesh_pbr.wgsl.
///
/// Layout:
///   lights[8]:           8 × 64 = 512 bytes
///   num_lights_ambient:  vec4<f32>  (x = count, yzw = ambient color)
///
/// Total: 528 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightUniform {
    pub lights: [GpuLight; 8],
    pub num_lights_ambient: [f32; 4],
}

impl Default for LightUniform {
    fn default() -> Self {
        Self {
            lights: [GpuLight::default(); 8],
            num_lights_ambient: [0.0, 0.1, 0.1, 0.1], // 0 lights, dim ambient
        }
    }
}

// ---------------------------------------------------------------------------
// LightState: collects lights, packs into LightUniform
// ---------------------------------------------------------------------------

/// Maximum number of forward lights.
pub const MAX_LIGHTS: usize = 8;

/// Manages the set of active lights and produces a packed GPU uniform.
pub struct LightState {
    directional: Vec<DirectionalLight>,
    point: Vec<PointLight>,
    spot: Vec<SpotLight>,
    pub ambient: Color,
}

impl LightState {
    pub fn new() -> Self {
        Self {
            directional: Vec::new(),
            point: Vec::new(),
            spot: Vec::new(),
            ambient: Color::rgb(0.1, 0.1, 0.1),
        }
    }

    pub fn clear(&mut self) {
        self.directional.clear();
        self.point.clear();
        self.spot.clear();
    }

    pub fn add_directional(&mut self, light: DirectionalLight) {
        self.directional.push(light);
    }

    pub fn add_point(&mut self, light: PointLight) {
        self.point.push(light);
    }

    pub fn add_spot(&mut self, light: SpotLight) {
        self.spot.push(light);
    }

    /// Pack all lights into a GPU uniform. Directional lights come first.
    pub fn to_uniform(&self) -> LightUniform {
        let mut uniform = LightUniform::default();
        let mut idx = 0usize;

        for light in &self.directional {
            if idx >= MAX_LIGHTS { break; }
            uniform.lights[idx] = GpuLight::from_directional(light);
            idx += 1;
        }

        for light in &self.point {
            if idx >= MAX_LIGHTS { break; }
            uniform.lights[idx] = GpuLight::from_point(light);
            idx += 1;
        }

        for light in &self.spot {
            if idx >= MAX_LIGHTS { break; }
            uniform.lights[idx] = GpuLight::from_spot(light);
            idx += 1;
        }

        uniform.num_lights_ambient = [
            idx as f32,
            self.ambient.r,
            self.ambient.g,
            self.ambient.b,
        ];

        uniform
    }

    pub fn total_count(&self) -> usize {
        (self.directional.len() + self.point.len() + self.spot.len()).min(MAX_LIGHTS)
    }

    pub fn first_directional(&self) -> Option<&DirectionalLight> {
        self.directional.first()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_light_size() {
        assert_eq!(std::mem::size_of::<GpuLight>(), 64);
    }

    #[test]
    fn light_uniform_size() {
        assert_eq!(std::mem::size_of::<LightUniform>(), 528);
    }

    #[test]
    fn directional_light_packing() {
        let light = DirectionalLight::new(
            Vec3::new(0.0, -1.0, 0.0),
            Color::WHITE,
            2.0,
        );
        let gpu = GpuLight::from_directional(&light);

        assert_eq!(gpu.position_type[3], 0.0); // type = directional
        assert!((gpu.direction_range[1] - (-1.0)).abs() < 1e-5);
        assert_eq!(gpu.color_intensity[3], 2.0); // intensity
    }

    #[test]
    fn point_light_packing() {
        let light = PointLight::new(
            Vec3::new(1.0, 2.0, 3.0),
            Color::RED,
            5.0,
            10.0,
        );
        let gpu = GpuLight::from_point(&light);

        assert_eq!(gpu.position_type[0], 1.0);
        assert_eq!(gpu.position_type[1], 2.0);
        assert_eq!(gpu.position_type[2], 3.0);
        assert_eq!(gpu.position_type[3], 1.0); // type = point
        assert_eq!(gpu.direction_range[3], 10.0); // range
        assert_eq!(gpu.color_intensity[0], 1.0); // red
        assert_eq!(gpu.color_intensity[3], 5.0); // intensity
    }

    #[test]
    fn spot_light_packing() {
        let inner = std::f32::consts::FRAC_PI_6; // 30 degrees
        let outer = std::f32::consts::FRAC_PI_4; // 45 degrees
        let light = SpotLight::new(
            Vec3::new(0.0, 5.0, 0.0),
            Vec3::new(0.0, -1.0, 0.0),
            Color::GREEN,
            3.0,
            inner,
            outer,
            15.0,
        );
        let gpu = GpuLight::from_spot(&light);

        assert_eq!(gpu.position_type[3], 2.0); // type = spot
        assert_eq!(gpu.direction_range[3], 15.0); // range
        assert!((gpu.spot_params[0] - inner.cos()).abs() < 1e-5);
        assert!((gpu.spot_params[1] - outer.cos()).abs() < 1e-5);
    }

    #[test]
    fn pack_1_directional_2_point_lights() {
        let mut state = LightState::new();
        state.ambient = Color::rgb(0.2, 0.2, 0.2);

        state.add_directional(DirectionalLight::new(
            Vec3::new(0.0, -1.0, -1.0),
            Color::WHITE,
            1.5,
        ));
        state.add_point(PointLight::new(
            Vec3::new(3.0, 1.0, 0.0),
            Color::RED,
            4.0,
            8.0,
        ));
        state.add_point(PointLight::new(
            Vec3::new(-2.0, 3.0, 1.0),
            Color::BLUE,
            2.0,
            6.0,
        ));

        let uniform = state.to_uniform();

        // Count
        assert_eq!(uniform.num_lights_ambient[0] as u32, 3);

        // Ambient
        assert!((uniform.num_lights_ambient[1] - 0.2).abs() < 1e-5);
        assert!((uniform.num_lights_ambient[2] - 0.2).abs() < 1e-5);
        assert!((uniform.num_lights_ambient[3] - 0.2).abs() < 1e-5);

        // Light 0 = directional (type 0)
        assert_eq!(uniform.lights[0].position_type[3], 0.0);
        assert_eq!(uniform.lights[0].color_intensity[3], 1.5);

        // Light 1 = point (type 1)
        assert_eq!(uniform.lights[1].position_type[3], 1.0);
        assert_eq!(uniform.lights[1].position_type[0], 3.0);
        assert_eq!(uniform.lights[1].color_intensity[3], 4.0);
        assert_eq!(uniform.lights[1].direction_range[3], 8.0);

        // Light 2 = point (type 1)
        assert_eq!(uniform.lights[2].position_type[3], 1.0);
        assert_eq!(uniform.lights[2].position_type[0], -2.0);
        assert_eq!(uniform.lights[2].color_intensity[3], 2.0);
        assert_eq!(uniform.lights[2].direction_range[3], 6.0);

        // Verify raw byte layout
        let bytes = bytemuck::bytes_of(&uniform);
        assert_eq!(bytes.len(), 528);

        // Light count at offset 512 (8 lights * 64 bytes)
        let count_bytes = &bytes[512..516];
        let count: f32 = *bytemuck::from_bytes(count_bytes);
        assert_eq!(count as u32, 3);
    }

    #[test]
    fn max_lights_clamped() {
        let mut state = LightState::new();
        for i in 0..20 {
            state.add_point(PointLight::new(
                Vec3::new(i as f32, 0.0, 0.0),
                Color::WHITE,
                1.0,
                5.0,
            ));
        }

        let uniform = state.to_uniform();
        assert_eq!(uniform.num_lights_ambient[0] as u32, 8);
        assert_eq!(state.total_count(), 8);
    }

    #[test]
    fn light_state_clear() {
        let mut state = LightState::new();
        state.add_directional(DirectionalLight::new(Vec3::Y, Color::WHITE, 1.0));
        state.add_point(PointLight::new(Vec3::ZERO, Color::RED, 1.0, 5.0));
        assert_eq!(state.total_count(), 2);

        state.clear();
        assert_eq!(state.total_count(), 0);
        assert_eq!(state.to_uniform().num_lights_ambient[0] as u32, 0);
    }

    #[test]
    fn directional_lights_first_in_uniform() {
        let mut state = LightState::new();
        // Add point first, then directional
        state.add_point(PointLight::new(Vec3::ZERO, Color::RED, 1.0, 5.0));
        state.add_directional(DirectionalLight::new(Vec3::Y, Color::WHITE, 2.0));

        let uniform = state.to_uniform();
        // Directional should be index 0 (sorted first)
        assert_eq!(uniform.lights[0].position_type[3], 0.0); // directional
        assert_eq!(uniform.lights[1].position_type[3], 1.0); // point
    }
}
