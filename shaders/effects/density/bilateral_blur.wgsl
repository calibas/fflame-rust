// Bilateral Blur Effect
//
// Edge-preserving blur that smooths noise while maintaining sharp edges.
// Uses both spatial distance and intensity similarity for weighting.
// Parameters:
//   params[0] = radius (1-10): Blur kernel radius in pixels
//   params[1] = sigma_spatial (1-10): Spatial falloff (larger = more blur)
//   params[2] = sigma_range (0.05-0.5): Range/intensity falloff (smaller = more edge preservation)

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

fn gaussian(x: f32, sigma: f32) -> f32 {
    return exp(-(x * x) / (2.0 * sigma * sigma));
}

fn luminance(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let radius = i32(clamp(get_param(0u), 1.0, 10.0));
    let sigma_spatial = max(0.1, get_param(1u));
    let sigma_range = max(0.01, get_param(2u));

    let texel = vec2<f32>(1.0 / f32(effect_params.width), 1.0 / f32(effect_params.height));

    // Sample center pixel
    let center = textureSample(input_texture, input_sampler, input.uv);
    let center_lum = luminance(center.rgb);

    var color_sum = vec3<f32>(0.0);
    var alpha_sum = 0.0;
    var weight_sum = 0.0;

    // Iterate over kernel
    for (var y = -radius; y <= radius; y++) {
        for (var x = -radius; x <= radius; x++) {
            let offset = vec2<f32>(f32(x), f32(y)) * texel;
            let sample_color = textureSample(input_texture, input_sampler, input.uv + offset);

            // Spatial weight (distance from center)
            let spatial_dist = length(vec2<f32>(f32(x), f32(y)));
            let spatial_w = gaussian(spatial_dist, sigma_spatial);

            // Range weight (intensity similarity)
            let range_dist = abs(center_lum - luminance(sample_color.rgb));
            let range_w = gaussian(range_dist, sigma_range);

            // Combined weight
            let weight = spatial_w * range_w;

            color_sum += sample_color.rgb * weight;
            alpha_sum += sample_color.a * weight;
            weight_sum += weight;
        }
    }

    // Normalize
    let final_color = color_sum / max(weight_sum, 0.0001);
    let final_alpha = alpha_sum / max(weight_sum, 0.0001);

    return vec4<f32>(final_color, final_alpha);
}
