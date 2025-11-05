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
    use_curve: u32,  // 0 = disabled, 1 = enabled (completes vec4 with background_color)
    vibrancy: f32,  // Blend between old and new color algorithms (0.0-1.0)
    _pad0: f32,  // Padding for std140 alignment
    _pad1: f32,
    _pad2: f32,
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

    // Vibrancy blend (Apophysis compatibility)
    // Exact formula from ImageMaker.pas:599-621:
    //   vib := round(fcp.vibrancy * 256.0);
    //   notvib := 256 - vib;
    //   if (notvib > 0) then begin
    //     ri := Round(ls * fp[0] + notvib * power(fp[0], gamma));
    //   end else begin
    //     ri := Round(ls * fp[0]);
    //   end;
    //
    // Where:
    //   ls = vib * alpha / fp[3] (ImageMaker.pas:599)
    //      = vibrancy * gamma_corrected_brightness / raw_density
    //   fp[x] = accumulated color from palette lookups
    //   notvib = 256 - vib (ImageMaker.pas:413)
    //
    // The formula blends:
    //   new algorithm: ls * fp[x] (vibrancy-scaled brightness)
    //   old algorithm: power(fp[x], gamma) (gamma-corrected color)
    //
    // When vibrancy = 1.0: notvib = 0, result = ls * fp[x] (pure new/vibrant)
    // When vibrancy = 0.0: notvib = 256, result = ls * fp[x] + 256 * power(fp[x], gamma) (new + old)

    // Convert vibrancy from UI range (0-30) to Apophysis range (0-256)
    // Note: UI shows 0-30, but internally Apophysis uses vibrancy/100 to get 0.0-0.3, then * 256 = 0-76.8
    // Actually: fcp.vibrancy is stored as 0-100 in Apophysis, so vibrancy=1.0 in UI -> 100 -> * 256 = 25600
    // Wait, let me recalculate: vibrancy slider 0-30 -> divide by 100 -> 0.0-0.3 -> * 256 -> 0-76.8
    let vib_normalized = tonemap_params.vibrancy / 100.0;  // Convert UI 0-30 to 0.0-0.3
    let vib = vib_normalized * 256.0;  // Scale to Apophysis range
    let notvib = 256.0 - vib;

    // Calculate ls (vibrancy-scaled brightness)
    // In Apophysis: ls = vib * alpha / fp[3]
    // For us: alpha is gamma-corrected brightness from density, fp[3] is raw density
    // Simplify: ls = vibrancy * brightness
    let brightness = (color.r + color.g + color.b) / 3.0;
    let ls = vib * brightness;

    if (notvib > 0.0) {
        // Blend: ls * color + (notvib/256) * pow(color, gamma)
        let new_algo = ls * color;  // Vibrancy-scaled brightness
        let old_algo = pow(color, vec3<f32>(tonemap_params.gamma));  // Gamma-corrected
        color = new_algo + (notvib / 256.0) * old_algo;
    } else {
        // Full new algorithm (vibrancy >= 1.0)
        color = ls * color;
    }

    // Apply final gamma correction
    color = pow(color, vec3<f32>(1.0 / tonemap_params.gamma));

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
