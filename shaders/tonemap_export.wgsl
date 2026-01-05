// Tonemap shader for export - simplified version without path buffer/palette bindings
// Used by high_res.rs export system

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
    _pad_bg: f32,  // Padding to align vec3 to 16 bytes (std140 rule)
    use_curve: u32,  // 0 = disabled, 1 = enabled
    vibrancy: f32,  // Blend between old and new color algorithms (0.0-30.0)
    brightness: f32,  // Logarithmic brightness scaling (0.0-5.0, default 1.0)
    white_level: f32,  // Apophysis white_level constant (default 200.0)
    prefilter_white: f32,  // Apophysis PREFILTER_WHITE constant (67108864.0)
    bright_adjust: f32,  // Apophysis BRIGHT_ADJUST constant (2.3)
    area: f32,  // Render area (width * height)
    sample_density: f32,  // Iterations per pixel
    saturation: f32,  // Color saturation boost (1.0 = no change, >1.0 = more saturated)
    hue_shift: f32,  // Hue rotation in degrees (-180.0 to 180.0)
    gamma_threshold: f32,  // Smooths gamma curve at low densities (default 0.0025)
    alpha_blend_low: f32,  // Start blending toward linear alpha at this value
    alpha_blend_high: f32,  // Full linear alpha above this value
    transparent_mode: u32,  // 0 = normal (blend with background), 1 = transparent export
    color_mode: u32,  // 0 = palette, 1 = speed, 2 = path_map (not used in export)
    width: u32,  // Texture width (unused in export)
    height: u32,  // Texture height (unused in export)
    path_map_style: u32,  // Unused in export
    burn_in: u32,  // Unused in export
    num_transforms: u32,  // Unused in export
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
    _pad3: u32,
}

@group(0) @binding(0) var accumulation_texture: texture_2d<f32>;
@group(0) @binding(1) var accumulation_sampler: sampler;
@group(0) @binding(2) var<uniform> tonemap_params: TonemapParams;
@group(0) @binding(3) var curve_lut_texture: texture_2d<f32>;
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

// Helper function: Calculate brightness scaling factor from logarithmic curve
fn brightness_scale(count: f32) -> f32 {
    let contrast = 1.0;
    let k1 = contrast * tonemap_params.bright_adjust * tonemap_params.brightness * 268.0 * tonemap_params.prefilter_white / 256.0;
    let k2 = 1.0 / (contrast * tonemap_params.area * tonemap_params.white_level * tonemap_params.sample_density);

    if (count < 0.001) {
        return 0.0;
    } else {
        let log10_value = log(1.0 + tonemap_params.white_level * count * k2) / log(10.0);
        return (k1 * log10_value) / (tonemap_params.white_level * count);
    }
}

// Helper function: Convert RGB to HSV
fn rgb_to_hsv(rgb: vec3<f32>) -> vec3<f32> {
    let r = rgb.r;
    let g = rgb.g;
    let b = rgb.b;

    let max_val = max(max(r, g), b);
    let min_val = min(min(r, g), b);
    let delta = max_val - min_val;

    var h = 0.0;
    var s = 0.0;
    let v = max_val;

    if (delta > 0.00001) {
        s = delta / max_val;

        if (r >= max_val) {
            h = (g - b) / delta;
        } else if (g >= max_val) {
            h = 2.0 + (b - r) / delta;
        } else {
            h = 4.0 + (r - g) / delta;
        }

        h = h * 60.0;
        if (h < 0.0) {
            h = h + 360.0;
        }
    }

    return vec3<f32>(h, s, v);
}

// Helper function: Convert HSV to RGB
fn hsv_to_rgb(hsv: vec3<f32>) -> vec3<f32> {
    let h = hsv.x;
    let s = hsv.y;
    let v = hsv.z;

    if (s <= 0.0) {
        return vec3<f32>(v, v, v);
    }

    var hh = h;
    if (hh >= 360.0) {
        hh = 0.0;
    }
    hh = hh / 60.0;

    let i = u32(hh);
    let ff = hh - f32(i);
    let p = v * (1.0 - s);
    let q = v * (1.0 - (s * ff));
    let t = v * (1.0 - (s * (1.0 - ff)));

    switch (i) {
        case 0u: { return vec3<f32>(v, t, p); }
        case 1u: { return vec3<f32>(q, v, p); }
        case 2u: { return vec3<f32>(p, v, t); }
        case 3u: { return vec3<f32>(p, q, v); }
        case 4u: { return vec3<f32>(t, p, v); }
        default: { return vec3<f32>(v, p, q); }
    }
}

