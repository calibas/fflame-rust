// Sharpen Effect
//
// Enhances detail using unsharp mask technique.
// Parameters:
//   params[0] = amount (0-2): Sharpening strength
//   params[1] = radius (0.5-5): Blur radius for unsharp mask

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

// Gaussian weight function
fn gaussian(x: f32, sigma: f32) -> f32 {
    return exp(-(x * x) / (2.0 * sigma * sigma));
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Get parameters
    let amount = get_param(0u);
    let radius = get_param(1u);

    let pixel_size = vec2<f32>(1.0 / f32(effect_params.width), 1.0 / f32(effect_params.height));

    // Sample center pixel
    let center = textureSample(input_texture, input_sampler, input.uv);

    // If no sharpening needed, return original
    if (amount < 0.01) {
        return center;
    }

    // Calculate blurred version using Gaussian blur
    var blur_sum = vec4<f32>(0.0);
    var weight_sum: f32 = 0.0;
    let sigma = radius / 2.0;
    let kernel_size = i32(ceil(radius));

    for (var y: i32 = -kernel_size; y <= kernel_size; y++) {
        for (var x: i32 = -kernel_size; x <= kernel_size; x++) {
            let offset = vec2<f32>(f32(x), f32(y)) * pixel_size;
            let dist = length(vec2<f32>(f32(x), f32(y)));

            if (dist <= radius) {
                let weight = gaussian(dist, sigma);
                let sample_color = textureSample(input_texture, input_sampler, input.uv + offset);
                blur_sum += sample_color * weight;
                weight_sum += weight;
            }
        }
    }

    let blurred = blur_sum / weight_sum;

    // Unsharp mask: original + amount * (original - blurred)
    // This enhances edges by adding back the high-frequency detail
    let sharpened = center + (center - blurred) * amount;

    // Clamp to valid range while preserving alpha
    return vec4<f32>(clamp(sharpened.rgb, vec3<f32>(0.0), vec3<f32>(1.0)), center.a);
}
