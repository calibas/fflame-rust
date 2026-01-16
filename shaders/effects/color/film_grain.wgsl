// Film Grain Effect
//
// Adds per-pixel random noise for a filmic look.
// Parameters:
//   params[0] = intensity (0-1): How much grain to add
//   params[1] = seed (0-1000): Random seed for variation between frames

struct EffectParams {
    // Parameters packed into vec4s for uniform buffer alignment
    // Access as: params[i/4][i%4] or use helper below
    params: array<vec4<f32>, 4>,
    width: u32,
    height: u32,
    _padding: vec2<f32>,
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

// High-quality per-pixel hash (no interpolation - true grain look)
fn hash_pixel(pixel: vec2<f32>, seed: f32) -> f32 {
    // Use pixel coordinates + seed for unique noise per pixel
    let p = pixel + vec2<f32>(seed * 127.1, seed * 311.7);
    var h = dot(p, vec2<f32>(127.1, 311.7));
    h = fract(sin(h) * 43758.5453);
    return h;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Sample input texture
    let color = textureSample(input_texture, input_sampler, input.uv);

    // Get parameters
    let intensity = get_param(0u);
    let seed = get_param(1u);

    // Get pixel coordinates for per-pixel noise
    let pixel = input.uv * vec2<f32>(f32(effect_params.width), f32(effect_params.height));

    // Generate per-pixel random noise (no interpolation = true grain)
    let grain = hash_pixel(pixel, seed);

    // Center around 0 (-0.5 to +0.5) and apply intensity
    let noise_value = (grain - 0.5) * intensity;

    // Apply grain (additive blending, preserve alpha)
    return vec4<f32>(color.rgb + vec3<f32>(noise_value), color.a);
}
