// Particle system shaders: compute update + vertex/fragment rendering.

// ---------------------------------------------------------------------------
// Compute shader: particle simulation
// ---------------------------------------------------------------------------

// Field order: vec4 first (align 16), vec2 (align 8), f32 (align 4).
// Matches Rust GpuParticle repr(C) layout with no internal padding.
struct Particle {
    color: vec4<f32>,
    position: vec2<f32>,
    velocity: vec2<f32>,
    age: f32,
    lifetime: f32,
    size: f32,
    rotation: f32,
};

struct SimParams {
    dt: f32,
    gravity_x: f32,
    gravity_y: f32,
    particle_count: u32,
};

@group(0) @binding(0)
var<storage, read> particles_in: array<Particle>;

@group(0) @binding(1)
var<storage, read_write> particles_out: array<Particle>;

@group(0) @binding(2)
var<storage, read_write> alive_counter: atomic<u32>;

@group(0) @binding(3)
var<uniform> params: SimParams;

@compute @workgroup_size(64)
fn cs_update(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if (idx >= params.particle_count) {
        return;
    }

    var p = particles_in[idx];

    // Advance age
    p.age = p.age + params.dt;

    // Kill expired
    if (p.age >= p.lifetime) {
        return;
    }

    // Apply gravity
    p.velocity.x = p.velocity.x + params.gravity_x * params.dt;
    p.velocity.y = p.velocity.y + params.gravity_y * params.dt;

    // Euler integration
    p.position.x = p.position.x + p.velocity.x * params.dt;
    p.position.y = p.position.y + p.velocity.y * params.dt;

    // Compact: write alive particles contiguously using atomic counter
    let out_idx = atomicAdd(&alive_counter, 1u);
    particles_out[out_idx] = p;
}

// ---------------------------------------------------------------------------
// Vertex/Fragment shader: billboard quad rendering
// ---------------------------------------------------------------------------

struct CameraUniform {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

struct VertexInput {
    @location(0) quad_pos: vec2<f32>,
    @location(1) quad_uv: vec2<f32>,
};

struct InstanceInput {
    @location(2) inst_position: vec2<f32>,
    @location(3) inst_size: f32,
    @location(4) inst_rotation: f32,
    @location(5) inst_color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_particle(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    // Apply rotation and scale to the quad vertex
    let c = cos(instance.inst_rotation);
    let s = sin(instance.inst_rotation);
    let rotated = vec2<f32>(
        vertex.quad_pos.x * c - vertex.quad_pos.y * s,
        vertex.quad_pos.x * s + vertex.quad_pos.y * c,
    );
    let scaled = rotated * instance.inst_size;

    // World position = instance position + scaled/rotated quad offset
    let world_pos = vec4<f32>(
        instance.inst_position.x + scaled.x,
        instance.inst_position.y + scaled.y,
        0.0,
        1.0,
    );

    var out: VertexOutput;
    out.clip_position = camera.view_proj * world_pos;
    out.uv = vertex.quad_uv;
    out.color = instance.inst_color;
    return out;
}

@fragment
fn fs_particle(in: VertexOutput) -> @location(0) vec4<f32> {
    // Simple soft circle falloff based on UV distance from center
    let center = vec2<f32>(0.5, 0.5);
    let dist = length(in.uv - center) * 2.0;
    let alpha = saturate(1.0 - dist);
    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}
