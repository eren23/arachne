// Post-processing shader — bloom (threshold + blur), tonemapping, FXAA.

struct PostprocessParams {
    screen_size: vec4<f32>,       // xy = size, zw = 1/size
    bloom_threshold: f32,
    bloom_intensity: f32,
    tonemap_mode: u32,            // 0 = none, 1 = reinhard, 2 = ACES
    fxaa_enabled: u32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@group(0) @binding(0) var t_scene: texture_2d<f32>;
@group(0) @binding(1) var s_scene: sampler;
@group(0) @binding(2) var<uniform> params: PostprocessParams;

// Full-screen triangle via vertex index
@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VertexOutput {
    var out: VertexOutput;
    // 0 -> (-1,-1), 1 -> (3,-1), 2 -> (-1,3)
    let x = f32(i32(vi & 1u) * 4 - 1);
    let y = f32(i32(vi >> 1u) * 4 - 1);
    out.clip_position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>(x * 0.5 + 0.5, 1.0 - (y * 0.5 + 0.5));
    return out;
}

// ---------------------------------------------------------------------------
// Tonemapping
// ---------------------------------------------------------------------------

fn reinhard(c: vec3<f32>) -> vec3<f32> {
    return c / (c + vec3<f32>(1.0));
}

fn aces(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

// ---------------------------------------------------------------------------
// Bloom extraction (bright-pass threshold)
// ---------------------------------------------------------------------------

fn bloom_extract(color: vec3<f32>, threshold: f32) -> vec3<f32> {
    let brightness = dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
    let contribution = max(brightness - threshold, 0.0);
    let factor = contribution / max(brightness, 0.001);
    return color * factor;
}

// Simple 5-tap Gaussian blur
fn bloom_blur(uv: vec2<f32>, pixel_size: vec2<f32>) -> vec3<f32> {
    let offsets = array<f32, 3>(0.0, 1.3846153846, 3.2307692308);
    let weights = array<f32, 3>(0.2270270270, 0.3162162162, 0.0702702703);

    var result = textureSample(t_scene, s_scene, uv).rgb * weights[0];

    for (var i = 1; i < 3; i++) {
        let off = pixel_size * offsets[i];
        result += textureSample(t_scene, s_scene, uv + vec2<f32>(off.x, 0.0)).rgb * weights[i];
        result += textureSample(t_scene, s_scene, uv - vec2<f32>(off.x, 0.0)).rgb * weights[i];
        result += textureSample(t_scene, s_scene, uv + vec2<f32>(0.0, off.y)).rgb * weights[i];
        result += textureSample(t_scene, s_scene, uv - vec2<f32>(0.0, off.y)).rgb * weights[i];
    }

    return result;
}

// ---------------------------------------------------------------------------
// FXAA (simplified single-pass)
// ---------------------------------------------------------------------------

fn luminance(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.299, 0.587, 0.114));
}

fn fxaa(uv: vec2<f32>, pixel_size: vec2<f32>) -> vec3<f32> {
    let center = textureSample(t_scene, s_scene, uv).rgb;
    let luma_c = luminance(center);
    let luma_n = luminance(textureSample(t_scene, s_scene, uv + vec2<f32>(0.0, -pixel_size.y)).rgb);
    let luma_s = luminance(textureSample(t_scene, s_scene, uv + vec2<f32>(0.0, pixel_size.y)).rgb);
    let luma_e = luminance(textureSample(t_scene, s_scene, uv + vec2<f32>(pixel_size.x, 0.0)).rgb);
    let luma_w = luminance(textureSample(t_scene, s_scene, uv + vec2<f32>(-pixel_size.x, 0.0)).rgb);

    let luma_min = min(luma_c, min(min(luma_n, luma_s), min(luma_e, luma_w)));
    let luma_max = max(luma_c, max(max(luma_n, luma_s), max(luma_e, luma_w)));
    let luma_range = luma_max - luma_min;

    if luma_range < max(0.0312, luma_max * 0.125) {
        return center;
    }

    let dir_x = -((luma_n + luma_s) - (luma_e + luma_w));
    let dir_y = (luma_n + luma_e) - (luma_s + luma_w);
    let dir_reduce = max((luma_n + luma_s + luma_e + luma_w) * 0.25 * 0.25, 1.0 / 128.0);
    let rcp_dir_min = 1.0 / (min(abs(dir_x), abs(dir_y)) + dir_reduce);
    let dir = clamp(
        vec2<f32>(dir_x, dir_y) * rcp_dir_min,
        vec2<f32>(-8.0),
        vec2<f32>(8.0),
    ) * pixel_size;

    let a = textureSample(t_scene, s_scene, uv + dir * (1.0 / 3.0 - 0.5)).rgb;
    let b = textureSample(t_scene, s_scene, uv + dir * (2.0 / 3.0 - 0.5)).rgb;
    let result_ab = (a + b) * 0.5;

    let c = textureSample(t_scene, s_scene, uv + dir * -0.5).rgb;
    let d = textureSample(t_scene, s_scene, uv + dir * 0.5).rgb;
    let result_cd = result_ab * 0.5 + (c + d) * 0.25;

    let luma_cd = luminance(result_cd);
    if luma_cd < luma_min || luma_cd > luma_max {
        return result_ab;
    }
    return result_cd;
}

// ---------------------------------------------------------------------------
// Fragment
// ---------------------------------------------------------------------------

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let pixel_size = params.screen_size.zw;
    var color: vec3<f32>;

    // FXAA
    if params.fxaa_enabled != 0u {
        color = fxaa(in.uv, pixel_size);
    } else {
        color = textureSample(t_scene, s_scene, in.uv).rgb;
    }

    // Bloom
    if params.bloom_intensity > 0.0 {
        let bloom = bloom_extract(bloom_blur(in.uv, pixel_size), params.bloom_threshold);
        color += bloom * params.bloom_intensity;
    }

    // Tonemapping
    if params.tonemap_mode == 1u {
        color = reinhard(color);
    } else if params.tonemap_mode == 2u {
        color = aces(color);
    }

    return vec4<f32>(color, 1.0);
}
