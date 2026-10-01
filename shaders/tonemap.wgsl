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
    gamma_threshold: f32,  // Smooths gamma curve at low densities (default 0.0025)
    alpha_blend_low: f32,  // Start blending toward linear alpha at this value
    alpha_blend_high: f32,  // Full linear alpha above this value
    transparent_mode: u32,  // 0 = normal (blend with background), 1 = transparent export
    color_mode: u32,  // 0 = palette, 1 = speed, 2 = path_map
    width: u32,  // Texture width for path buffer indexing
    height: u32,  // Texture height for path buffer indexing
    // Read by nothing since PathMap colours in the compute pass (its
    // paths carry their colour); kept so the fields below stay put.
    path_map_style: u32,
    burn_in: u32,
    // Uploaded from `flame.transforms.len()`, currently read by
    // nothing. Kept because removing it would shift every field below
    // it by four bytes, and a WGSL-only edit would still validate --
    // the struct's 16-byte alignment absorbs the loss -- while
    // silently reading `palette_size` out of this slot.
    num_transforms: u32,
    palette_size: u32,  // Palette texture size (256-4096), for shader index calculations
    // Levels controls (histogram-based density remapping)
    // Note: density = background/transparent, NOT black/dark!
    levels_low: f32,  // Density below this becomes fully transparent/background
    levels_high: f32,  // Density above this becomes fully opaque
    levels_gamma: f32,  // Gamma for density curve (1.0 = linear)
    highlight_mode: u32,  // 0 = Clip (per-channel clamp, Apophysis), 1 = MaxNorm (hue-preserving)
    levels_enabled: u32,  // 0 = Levels off (Apo-matching, alpha bypasses opacity remap), 1 = on
    // Two trailing scalar u32s rather than `vec2<u32>`: keep std140/std430
    // layout aligned to the Rust packing — see comment on the Rust side.
    // The in-frame mean density Levels is measured against; see the
    // Rust field for why it is not `sample_density`.
    levels_density: f32,
    _pad_levels_1: u32,
}

@group(0) @binding(0) var accumulation_texture: texture_2d<f32>;
@group(0) @binding(1) var accumulation_sampler: sampler;
@group(0) @binding(2) var<uniform> tonemap_params: TonemapParams;
@group(0) @binding(3) var curve_lut_texture: texture_2d<f32>;
@group(0) @binding(4) var curve_lut_sampler: sampler;
// Bindings 5-7 (the path buffer and the palette) are still in the layout
// but read by nothing: PathMap colours in the compute pass now, where its
// paths carry their colour (docs/projects/word-editing.md §10).

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

// sRGB → linear decoding. Palette colors (.flame XML hex, .palette JSON)
// and the background color in `TonemapParams` are authored against an sRGB
// monitor; the pipeline math is linear. Use the gamma-2.2 approximation
// so it's symmetric with the pow(1/2.2) encoder at the fragment-shader
// tail (encode∘decode = identity on round-tripped values).
fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    return pow(max(c, vec3<f32>(0.0)), vec3<f32>(2.2));
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