// Fragment shader with Apophysis-compatible tone mapping
@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Sample accumulation buffer
    let accum = textureSample(accumulation_texture, accumulation_sampler, input.uv);

    // Extract accumulated RGB and density
    let bucket_count = accum.a * 100.0;  // Scale back from 0.01 per hit

    // Check if pixel is empty
    let is_empty = bucket_count < 0.001;

    // Convert averaged colors back to raw accumulated sums
    let bucket_red = accum.r * bucket_count;
    let bucket_green = accum.g * bucket_count;
    let bucket_blue = accum.b * bucket_count;

    // Variables for tone mapping output
    var color: vec3<f32>;
    var alpha: f32 = 0.0;

    // TONE MAP MODE BRANCHING
    if (tonemap_params.tonemap_mode == 0u) {
        // LINEAR MODE
        color = accum.rgb * tonemap_params.exposure;
        let gamma = select(1.0 / tonemap_params.gamma, tonemap_params.gamma, tonemap_params.gamma == 0.0);
        color = pow(color, vec3<f32>(gamma));
        alpha = clamp(bucket_count * 0.01 * tonemap_params.density_scale, 0.0, 1.0);

    } else if (tonemap_params.tonemap_mode == 2u) {
        // DENSITY VISUALIZATION MODE
        let normalized_density = clamp(bucket_count * 0.01 * tonemap_params.exposure, 0.0, 1.0);
        let gamma = select(1.0 / tonemap_params.gamma, tonemap_params.gamma, tonemap_params.gamma == 0.0);
        let gamma_density = pow(normalized_density, gamma);
        color = vec3<f32>(gamma_density, gamma_density, gamma_density);
        alpha = normalized_density;

    } else {
        // LOGARITHMIC MODE (DEFAULT - Apophysis compatible)

        // Apply Brightness to Palette Colors
        var ls = brightness_scale(bucket_count) / tonemap_params.prefilter_white;

        var fp0 = ls * bucket_red;
        var fp1 = ls * bucket_green;
        var fp2 = ls * bucket_blue;
        let fp3 = ls * bucket_count * tonemap_params.white_level;

        // Apply Gamma to Density
        let gamma = select(1.0 / tonemap_params.gamma, tonemap_params.gamma, tonemap_params.gamma == 0.0);

        var funcval = 0.0;
        if (tonemap_params.gamma_threshold != 0.0) {
            funcval = pow(tonemap_params.gamma_threshold, gamma - 1.0);
        }

        if (fp3 > 0.0) {
            if (fp3 <= tonemap_params.gamma_threshold) {
                let frac = fp3 / tonemap_params.gamma_threshold;
                alpha = (1.0 - frac) * fp3 * funcval + frac * pow(fp3, gamma);
            } else {
                alpha = pow(fp3, gamma);
            }
        }

        // Calculate Vibrancy-Weighted Multiplier
        let vib = round(tonemap_params.vibrancy * 256.0);
        let notvib = 256.0 - vib;

        if (fp3 > 0.0) {
            ls = vib * alpha / fp3;
        } else {
            ls = 0.0;
        }

        // Vibrancy Blend
        if (notvib > 0.0) {
            let new_r = ls * fp0;
            let new_g = ls * fp1;
            let new_b = ls * fp2;

            let old_r = notvib * pow(fp0, gamma);
            let old_g = notvib * pow(fp1, gamma);
            let old_b = notvib * pow(fp2, gamma);

            color = vec3<f32>(new_r + old_r, new_g + old_g, new_b + old_b);
        } else {
            color = vec3<f32>(ls * fp0, ls * fp1, ls * fp2);
        }
    }

    // HSV Adjustments
    let needs_hsv = tonemap_params.saturation != 1.0 || tonemap_params.hue_shift != 0.0;
    if (needs_hsv) {
        var hsv = rgb_to_hsv(color);

        if (tonemap_params.hue_shift != 0.0) {
            hsv.x = hsv.x + tonemap_params.hue_shift;
            if (hsv.x < 0.0) {
                hsv.x = hsv.x + 360.0;
            } else if (hsv.x >= 360.0) {
                hsv.x = hsv.x - 360.0;
            }
        }

        if (tonemap_params.saturation != 1.0) {
            hsv.y = clamp(hsv.y * tonemap_params.saturation, 0.0, 1.0);
        }

        color = hsv_to_rgb(hsv);
    }

    // Apply exposure for Logarithmic mode only
    if (tonemap_params.tonemap_mode == 1u) {
        color *= tonemap_params.exposure;
    }

    // Clamp to valid range
    color = clamp(color, vec3<f32>(0.0), vec3<f32>(1.0));

    // Apply tone curve
    let curve_r = textureSample(curve_lut_texture, curve_lut_sampler, vec2<f32>(color.r, 0.5)).r;
    let curve_g = textureSample(curve_lut_texture, curve_lut_sampler, vec2<f32>(color.g, 0.5)).r;
    let curve_b = textureSample(curve_lut_texture, curve_lut_sampler, vec2<f32>(color.b, 0.5)).r;

    let should_apply_curve = tonemap_params.use_curve != 0u && bucket_count > 0.001;
    var fractal_color = select(color, vec3<f32>(curve_r, curve_g, curve_b), should_apply_curve);

    // Background Blending
    let linear_alpha = clamp(bucket_count * 0.01 * tonemap_params.density_scale, 0.0, 1.0);
    let gamma_alpha = clamp(alpha, 0.0, 1.0);

    let blend_t = smoothstep(tonemap_params.alpha_blend_low, tonemap_params.alpha_blend_high, gamma_alpha);
    let fractal_alpha = mix(gamma_alpha, linear_alpha, blend_t);

    var final_color: vec3<f32>;
    var final_alpha: f32;

    if (tonemap_params.transparent_mode != 0u) {
        final_color = fractal_color;
        final_alpha = fractal_alpha;
    } else {
        final_color = tonemap_params.background_color * (1.0 - fractal_alpha) + fractal_color * fractal_alpha;
        final_alpha = 1.0;
    }

    // Convert from linear to sRGB for display
    let srgb_color = pow(final_color, vec3<f32>(1.0 / 2.2));

    return vec4<f32>(srgb_color, final_alpha);
}
