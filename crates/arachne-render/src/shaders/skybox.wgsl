// Skybox shader — samples cubemap using vertex position as view direction.
// Rendered as an inverted cube with depth write disabled, z forced to far plane.

struct SkyboxCamera {
    view_rotation_proj: mat4x4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) view_dir: vec3<f32>,
};

@group(0) @binding(0) var<uniform> camera: SkyboxCamera;
@group(1) @binding(0) var t_skybox: texture_cube<f32>;
@group(1) @binding(1) var s_skybox: sampler;

@vertex
fn vs_main(@location(0) position: vec3<f32>) -> VertexOutput {
    var out: VertexOutput;
    out.view_dir = position;
    let clip = camera.view_rotation_proj * vec4<f32>(position, 1.0);
    // Force z = w so depth = 1.0 after perspective divide (maximum depth).
    out.clip_position = clip.xyww;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_skybox, s_skybox, normalize(in.view_dir));
}
