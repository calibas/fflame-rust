// Sobel Edge Detection Effect
//
// Detects edges using Sobel operator and creates neon glow aesthetic.
// Parameters:
//   params[0] = intensity (0-2): Edge brightness multiplier
//   params[1] = threshold (0-1): Edge detection threshold
//   params[2] = glow (0-1): Amount of edge glow/bloom
//   params[3] = preserve_color (0-1): Blend between edge color and original

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

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var output: VertexOutput;
    let x = f32((vertex_index & 1u) << 2u);
    let y = f32((vertex_index & 2u) << 1u);
    output.position = vec4<f32>(x - 1.0, 1.0 - y, 0.0, 1.0);
    output.uv = vec2<f32>(x * 0.5, y * 0.5);
    return output;
}

fn luminance(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

fn sample_lum(uv: vec2<f32>) -> f32 {
    return luminance(textureSample(input_texture, input_sampler, uv).rgb);
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let intensity = get_param(0u);
    let threshold = get_param(1u);
    let glow = get_param(2u);
    let preserve_color = get_param(3u);

    let texel = vec2<f32>(1.0 / f32(effect_params.width), 1.0 / f32(effect_params.height));
    let uv = input.uv;

    // Sample 3x3 neighborhood luminance
    let n0 = sample_lum(uv + vec2<f32>(-texel.x, -texel.y));
    let n1 = sample_lum(uv + vec2<f32>(0.0, -texel.y));
    let n2 = sample_lum(uv + vec2<f32>(texel.x, -texel.y));
    let n3 = sample_lum(uv + vec2<f32>(-texel.x, 0.0));
    let n5 = sample_lum(uv + vec2<f32>(texel.x, 0.0));
    let n6 = sample_lum(uv + vec2<f32>(-texel.x, texel.y));
    let n7 = sample_lum(uv + vec2<f32>(0.0, texel.y));
    let n8 = sample_lum(uv + vec2<f32>(texel.x, texel.y));

    // Sobel operators
    let gx = n2 + 2.0 * n5 + n8 - (n0 + 2.0 * n3 + n6);
    let gy = n0 + 2.0 * n1 + n2 - (n6 + 2.0 * n7 + n8);

    // Edge magnitude
    var edge = sqrt(gx * gx + gy * gy);

    // Apply threshold
    edge = smoothstep(threshold * 0.5, threshold, edge);

    // Get original color
    let original = textureSample(input_texture, input_sampler, uv);

    // Create neon edge color (use original hue, boost saturation)
    let edge_color = original.rgb * (1.0 + glow * 2.0);

    // Mix edge with original based on edge strength and preserve_color
    let edge_contribution = edge * intensity;
    let result = mix(
        original.rgb * (1.0 - edge_contribution * (1.0 - preserve_color)),
        edge_color,
        edge_contribution
    );

    return vec4<f32>(result, original.a);
}
