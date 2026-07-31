// Density Blur Effect
//
// Blurs low-density areas more than high-density areas.
// Useful for smoothing sparse regions while keeping detail in dense areas.
// Parameters:
//   params[0] = radius (0-10): Maximum blur radius in pixels
//   params[1] = threshold (0-1): Density below which to apply blur
//   params[2] = falloff (0-1): How quickly blur falls off above threshold

struct EffectParams {
    // Parameters packed into vec4s for uniform buffer alignment
    // Access as: params[i/4][i%4] or use helper below
    params: array<vec4<f32>, 12>,
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
    let max_radius = get_param(0u);
    let threshold = get_param(1u);
    let falloff = get_param(2u);

    let pixel_size = vec2<f32>(1.0 / f32(effect_params.width), 1.0 / f32(effect_params.height));

    // Sample center pixel
    let center = textureSample(input_texture, input_sampler, input.uv);
    let density = center.a;

    // Calculate blur amount based on density
    // Low density = more blur, high density = less blur
    var blur_factor: f32;
    if (density < threshold) {
        blur_factor = 1.0;
    } else {
        blur_factor = 1.0 - smoothstep(threshold, threshold + falloff, density);
    }

    let radius = max_radius * blur_factor;

    // If no blur needed, return original
    if (radius < 0.5) {
        return center;
    }

    // Gaussian blur with adaptive radius
    // Use stepped sampling for large radii to maintain O(1) sample count
    // Max ~81 samples (9x9) regardless of radius - GPU bilinear filtering smooths between steps
    var color_sum = vec4<f32>(0.0);
    var weight_sum: f32 = 0.0;
    let sigma = radius / 2.0;

    // Cap kernel iterations to 9x9 max, step size scales with radius
    let max_steps = 4;  // -4 to +4 = 9 steps per axis = 81 samples max
    let step_size = max(1.0, radius / f32(max_steps));

    for (var yi: i32 = -max_steps; yi <= max_steps; yi++) {
        for (var xi: i32 = -max_steps; xi <= max_steps; xi++) {
            let sample_offset = vec2<f32>(f32(xi), f32(yi)) * step_size;
            let dist = length(sample_offset);

            if (dist <= radius) {
                let offset = sample_offset * pixel_size;
                let weight = gaussian(dist, sigma);
                let sample_color = textureSample(input_texture, input_sampler, input.uv + offset);
                color_sum += sample_color * weight;
                weight_sum += weight;
            }
        }
    }

    if (weight_sum > 0.0) {
        return color_sum / weight_sum;
    }
    return center;
}
