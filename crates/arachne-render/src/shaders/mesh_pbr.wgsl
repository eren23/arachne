// PBR mesh shader — Cook-Torrance BRDF
// Up to 8 forward lights, normal mapping, shadow mapping with PCF, ACES tonemapping

struct CameraUniforms {
    view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
};

struct MaterialUniforms {
    albedo: vec4<f32>,
    metallic: f32,
    roughness: f32,
    has_albedo_tex: u32,
    has_normal_map: u32,
    emissive: vec4<f32>,
};

struct Light {
    position_type: vec4<f32>,
    direction_range: vec4<f32>,
    color_intensity: vec4<f32>,
    spot_params: vec4<f32>,
};

struct LightUniforms {
    lights: array<Light, 8>,
    num_lights_ambient: vec4<f32>,
};

struct ShadowUniforms {
    light_view_proj: mat4x4<f32>,
    shadow_params: vec4<f32>,
};

// Vertex input
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) texcoord: vec2<f32>,
    @location(3) tangent: vec4<f32>,
};

// Instance input (model matrix as 4 vec4 columns)
struct InstanceInput {
    @location(4) model_0: vec4<f32>,
    @location(5) model_1: vec4<f32>,
    @location(6) model_2: vec4<f32>,
    @location(7) model_3: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) texcoord: vec2<f32>,
    @location(3) world_tangent: vec3<f32>,
    @location(4) world_bitangent: vec3<f32>,
    @location(5) shadow_pos: vec4<f32>,
};

@group(0) @binding(0) var<uniform> camera: CameraUniforms;

@group(1) @binding(0) var<uniform> material: MaterialUniforms;
@group(1) @binding(1) var t_albedo: texture_2d<f32>;
@group(1) @binding(2) var s_albedo: sampler;
@group(1) @binding(3) var t_normal: texture_2d<f32>;
@group(1) @binding(4) var s_normal: sampler;

@group(2) @binding(0) var<uniform> light_data: LightUniforms;

@group(3) @binding(0) var<uniform> shadow: ShadowUniforms;
@group(3) @binding(1) var t_shadow: texture_depth_2d;
@group(3) @binding(2) var s_shadow: sampler_comparison;

const PI: f32 = 3.14159265359;

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    let model = mat4x4<f32>(
        instance.model_0,
        instance.model_1,
        instance.model_2,
        instance.model_3,
    );

    let world_pos = model * vec4<f32>(vertex.position, 1.0);

    let normal_mat = mat3x3<f32>(
        model[0].xyz,
        model[1].xyz,
        model[2].xyz,
    );

    var out: VertexOutput;
    out.clip_position = camera.view_proj * world_pos;
    out.world_pos = world_pos.xyz;
    out.world_normal = normalize(normal_mat * vertex.normal);
    out.texcoord = vertex.texcoord;
    out.world_tangent = normalize(normal_mat * vertex.tangent.xyz);
    out.world_bitangent = cross(out.world_normal, out.world_tangent) * vertex.tangent.w;
    out.shadow_pos = shadow.light_view_proj * world_pos;

    return out;
}

// ---------------------------------------------------------------------------
// PBR helpers
// ---------------------------------------------------------------------------

fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (vec3<f32>(1.0) - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

fn distribution_ggx(n_dot_h: f32, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let denom = n_dot_h * n_dot_h * (a2 - 1.0) + 1.0;
    return a2 / (PI * denom * denom);
}

fn geometry_schlick_ggx(n_dot: f32, roughness: f32) -> f32 {
    let r = roughness + 1.0;
    let k = (r * r) / 8.0;
    return n_dot / (n_dot * (1.0 - k) + k);
}

fn geometry_smith(n_dot_v: f32, n_dot_l: f32, roughness: f32) -> f32 {
    return geometry_schlick_ggx(n_dot_v, roughness) * geometry_schlick_ggx(n_dot_l, roughness);
}

// ---------------------------------------------------------------------------
// Shadow PCF (3x3 kernel)
// ---------------------------------------------------------------------------

fn shadow_pcf(shadow_pos: vec4<f32>, map_size: f32) -> f32 {
    let proj = shadow_pos.xyz / shadow_pos.w;
    let uv = proj.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5);
    let depth = proj.z;

    if uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 || depth < 0.0 || depth > 1.0 {
        return 1.0;
    }

    let texel_size = 1.0 / map_size;
    var total = 0.0;

    for (var x: i32 = -1; x <= 1; x++) {
        for (var y: i32 = -1; y <= 1; y++) {
            let offset = vec2<f32>(f32(x), f32(y)) * texel_size;
            total += textureSampleCompare(t_shadow, s_shadow, uv + offset, depth);
        }
    }

    return total / 9.0;
}

