// Tonemap shader for displaying the accumulation buffer
// Applies logarithmic tone mapping and gamma correction

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

struct TonemapParams {
    exposure: f32,
    gamma: f32,
    density_scale: f32,
    tonemap_mode: u32,  // 0 = Linear, 1 = Logarithmic, 2 = DensityVisualization
    background_color: vec3<f32>,
    use_curve: u32,  // 0 = disabled, 1 = enabled
    vibrancy: f32,  // Blend between old and new color algorithms (0.0-1.0)
    _pad: vec3<f32>,  // Padding to vec4 boundary
}

@group(0) @binding(0) var accumulation_texture: texture_2d<f32>;
@group(0) @binding(1) var accumulation_sampler: sampler;
@group(0) @binding(2) var<uniform> tonemap_params: TonemapParams;
@group(0) @binding(3) var curve_lut_texture: texture_1d<f32>;
@group(0) @binding(4) var curve_lut_sampler: sampler;

// Vertex shader for fullscreen quad
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var output: VertexOutput;

    // Generate fullscreen triangle
    let x = f32((vertex_index & 1u) << 2u);
    let y = f32((vertex_index & 2u) << 1u);

    output.position = vec4<f32>(x - 1.0, 1.0 - y, 0.0, 1.0);
    output.uv = vec2<f32>(x * 0.5, y * 0.5);

    return output;
}

// Fragment shader with tone mapping
@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Sample accumulation buffer
    let accum = textureSample(accumulation_texture, accumulation_sampler, input.uv);

    // Extract RGB and alpha (density)
    var color = accum.rgb;
    let density = accum.a;

    // Normalize color by density, but cap density to prevent unbounded growth
    // Density represents total samples * 0.01, so density=1.0 means 100 samples
    // We use sqrt to compress high densities while preserving low-density detail
    // Apply density_scale so user can control the brightness contribution
    let normalized_density = sqrt(density * tonemap_params.density_scale);

    if (density > 0.001) {
        color = color * normalized_density;
    }

    // Apply exposure
    color *= tonemap_params.exposure;

    // Tone mapping (mode-based)
    if (tonemap_params.tonemap_mode == 0u) {
        // Linear tone mapping: simple clamping
        color = clamp(color, vec3<f32>(0.0), vec3<f32>(1.0));
    } else if (tonemap_params.tonemap_mode == 1u) {
        // Logarithmic tone mapping: compress bright areas
        color = log(color + 1.0) / log(10.0);
    } else if (tonemap_params.tonemap_mode == 2u) {
        // Density visualization: show raw accumulated density as grayscale
        // Scale by 0.01 so density=100 shows as white
        color = vec3<f32>(density * 0.01);
    }

    // Gamma correction with vibrancy blend (Apophysis compatibility)
    // Vibrancy blends between old (pre-gamma brightness) and new (post-gamma brightness)
    // The "old" algorithm applies brightness before gamma, "new" applies after gamma
    let vib = tonemap_params.vibrancy;

    if (vib < 1.0) {
        // Old algorithm: brightness * gamma (less vibrant, washed out)
        let old_color = pow(color, vec3<f32>(tonemap_params.gamma));

        // New algorithm: gamma correction only (modern, vibrant)
        let new_color = pow(color, vec3<f32>(1.0 / tonemap_params.gamma));

        // Blend between algorithms
        color = new_color * vib + old_color * (1.0 - vib);
    } else {
        // Full new algorithm (vib=1.0, default)
        color = pow(color, vec3<f32>(1.0 / tonemap_params.gamma));
    }

    // Clamp to valid range
    color = clamp(color, vec3<f32>(0.0), vec3<f32>(1.0));

    // Apply tone curve to fractal color only (not background)
    // Only apply where there's significant fractal density to avoid affecting background
    var fractal_color = color;
    if (tonemap_params.use_curve != 0u && density > 0.001) {
        let r = textureSample(curve_lut_texture, curve_lut_sampler, color.r).r;
        let g = textureSample(curve_lut_texture, curve_lut_sampler, color.g).r;
        let b = textureSample(curve_lut_texture, curve_lut_sampler, color.b).r;
        fractal_color = vec3<f32>(r, g, b);
    }

    // Map density to alpha using density_scale
    // The density represents how many samples hit this pixel
    // density_scale controls transparency: higher = more opaque
    let alpha = clamp(density * tonemap_params.density_scale, 0.0, 1.0);

    // Check if background is black (transparent export mode)
    let bg_sum = tonemap_params.background_color.r + tonemap_params.background_color.g + tonemap_params.background_color.b;
    let is_transparent_mode = bg_sum < 0.001;

    // Composite: background * (1 - alpha) + tone_curved_fractal * alpha
    // This ensures tone curve only affects the fractal layer, not the background
    let final_color = select(
        tonemap_params.background_color * (1.0 - alpha) + fractal_color * alpha,  // Normal mode: manual blend
        fractal_color,                                                              // Transparent mode: just fractal
        is_transparent_mode
    );
    let output_alpha = select(1.0, alpha, is_transparent_mode);

    return vec4<f32>(final_color, output_alpha);
}
