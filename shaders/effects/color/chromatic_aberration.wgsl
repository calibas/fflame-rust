// Chromatic Aberration Effect
//
// Separates RGB channels with offset for lens distortion effect.
// Parameters:
//   params[0] = amount (0-20): Pixel offset for R/B channels
//   params[1] = radial (0 or 1): If 1, offset increases toward edges
//   params[2] = intensity (0-1): Blend with original
//   params[3] = blend_mode (0-12): See blend_modes.wgsl for options

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
    let amount = get_param(0u);
    let radial = get_param(1u);
    let intensity = get_param(2u);
    let blend_mode = i32(get_param(3u));

    let original = textureSample(input_texture, input_sampler, input.uv);

    // Calculate offset in UV space
    let pixel_size = vec2<f32>(1.0 / f32(effect_params.width), 1.0 / f32(effect_params.height));

    var offset: vec2<f32>;
    if (radial > 0.5) {
        // Radial mode: offset points away from center, stronger at edges
        let center = vec2<f32>(0.5, 0.5);
        let to_edge = input.uv - center;
        let dist = length(to_edge) * 2.0;
        offset = normalize(to_edge + vec2<f32>(0.0001)) * amount * pixel_size * dist;
    } else {
        // Linear mode: constant horizontal offset
        offset = vec2<f32>(amount * pixel_size.x, 0.0);
    }

    // Sample each channel with offset
    let r = textureSample(input_texture, input_sampler, input.uv + offset).r;
    let g = textureSample(input_texture, input_sampler, input.uv).g;
    let b = textureSample(input_texture, input_sampler, input.uv - offset).b;

    let aberrated = vec3<f32>(r, g, b);

    // Apply blend mode
    let result = apply_blend(original.rgb, aberrated, blend_mode, intensity);

    return vec4<f32>(result, original.a);
}
