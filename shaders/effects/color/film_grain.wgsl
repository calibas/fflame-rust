// Film Grain Effect
//
// Adds animated noise overlay for a filmic look.
// Parameters:
//   params[0] = intensity (0-1): How much grain to add
//   params[1] = size (0.5-4): Grain size multiplier

struct EffectParams {
    // Parameters packed into vec4s for uniform buffer alignment
    // Access as: params[i/4][i%4] or use helper below
    params: array<vec4<f32>, 4>,
    width: u32,
    height: u32,
    time: f32,
    _padding: f32,
}

// Helper to get parameter by index
fn get_param(index: u32) -> f32 {
    return effect_params.params[index / 4u][index % 4u];
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@group(0) @binding(0) var input_texture: texture_2d<f32>;
@group(0) @binding(1) var input_sampler: sampler;
@group(0) @binding(2) var<uniform> effect_params: EffectParams;

// Fullscreen triangle vertex shader
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var output: VertexOutput;

    // Generate fullscreen triangle (no vertex buffer needed)
    // Triangle covers [-1, 3] x [-1, 3] in clip space
    let x = f32((vertex_index & 1u) << 2u);
    let y = f32((vertex_index & 2u) << 1u);

    output.position = vec4<f32>(x - 1.0, 1.0 - y, 0.0, 1.0);
    output.uv = vec2<f32>(x * 0.5, y * 0.5);

    return output;
}

// Hash function for noise generation
fn hash(p: vec2<f32>) -> f32 {
    let p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.13);
    let dot_val = dot(p3, vec3<f32>(p3.y + 19.19, p3.z + 19.19, p3.x + 19.19));
    return fract(dot_val);
}

// Smooth noise
fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);

    // Cubic interpolation
    let u = f * f * (3.0 - 2.0 * f);

    // Four corners
    let a = hash(i + vec2<f32>(0.0, 0.0));
    let b = hash(i + vec2<f32>(1.0, 0.0));
    let c = hash(i + vec2<f32>(0.0, 1.0));
    let d = hash(i + vec2<f32>(1.0, 1.0));

    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Sample input texture
    let color = textureSample(input_texture, input_sampler, input.uv);

    // Get parameters
    let intensity = get_param(0u);
    let size = get_param(1u);

    // Calculate noise coordinates with time animation
    let noise_scale = vec2<f32>(f32(effect_params.width), f32(effect_params.height)) / size;
    let noise_coord = input.uv * noise_scale + effect_params.time * 10.0;

    // Multi-octave noise for more organic grain
    var grain = noise(noise_coord);
    grain += noise(noise_coord * 2.0) * 0.5;
    grain += noise(noise_coord * 4.0) * 0.25;
    grain = grain / 1.75; // Normalize

    // Center around 0 and apply intensity
    grain = (grain - 0.5) * 2.0 * intensity;

    // Apply grain (additive blending, preserve alpha)
    return vec4<f32>(color.rgb + vec3<f32>(grain), color.a);
}
