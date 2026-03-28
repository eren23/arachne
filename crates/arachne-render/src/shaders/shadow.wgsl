// Shadow map depth-only pass — minimal vertex transform, no fragment output.

struct ShadowUniforms {
    light_view_proj: mat4x4<f32>,
    shadow_params: vec4<f32>,
};

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) texcoord: vec2<f32>,
    @location(3) tangent: vec4<f32>,
};

struct InstanceInput {
    @location(4) model_0: vec4<f32>,
    @location(5) model_1: vec4<f32>,
    @location(6) model_2: vec4<f32>,
    @location(7) model_3: vec4<f32>,
};

@group(0) @binding(0) var<uniform> shadow: ShadowUniforms;

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> @builtin(position) vec4<f32> {
    let model = mat4x4<f32>(
        instance.model_0,
        instance.model_1,
        instance.model_2,
        instance.model_3,
    );
    return shadow.light_view_proj * model * vec4<f32>(vertex.position, 1.0);
}
