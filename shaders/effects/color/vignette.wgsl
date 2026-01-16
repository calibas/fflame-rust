// Vignette Effect
//
// Darkens the edges of the image with a smooth radial falloff.
// Parameters:
//   params[0] = intensity (0-1): How much to darken edges
//   params[1] = radius (0-1): How far from center the effect starts
//   params[2] = softness (0-1): How gradual the falloff is

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

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Sample input texture
    let color = textureSample(input_texture, input_sampler, input.uv);

    // Get parameters
    let intensity = get_param(0u);
    let radius = get_param(1u);
    let softness = get_param(2u);

    // Calculate distance from center (accounting for aspect ratio)
    let aspect = f32(effect_params.width) / f32(effect_params.height);
    let center = vec2<f32>(0.5, 0.5);
    var uv_corrected = input.uv - center;
    uv_corrected.x *= aspect;
    let dist = length(uv_corrected);

    // Calculate vignette factor
    // At dist < radius: factor = 1 (no darkening)
    // At dist > radius: factor decreases toward 0
    let vignette_start = radius * 0.5 * max(1.0, aspect);
    let vignette_end = vignette_start + softness * 0.5 * max(1.0, aspect);
    let vignette = 1.0 - smoothstep(vignette_start, vignette_end, dist) * intensity;

    // Apply vignette (darken RGB, preserve alpha)
    return vec4<f32>(color.rgb * vignette, color.a);
}
