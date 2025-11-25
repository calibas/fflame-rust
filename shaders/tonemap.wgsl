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
    value_scale: f32,  // Value (brightness) multiplier (1.0 = no change)
    gamma_threshold: f32,  // Smooths gamma curve at low densities (default 0.0025)
    alpha_blend_low: f32,  // Start blending toward linear alpha at this value
    alpha_blend_high: f32,  // Full linear alpha above this value
    _pad_alpha: vec2<f32>,  // Padding to align to 16 bytes
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
// This implements the Apophysis lsa[] lookup table inline
fn brightness_scale(count: f32) -> f32 {
    // Calculate k1 and k2 (simplified: contrast=1, oversample=1)
    let contrast = 1.0;
    // k1 = (contrast * BRIGHT_ADJUST * brightness * 268 * PREFILTER_WHITE) / 256.0
    let k1 = contrast * tonemap_params.bright_adjust * tonemap_params.brightness * 268.0 * tonemap_params.prefilter_white / 256.0;

    // k2 = (oversample^2) / (contrast * area * white_level * sample_density)
    // Simplified: oversample=1, contrast=1
    let k2 = 1.0 / (contrast * tonemap_params.area * tonemap_params.white_level * tonemap_params.sample_density);

    if (count < 0.001) {
        return 0.0;
    } else {
        // lsa[i] = (k1 * log10(1 + white_level * i * k2)) / (white_level * i)
        // WGSL doesn't have log10, so convert: log10(x) = log(x) / log(10)
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
    // ===== ALL TEXTURE SAMPLING MUST HAPPEN FIRST =====
    // Chrome WebGPU requires textureSample to be in uniform control flow
    // Any branching on texture data makes subsequent textureSample calls non-uniform

    // Sample accumulation buffer
    let accum = textureSample(accumulation_texture, accumulation_sampler, input.uv);

    // Extract accumulated RGB and density (analogous to bucket.Red/Green/Blue/Count)
    // NOTE: Our accumulation buffer stores AVERAGED colors (sum/count), but Apophysis uses raw SUMS
    // So we need to multiply back by count to get the raw accumulated values
    let bucket_count = accum.a * 100.0;  // Scale back from 0.01 per hit

    // Check if pixel is empty (Chrome WebGPU: avoid early return to keep uniform control flow for textureSample)
    let is_empty = bucket_count < 0.001;

    // Convert averaged colors back to raw accumulated sums (Apophysis bucket format)
    let bucket_red = accum.r * bucket_count;
    let bucket_green = accum.g * bucket_count;
    let bucket_blue = accum.b * bucket_count;

    // ===== STAGE 3A: Apply Brightness to Palette Colors =====
    // Calculate brightness scaling factor (ls) from logarithmic curve
    var ls = brightness_scale(bucket_count) / tonemap_params.prefilter_white;

    // Apply brightness scaling to accumulated color sums
    var fp0 = ls * bucket_red;     // brightness-scaled red
    var fp1 = ls * bucket_green;   // brightness-scaled green
    var fp2 = ls * bucket_blue;    // brightness-scaled blue
    let fp3 = ls * bucket_count * tonemap_params.white_level;  // weighted density

    // ===== STAGE 3B: Apply Gamma to Density =====
    // Invert gamma (Apophysis ImageMaker.pas:410)
    let gamma = select(1.0 / tonemap_params.gamma, tonemap_params.gamma, tonemap_params.gamma == 0.0);

    // Pre-calculate funcval for gamma threshold (Apophysis setup phase)
    // funcval = gamma_threshold ^ (gamma - 1)
    var funcval = 0.0;
    if (tonemap_params.gamma_threshold != 0.0) {
        funcval = pow(tonemap_params.gamma_threshold, gamma - 1.0);
    }

    // Apply gamma to density with threshold smoothing
    var alpha = 0.0;
    if (fp3 > 0.0) {
        if (fp3 <= tonemap_params.gamma_threshold) {
            // Blend between linear and gamma curves at low densities
            let frac = fp3 / tonemap_params.gamma_threshold;
            alpha = (1.0 - frac) * fp3 * funcval + frac * pow(fp3, gamma);
        } else {
            // Standard gamma curve
            alpha = pow(fp3, gamma);
        }
    }

    // ===== STAGE 3C: Calculate Vibrancy-Weighted Multiplier (REUSE ls!) =====
    // Scale vibrancy to Apophysis range (ImageMaker.pas:412)
    let vib = round(tonemap_params.vibrancy * 256.0);
    let notvib = 256.0 - vib;

    // Calculate vibrancy-weighted brightness multiplier (ImageMaker.pas:599)
    // IMPORTANT: ls is OVERWRITTEN here with a new meaning!
    if (fp3 > 0.0) {
        ls = vib * alpha / fp3;
    } else {
        ls = 0.0;
    }

    // ===== STAGE 3D: Vibrancy Blend =====
    // Blend between new (gamma on brightness) and old (gamma on colors) algorithms
    // ImageMaker.pas:612-621
    var color: vec3<f32>;
    if (notvib > 0.0) {
        // NEW algorithm: ls * fp[x] (vibrancy-weighted brightness × brightness-scaled color)
        let new_r = ls * fp0;
        let new_g = ls * fp1;
        let new_b = ls * fp2;

        // OLD algorithm: notvib * power(fp[x], gamma) (gamma applied to colors)
        let old_r = notvib * pow(fp0, gamma);
        let old_g = notvib * pow(fp1, gamma);
        let old_b = notvib * pow(fp2, gamma);

        // Additive blend
        color = vec3<f32>(new_r + old_r, new_g + old_g, new_b + old_b);
    } else {
        // Pure new algorithm (vibrancy >= 256)
        color = vec3<f32>(ls * fp0, ls * fp1, ls * fp2);
    }

    // ===== STAGE 3E: HSV Adjustments =====
    // Apply hue shift, saturation boost, and value scaling
    // Only convert to HSV if at least one adjustment is active
    let needs_hsv = tonemap_params.saturation != 1.0 || tonemap_params.hue_shift != 0.0 || tonemap_params.value_scale != 1.0;
    if (needs_hsv) {
        var hsv = rgb_to_hsv(color);

        // Hue shift (rotate hue around color wheel)
        if (tonemap_params.hue_shift != 0.0) {
            hsv.x = hsv.x + tonemap_params.hue_shift;
            // Wrap hue to 0-360 range
            if (hsv.x < 0.0) {
                hsv.x = hsv.x + 360.0;
            } else if (hsv.x >= 360.0) {
                hsv.x = hsv.x - 360.0;
            }
        }

        // Saturation boost
        if (tonemap_params.saturation != 1.0) {
            hsv.y = clamp(hsv.y * tonemap_params.saturation, 0.0, 1.0);
        }

        // Value scaling (brightness multiplier)
        if (tonemap_params.value_scale != 1.0) {
            hsv.z = clamp(hsv.z * tonemap_params.value_scale, 0.0, 1.0);
        }

        color = hsv_to_rgb(hsv);
    }

    // Apply exposure (our extension, not in Apophysis)
    color *= tonemap_params.exposure;

    // Clamp to valid range
    color = clamp(color, vec3<f32>(0.0), vec3<f32>(1.0));

    // Apply tone curve to fractal color only (not background)
    // Sample curve LUT unconditionally (WebGPU requires textureSample in uniform control flow)
    let curve_r = textureSample(curve_lut_texture, curve_lut_sampler, vec2<f32>(color.r, 0.5)).r;
    let curve_g = textureSample(curve_lut_texture, curve_lut_sampler, vec2<f32>(color.g, 0.5)).r;
    let curve_b = textureSample(curve_lut_texture, curve_lut_sampler, vec2<f32>(color.b, 0.5)).r;

    // Only apply curve where there's significant fractal density
    let should_apply_curve = tonemap_params.use_curve != 0u && bucket_count > 0.001;
    var fractal_color = select(color, vec3<f32>(curve_r, curve_g, curve_b), should_apply_curve);

    // ===== STAGE 3F: Background Blending =====
    // We need an alpha curve that:
    // 1. Rises quickly at low densities (avoids dark halos at edges)
    // 2. Preserves variation in mid-range (maintains detail)
    //
    // Strategy: Blend between gamma-corrected alpha (good edges) and linear (good detail)
    // At low density: use mostly gamma alpha (fast rise, no halos)
    // At high density: use mostly linear alpha (preserves detail)
    // Adjustable via alpha_blend_low and alpha_blend_high sliders
    let linear_alpha = clamp(bucket_count * 0.01 * tonemap_params.density_scale, 0.0, 1.0);
    let gamma_alpha = clamp(alpha, 0.0, 1.0);

    // Blend factor controlled by sliders: transition from gamma to linear alpha
    let blend_t = smoothstep(tonemap_params.alpha_blend_low, tonemap_params.alpha_blend_high, gamma_alpha);
    let fractal_alpha = mix(gamma_alpha, linear_alpha, blend_t);

    // Alpha composite using gamma-corrected density (Apophysis method)
    // This ensures the tonemapping and background blend use the same density curve
    let final_color = tonemap_params.background_color * (1.0 - fractal_alpha) + fractal_color * fractal_alpha;
    let final_alpha = 1.0;

    // Convert from linear to sRGB for display
    // (Rgba8Unorm is linear, but monitors expect sRGB)
    let srgb_color = pow(final_color, vec3<f32>(1.0 / 2.2));

    return vec4<f32>(srgb_color, final_alpha);
}
