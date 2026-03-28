// SDF text rendering shader with adjustable edge softness and outline.

struct CameraUniform {
    view_proj: mat4x4<f32>,
};

struct TextParams {
    edge_softness: f32,
    outline_width: f32,
    _pad0: f32,
    _pad1: f32,
    outline_color: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

@group(0) @binding(1)
var<uniform> text_params: TextParams;

@group(1) @binding(0)
var t_font: texture_2d<f32>;
@group(1) @binding(1)
var s_font: sampler;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(in.position, 0.0, 1.0);
    out.tex_coords = in.uv;
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let distance = textureSample(t_font, s_font, in.tex_coords).r;
    let edge = text_params.edge_softness;

    // Inner alpha: the glyph fill
    let alpha = smoothstep(0.5 - edge, 0.5 + edge, distance);

    // Outline alpha: wider band around glyph
    let outline_outer = 0.5 - text_params.outline_width;
    let outline_alpha = smoothstep(outline_outer - edge, outline_outer + edge, distance);

    // Blend: outline color outside, fill color inside
    let base_color = mix(text_params.outline_color, in.color, alpha);
    let final_alpha = max(alpha * in.color.a, outline_alpha * text_params.outline_color.a);

    if (final_alpha < 0.01) {
        discard;
    }

    return vec4<f32>(base_color.rgb, final_alpha);
}
