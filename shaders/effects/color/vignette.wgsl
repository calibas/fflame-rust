// Vignette Effect
//
// Darkens the edges of the image with a smooth radial falloff.
// Parameters:
//   params[0] = intensity (0-1): Blend with vignette effect
//   params[1] = radius (0-1): How far from center the effect starts
//   params[2] = softness (0-1): How gradual the falloff is
//   params[3] = blend_mode (0-12): See blend_modes.wgsl for options

struct EffectParams {
    params: array<vec4<f32>, 4>,
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
    let radius = get_param(1u);
    let softness = get_param(2u);
    let blend_mode = i32(get_param(3u));

    let original = textureSample(input_texture, input_sampler, input.uv);

    // Calculate distance from center (accounting for aspect ratio)
    let aspect = f32(effect_params.width) / f32(effect_params.height);
    let center = vec2<f32>(0.5, 0.5);
    var uv_corrected = input.uv - center;
    uv_corrected.x *= aspect;
    let dist = length(uv_corrected);

    // Calculate vignette factor (0 = full darken, 1 = no change)
    let vignette_start = radius * 0.5 * max(1.0, aspect);
    let vignette_end = vignette_start + softness * 0.5 * max(1.0, aspect);
    let vignette = 1.0 - smoothstep(vignette_start, vignette_end, dist);

    // Create vignette effect (darkened version)
    let vignette_color = original.rgb * vignette;

    // Apply blend mode
    let result = apply_blend(original.rgb, vignette_color, blend_mode, intensity);

    return vec4<f32>(result, original.a);
}