// ---------------------------------------------------------------------------
// Cook-Torrance contribution for a single light
// ---------------------------------------------------------------------------

fn compute_light(
    light_dir: vec3<f32>,
    light_color: vec3<f32>,
    attenuation: f32,
    N: vec3<f32>,
    V: vec3<f32>,
    albedo: vec3<f32>,
    metallic: f32,
    roughness: f32,
) -> vec3<f32> {
    let L = normalize(light_dir);
    let H = normalize(V + L);

    let n_dot_l = max(dot(N, L), 0.0);
    let n_dot_v = max(dot(N, V), 0.001);
    let n_dot_h = max(dot(N, H), 0.0);
    let h_dot_v = max(dot(H, V), 0.0);

    let f0 = mix(vec3<f32>(0.04), albedo, metallic);

    let D = distribution_ggx(n_dot_h, roughness);
    let G = geometry_smith(n_dot_v, n_dot_l, roughness);
    let F = fresnel_schlick(h_dot_v, f0);

    let specular = (D * G * F) / (4.0 * n_dot_v * n_dot_l + 0.0001);

    let kd = (vec3<f32>(1.0) - F) * (1.0 - metallic);
    let diffuse = kd * albedo / PI;

    return (diffuse + specular) * light_color * attenuation * n_dot_l;
}

// ---------------------------------------------------------------------------
// ACES tonemapping
// ---------------------------------------------------------------------------

fn aces_tonemap(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

// ---------------------------------------------------------------------------
// Fragment
// ---------------------------------------------------------------------------

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var albedo = material.albedo.rgb;
    if material.has_albedo_tex != 0u {
        albedo = textureSample(t_albedo, s_albedo, in.texcoord).rgb;
    }

    var N = normalize(in.world_normal);
    if material.has_normal_map != 0u {
        let tbn = mat3x3<f32>(
            normalize(in.world_tangent),
            normalize(in.world_bitangent),
            N,
        );
        let ns = textureSample(t_normal, s_normal, in.texcoord).rgb;
        N = normalize(tbn * (ns * 2.0 - 1.0));
    }

    let V = normalize(camera.camera_pos.xyz - in.world_pos);
    let metallic = material.metallic;
    let roughness = max(material.roughness, 0.04);

    let shadow_factor = shadow_pcf(in.shadow_pos, shadow.shadow_params.x);

    let num_lights = u32(light_data.num_lights_ambient.x);
    let ambient = light_data.num_lights_ambient.yzw;

    var lo = vec3<f32>(0.0);

    for (var i = 0u; i < 8u; i++) {
        if i >= num_lights { break; }

        let light = light_data.lights[i];
        let light_type = u32(light.position_type.w);
        let light_color = light.color_intensity.xyz * light.color_intensity.w;

        var light_dir: vec3<f32>;
        var atten: f32 = 1.0;

        if light_type == 0u {
            // Directional
            light_dir = -light.direction_range.xyz;
            if i == 0u { atten = shadow_factor; }
        } else if light_type == 1u {
            // Point
            let to_light = light.position_type.xyz - in.world_pos;
            let dist = length(to_light);
            light_dir = to_light / dist;
            let range = light.direction_range.w;
            let falloff = max(1.0 - (dist * dist) / (range * range), 0.0);
            atten = falloff * falloff;
        } else {
            // Spot
            let to_light = light.position_type.xyz - in.world_pos;
            let dist = length(to_light);
            light_dir = to_light / dist;
            let range = light.direction_range.w;
            let falloff = max(1.0 - (dist * dist) / (range * range), 0.0);
            atten = falloff * falloff;

            let spot_dir = normalize(light.direction_range.xyz);
            let theta = dot(-light_dir, spot_dir);
            let inner_cos = light.spot_params.x;
            let outer_cos = light.spot_params.y;
            let epsilon = inner_cos - outer_cos;
            atten *= clamp((theta - outer_cos) / max(epsilon, 0.001), 0.0, 1.0);
        }

        lo += compute_light(light_dir, light_color, atten, N, V, albedo, metallic, roughness);
    }

    var color = ambient * albedo + lo + material.emissive.rgb;

    color = aces_tonemap(color);
    color = pow(color, vec3<f32>(1.0 / 2.2));

    return vec4<f32>(color, material.albedo.a);
}
