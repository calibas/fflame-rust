// Film Grain Effect
//
// Adds per-pixel random noise for a filmic look.
// Parameters:
//   params[0] = intensity (0-1): Blend amount with grain
//   params[1] = seed (0-1000): Random seed for variation between frames
//   params[2] = blend_mode (0-12): See blend_modes.wgsl for options

struct EffectParams {
    params: array<vec4<f32>, 12>,
    width: u32,
    height: u32,
    _padding: vec2<f32>,
}

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

// INCLUDE_BLEND_MODES

// High-quality per-pixel hash (no interpolation - true grain look)
fn hash_pixel(pixel: vec2<f32>, seed: f32) -> f32 {
    let p = pixel + vec2<f32>(seed * 127.1, seed * 311.7);
    var h = dot(p, vec2<f32>(127.1, 311.7));
    h = fract(sin(h) * 43758.5453);
    return h;
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var output: VertexOutput;
    let x = f32((vertex_index & 1u) << 2u);
    let y = f32((vertex_index & 2u) << 1u);
    output.position = vec4<f32>(x - 1.0, 1.0 - y, 0.0, 1.0);
    output.uv = vec2<f32>(x * 0.5, y * 0.5);
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let intensity = get_param(0u);
    let seed = get_param(1u);
    let blend_mode = i32(get_param(2u));

    let original = textureSample(input_texture, input_sampler, input.uv);

    // Get pixel coordinates for per-pixel noise
    let pixel = input.uv * vec2<f32>(f32(effect_params.width), f32(effect_params.height));

    // Generate per-pixel random noise
    let grain = hash_pixel(pixel, seed);

    // Create grain color (gray noise centered around 0.5)
    let grain_color = vec3<f32>(grain);

    // Apply blend mode (grain overlays on original)
    let result = apply_blend(original.rgb, grain_color, blend_mode, intensity);

    return vec4<f32>(result, original.a);
}