// Helper function: Apply Levels transformation to density
// Maps density from [levels_low, levels_high] range to [0, 1] with gamma adjustment
// Note: This affects transparency/opacity, NOT brightness!
// - Low density → background color (transparent)
// - High density → fractal color (opaque)
//
// `levels_low` / `levels_high` are expressed in multiples of mean
// density (i.e., × `sample_density = total_iters / pixel_count`),
// not in raw cumulative-count units. This makes the slider's effect
// invariant to iteration count: `levels_high = 1.0` means "clip at
// the mean" whether the render has accumulated 1M or 200B iterations.
fn apply_levels(density: f32) -> f32 {
    // Get levels parameters (in × mean density units)
    let low = tonemap_params.levels_low;
    let high = tonemap_params.levels_high;
    let gamma = tonemap_params.levels_gamma;
    let mean = tonemap_params.levels_density;

    // First frame / empty buffer: no clipping (let base_alpha decide)
    if (mean <= 0.0) {
        return 1.0;
    }

    // Normalize density into "× mean density" units
    let normalized_density = density / mean;

    // Avoid division by zero
    if (high <= low) {
        return select(0.0, 1.0, normalized_density > low);
    }

    // Remap normalized density from [low, high] to [0, 1]
    let normalized = (normalized_density - low) / (high - low);
    let clamped = clamp(normalized, 0.0, 1.0);

    // Apply gamma curve (gamma=1.0 is linear, <1.0 compresses toward opaque, >1.0 compresses toward transparent)
    if (gamma != 1.0 && gamma > 0.0) {
        return pow(clamped, gamma);
    }
    return clamped;
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

    // Point-fetch the accumulation buffer. The texture is Rgba32Float
    // (Phase 8c precision fix) which isn't filterable without the
    // FLOAT32_FILTERABLE feature; for a 1:1 fullscreen pass `textureLoad`
    // is equivalent to a non-filtering sample anyway.
    let accum_coord = vec2<i32>(i32(input.uv.x * f32(tonemap_params.width)), i32(input.uv.y * f32(tonemap_params.height)));
    let accum = textureLoad(accumulation_texture, accum_coord, 0);

    // Extract accumulated RGB and density (analogous to bucket.Red/Green/Blue/Count).
    // accum.rgb is the running mean color (sum/count) in [0,1]; accum.a is
    // the raw cumulative iteration count for this pixel (Phase 8b).
    //
    // The 100× factor is a tonemap calibration constant. With the
    // scale-invariant `sample_density = total_iters / pixel_count`
    // formula, k2 ≈ 1/(area × white_level × sample_density), so
    // `count × white_level × k2 ≈ count / (area × sample_density)`
    // — for a "mid-density" pixel where count ≈ sample_density this
    // is ≈ 1/area, which on typical viewports is tiny (~1e-3) and
    // produces dim log() output. Multiplying by 100 here pushes the
    // log argument into the useful range (~0.1 - 10) so brightness
    // and exposure defaults map onto perceivable values. Pre-Phase 8b
    // this same `× 100` was conceptually "undoing the 0.01 scale in
    // the EMA accumulator's alpha update"; the storage scale is gone
    // but the calibration role is the same.
    let bucket_count = accum.a * 100.0;

    // Check if pixel is empty (Chrome WebGPU: avoid early return to keep uniform control flow for textureSample)
    let is_empty = bucket_count < 0.001;

    // Convert averaged colors back to raw accumulated sums (Apophysis bucket format)
    let bucket_red = accum.r * bucket_count;
    let bucket_green = accum.g * bucket_count;
    let bucket_blue = accum.b * bucket_count;

    // Variables for tone mapping output
    var color: vec3<f32>;
    var alpha: f32 = 0.0;

    // ===== TONE MAP MODE BRANCHING =====
    // 0 = Linear, 1 = Logarithmic (Apophysis), 2 = Density Visualization
    if (tonemap_params.tonemap_mode == 0u) {
        // ===== LINEAR MODE =====
        // Simple linear scaling with gamma correction
        // Good for low-dynamic-range flames or when you want direct control

        // Apply exposure directly to averaged colors (no logarithmic curve)
        color = accum.rgb * tonemap_params.exposure;

        // Simple gamma correction
        let gamma = select(1.0 / tonemap_params.gamma, tonemap_params.gamma, tonemap_params.gamma == 0.0);
        color = pow(color, vec3<f32>(gamma));

        // Alpha from density with simple scaling
        alpha = clamp(bucket_count * 0.01 * tonemap_params.density_scale, 0.0, 1.0);

    } else if (tonemap_params.tonemap_mode == 2u) {
        // ===== DENSITY VISUALIZATION MODE =====
        // Shows raw density as grayscale - useful for debugging and analysis

        // Normalize density to visible range using exposure as sensitivity
        let normalized_density = clamp(bucket_count * 0.01 * tonemap_params.exposure, 0.0, 1.0);

        // Apply gamma for better visibility of low-density areas
        let gamma = select(1.0 / tonemap_params.gamma, tonemap_params.gamma, tonemap_params.gamma == 0.0);
        let gamma_density = pow(normalized_density, gamma);

        // Output as grayscale
        color = vec3<f32>(gamma_density, gamma_density, gamma_density);

        // Alpha matches density
        alpha = normalized_density;

    } else {
        // ===== LOGARITHMIC MODE (DEFAULT - Apophysis compatible) =====

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
    }

    // ===== STAGE 3E: HSV Adjustments =====
    // Apply hue shift and saturation boost
    // Only convert to HSV if at least one adjustment is active
    let needs_hsv = tonemap_params.saturation != 1.0 || tonemap_params.hue_shift != 0.0;
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

        color = hsv_to_rgb(hsv);
    }

    // Apply exposure for Logarithmic mode only (Linear mode applies it earlier)
    // Density mode uses exposure as sensitivity, already applied
    if (tonemap_params.tonemap_mode == 1u) {
        color *= tonemap_params.exposure;
    }

    // Map HDR values back into [0,1].
    //
    // `Clip` (Apophysis/JWildfire compatible): per-channel clamp. Any channel
    // exceeding 1.0 saturates independently, which pushes bright colors
    // toward the CMY/white corners of the RGB cube — orange (1, 0.5, 0) ×
    // exposure 5 → (5, 2.5, 0) → clamps to pure yellow.
    //
    // `MaxNorm` (hue-preserving): if any channel exceeds 1.0, divide all by
    // the max so the brightest channel lands at exactly 1.0 and the others
    // stay in ratio. Bright pixels desaturate by lowering value (luminance)
    // rather than shifting hue. Negative channels are clamped separately.
    color = max(color, vec3<f32>(0.0));
    if (tonemap_params.highlight_mode == 1u) {
        // MaxNorm: rescale by max channel so brightest channel = 1.0.
        let m = max(color.r, max(color.g, color.b));
        if (m > 1.0) {
            color = color / m;
        }
    } else if (tonemap_params.highlight_mode == 2u) {
        // Reinhard luminance-preserving: L_mapped = L / (1 + L), scale RGB
        // by L_mapped/L. Rec.709 luminance weights.
        let lum = dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
        if (lum > 1e-6) {
            let lum_mapped = lum / (1.0 + lum);
            color = color * (lum_mapped / lum);
        }
        // Safety clamp — Reinhard maps to (0,1) but float noise could leak.
        color = min(color, vec3<f32>(1.0));
    } else if (tonemap_params.highlight_mode == 3u) {
        // ACES filmic (Narkowicz approximation):
        //   f(x) = (x(ax+b)) / (x(cx+d)+e)
        // Constants tuned for SDR display output. Slight contrast boost in
        // midtones, gentle highlight roll-off, hue-mostly-preserving.
        let a = 2.51;
        let b = 0.03;
        let c = 2.43;
        let d = 0.59;
        let e = 0.14;
        color = (color * (a * color + vec3<f32>(b))) / (color * (c * color + vec3<f32>(d)) + vec3<f32>(e));
        color = clamp(color, vec3<f32>(0.0), vec3<f32>(1.0));
    } else {
        // Clip: per-channel clamp (Apophysis/JWildfire compatible).
        color = min(color, vec3<f32>(1.0));
    }

    // Apply tone curve to fractal color only (not background)
    // Sample curve LUT unconditionally (WebGPU requires textureSample in uniform control flow)
    let curve_r = textureSample(curve_lut_texture, curve_lut_sampler, vec2<f32>(color.r, 0.5)).r;
    let curve_g = textureSample(curve_lut_texture, curve_lut_sampler, vec2<f32>(color.g, 0.5)).r;
    let curve_b = textureSample(curve_lut_texture, curve_lut_sampler, vec2<f32>(color.b, 0.5)).r;

    // Only apply curve where there's significant fractal density
    let should_apply_curve = tonemap_params.use_curve != 0u && bucket_count > 0.001;
    var fractal_color = select(color, vec3<f32>(curve_r, curve_g, curve_b), should_apply_curve);

    // ===== STAGE 3F: Levels and Background Blending =====
    //
    // Apply Levels transformation to density for opacity control
    // Note: Levels affect OPACITY, not color brightness!
    // - levels_low: pixels below this density (× mean) become fully transparent
    // - levels_high: pixels above this density (× mean) become fully opaque
    // - levels_gamma: curve adjustment between the two thresholds
    //
    // Units are multiples of mean density (sample_density), not raw
    // counts. Defaults (low=0, high=1.0, gamma=1) clip at the mean,
    // independent of iteration count.
    //
    // Pass `accum.a` (raw density per pixel), NOT `bucket_count`.
    // `bucket_count = accum.a × 100` is a calibration scale for log/
    // brightness math elsewhere in this shader. `sample_density`
    // is in raw mean-density units (no ×100), so dividing
    // `bucket_count` by `sample_density` would inflate the ratio by
    // 100× — effectively disabling Levels at any reasonable
    // `levels_high` setting.
    let leveled_opacity = apply_levels(accum.a);

    // Original alpha blending strategy (kept for backward compatibility):
    // Blend between gamma-corrected alpha (good edges) and linear (good detail)
    let linear_alpha = clamp(bucket_count * 0.01 * tonemap_params.density_scale, 0.0, 1.0);
    let gamma_alpha = clamp(alpha, 0.0, 1.0);
    let blend_t = smoothstep(tonemap_params.alpha_blend_low, tonemap_params.alpha_blend_high, gamma_alpha);
    let base_alpha = mix(gamma_alpha, linear_alpha, blend_t);

    // Combine levels with the original alpha calculation. When
    // `levels_enabled == 0` (Apo-matching default), bypass the Levels
    // remap entirely — the docstring used to claim the defaults were
    // a no-op, but `levels_high = 10` caps mid-density opacity at 10%
    // via this `min()`. Apo has no Levels system at all; off should
    // mean off, not "no-op-ish."
    let fractal_alpha = select(base_alpha, min(base_alpha, leveled_opacity), tonemap_params.levels_enabled != 0u);

    if (tonemap_params.transparent_mode != 0u) {
        // Transparent export, built so a STANDARD straight-alpha flatten over
        // black — rgb·a, in sRGB space, what image editors do — reconstructs
        // the opaque export exactly.
        //
        // A fractal flame's brightness lives mostly in a LOW density alpha
        // (HDR color × small opacity = the look). Both straight alpha
        // (rgb = color) and premultiplied alpha (rgb = color·a) come out dim
        // when an editor flattens that over black: straight loses the gamma
        // (sRGB(color)·a ≠ sRGB(color·a)), premultiplied multiplies by the tiny
        // alpha twice. So instead we encode the over-black image itself and
        // re-split it: the opaque-over-black sRGB color is C = sRGB(color·a);
        // take the coverage alpha = max channel of C (so the un-multiplied rgb
        // never exceeds 1), and store rgb = C / coverage. Then the editor's
        // flatten rgb·coverage == C == the opaque pixel, exactly. Over other
        // backgrounds it composites as ordinary straight alpha (bright where
        // the flame is bright, transparent where it's dark).
        let over_black_lin = fractal_color * fractal_alpha;
        let over_black_srgb = pow(max(over_black_lin, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.2));

        if (tonemap_params.transparent_mode == 2u) {
            // PREMULTIPLIED alpha (opt-in): store the over-black colour directly
            // as the RGB and the fractal's own opacity as alpha. A premultiplied
            // "over" composite (rgb + bg·(1−a), what AE/Nuke/linear pipelines do
            // when told the PNG is premultiplied) reconstructs the opaque; a
            // plain straight flatten would dim it. The correct representation for
            // additive-glow content in a premultiplied pipeline.
            return vec4<f32>(over_black_srgb, fractal_alpha);
        }

        // Mode 1 (default): straight-alpha reconstruction — split so a standard
        // straight flatten over black (rgb·a) returns the over-black colour. The
        // coverage alpha = max channel of C keeps the un-multiplied rgb ≤ 1.
        let coverage = max(over_black_srgb.r, max(over_black_srgb.g, over_black_srgb.b));
        let straight_rgb = select(vec3<f32>(0.0), over_black_srgb / coverage, coverage > 0.0);
        return vec4<f32>(straight_rgb, coverage);
    }

    // Normal display / opaque export: composite with background, opaque output.
    // Background color is supplied as sRGB (matches what the user picks in the
    // UI); decode to linear before blending in linear space.
    let bg_linear = srgb_to_linear(tonemap_params.background_color);
    let final_color = bg_linear * (1.0 - fractal_alpha) + fractal_color * fractal_alpha;

    // Convert from linear to sRGB for display
    // (Rgba8Unorm is linear, but monitors expect sRGB)
    let srgb_color = pow(final_color, vec3<f32>(1.0 / 2.2));

    return vec4<f32>(srgb_color, 1.0);
}
