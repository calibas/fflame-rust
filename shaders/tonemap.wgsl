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
    path_map_style: u32,  // 0=Prefix, 1=Suffix, 2=PrefixDistinct, 3=SuffixDistinct, 4=Depth, 5=OriginRadial, 6=OriginHorizontal, 7=OriginVertical
    burn_in: u32,  // Burn-in iterations (for Depth gradient: start depth)
    num_transforms: u32,  // Number of transforms (for path coloring entropy)
    _pad0: u32,  // Padding to 16-byte boundary
    _pad1: u32,
    _pad2: u32,
    _pad3: u32,
}

// Path storage entry (matches compute shader PathEntry)
// Stores first 32 iterations losslessly (4 bits per transform, up to 16 transforms)
// Also stores initial random X/Y coordinates for gradient-based coloring
struct PathEntry {
    path0: u32,  // Iterations 0-7 (4 bits each, LSB = iteration 0)
    path1: u32,  // Iterations 8-15
    path2: u32,  // Iterations 16-23
    path3: u32,  // Iterations 24-31
    iteration_count: u32,  // Number of valid iterations stored (0-32)
    initial_x: f32,  // Initial random X coordinate [-1, 1]
    initial_y: f32,  // Initial random Y coordinate [-1, 1]
}

@group(0) @binding(0) var accumulation_texture: texture_2d<f32>;
@group(0) @binding(1) var accumulation_sampler: sampler;
@group(0) @binding(2) var<uniform> tonemap_params: TonemapParams;
@group(0) @binding(3) var curve_lut_texture: texture_2d<f32>;
@group(0) @binding(4) var curve_lut_sampler: sampler;
@group(0) @binding(5) var<storage, read> path_buffer: array<PathEntry>;
@group(0) @binding(6) var palette_texture: texture_2d<f32>;
@group(0) @binding(7) var palette_sampler: sampler;

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

// Hash function for scrambling - spreads similar values across color space
fn scramble_hash(x: u32) -> u32 {
    var h = x;
    h = h ^ (h >> 16u);
    h = h * 0x85ebca6bu;
    h = h ^ (h >> 13u);
    h = h * 0xc2b2ae35u;
    h = h ^ (h >> 16u);
    return h;
}

// Convert hue to RGB (full saturation and value for vibrant colors)
fn hue_to_rgb(hue: f32) -> vec3<f32> {
    let h = hue * 6.0;
    let i = floor(h);
    let f = h - i;
    let q = 1.0 - f;

    var r: f32;
    var g: f32;
    var b: f32;

    let sector = i32(i) % 6;
    if (sector == 0) {
        r = 1.0; g = f; b = 0.0;
    } else if (sector == 1) {
        r = q; g = 1.0; b = 0.0;
    } else if (sector == 2) {
        r = 0.0; g = 1.0; b = f;
    } else if (sector == 3) {
        r = 0.0; g = q; b = 1.0;
    } else if (sector == 4) {
        r = f; g = 0.0; b = 1.0;
    } else {
        r = 1.0; g = 0.0; b = q;
    }

    return vec3<f32>(r, g, b);
}

// Extract a single transform index from path data
// Each transform is stored in 4 bits: path0 has iterations 0-7, path1 has 8-15, etc.
fn get_transform_at(path: PathEntry, iteration: u32) -> u32 {
    let word_idx = iteration / 8u;
    let bit_offset = (iteration % 8u) * 4u;

    var word: u32;
    if (word_idx == 0u) {
        word = path.path0;
    } else if (word_idx == 1u) {
        word = path.path1;
    } else if (word_idx == 2u) {
        word = path.path2;
    } else {
        word = path.path3;
    }

    return (word >> bit_offset) & 0xFu;
}

// Get prefix data: first 8 iterations (path0 only)
fn get_prefix(path: PathEntry) -> u32 {
    return path.path0;
}

// Get suffix data: last 8 valid iterations based on iteration_count
fn get_suffix(path: PathEntry) -> u32 {
    let count = path.iteration_count;

    // If we have 8 or fewer iterations, use path0 (all we have)
    if (count <= 8u) {
        return path.path0;
    }

    // Find which word contains the end of our valid data
    // count=9-16 -> use path1, count=17-24 -> use path2, count=25-32 -> use path3
    if (count <= 16u) {
        return path.path1;
    } else if (count <= 24u) {
        return path.path2;
    } else {
        return path.path3;
    }
}

// Path coloring for style 0 (Prefix) and style 1 (Suffix)
// Similar paths produce similar colors - smooth hue gradient based on path value
fn path_to_color_smooth(value: u32, num_transforms: u32) -> vec3<f32> {
    // Calculate the effective bit width based on num_transforms
    // With N transforms, each 4-bit slot can only have values 0 to N-1
    // We want to normalize so full range of possible paths maps to full hue range

    // For 8 iterations × 4 bits = 32 bits total
    // If num_transforms <= 16, all values fit in 4 bits per slot
    // Max possible value depends on num_transforms

    // Calculate max possible value for this number of transforms
    // Each of 8 slots can have values 0 to (num_transforms-1)
    // Treated as a base-N number: max = N^8 - 1
    // But we have it packed as 4-bit slots, so we need to interpret differently

    // Simpler approach: treat the 32-bit value as a direct hue mapping
    // Use golden ratio for good distribution without full scrambling
    let golden_ratio = 0.618033988749895;

    // For smooth coloring, we want similar values to produce similar hues
    // Just normalize the value to 0-1 range and use as hue
    // This gives gradual color transitions for similar paths
    let hue = fract(f32(value) * golden_ratio / f32(0xFFFFFFFFu));

    return hue_to_rgb(hue);
}

// Path coloring for style 3 (SuffixDistinct)
// Maximum color separation - similar paths get very different colors
fn path_to_color_distinct(value: u32) -> vec3<f32> {
    let golden_ratio = 0.618033988749895;

    // Apply scramble hash for maximum color separation
    let scrambled = scramble_hash(value);
    let hue = fract(f32(scrambled) * golden_ratio / f32(0xFFFFFFFFu));

    return hue_to_rgb(hue);
}

// Path coloring for style 2 (PrefixDistinct)
// Incorporates iteration_count to distinguish paths of different lengths
// e.g., [0] vs [0,0] vs [0,0,0] all have path0=0 but different iteration counts
fn path_to_color_prefix_distinct(value: u32, iteration_count: u32) -> vec3<f32> {
    let golden_ratio = 0.618033988749895;

    // Mix iteration_count into the value before hashing
    // This ensures paths with same prefix but different lengths get different colors
    let mixed = value ^ (iteration_count * 0x9E3779B9u);
    let scrambled = scramble_hash(mixed);
    let hue = fract(f32(scrambled) * golden_ratio / f32(0xFFFFFFFFu));

    return hue_to_rgb(hue);
}

// Main path-to-color function that handles all 4 hash-based styles
// style 0 = Prefix (smooth), 1 = Suffix (smooth), 2 = PrefixDistinct, 3 = SuffixDistinct
fn path_to_color(path: PathEntry, style: u32, num_transforms: u32) -> vec3<f32> {
    // Get the relevant path data based on prefix/suffix
    var value: u32;
    if (style == 0u || style == 2u) {
        // Prefix styles: use first 8 iterations
        value = get_prefix(path);
    } else {
        // Suffix styles: use last 8 valid iterations
        value = get_suffix(path);
    }

    // Apply smooth or distinct coloring
    if (style <= 1u) {
        // Smooth: similar paths → similar colors
        return path_to_color_smooth(value, num_transforms);
    } else if (style == 2u) {
        // Prefix Distinct: include iteration_count to distinguish same-prefix paths
        return path_to_color_prefix_distinct(value, path.iteration_count);
    } else {
        // Suffix Distinct: scramble for maximum color separation
        return path_to_color_distinct(value);
    }
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

    // ===== PathMap Mode: Override Color from Path Buffer =====
    // In PathMap mode, the accumulation buffer stores white (density only)
    // The actual color is derived from the path stored in path_buffer
    //
    // Styles 0-3: Hash-based coloring (Prefix, Suffix, PrefixDistinct, SuffixDistinct)
    // Styles 4-7: Gradient-based coloring using palette (Depth, OriginRadial, OriginHorizontal, OriginVertical)
    if (tonemap_params.color_mode == 2u) {
        // Calculate pixel coordinates from UV
        let pixel_x = u32(input.uv.x * f32(tonemap_params.width));
        let pixel_y = u32(input.uv.y * f32(tonemap_params.height));
        let pixel_idx = pixel_y * tonemap_params.width + pixel_x;

        // Read path from buffer
        let path = path_buffer[pixel_idx];

        // Only color pixels with actual path data (iteration_count > 0)
        if (path.iteration_count > 0u) {
            let style = tonemap_params.path_map_style;

            if (style <= 3u) {
                // Hash-based coloring: Prefix, Suffix, PrefixDistinct, SuffixDistinct
                fractal_color = path_to_color(path, style, tonemap_params.num_transforms);
            } else {
                // Gradient-based coloring using palette
                var t: f32 = 0.0;

                if (style == 4u) {
                    // Depth: Color by iteration count
                    // Map from burn_in to 32 onto 0.0 to 1.0
                    let min_depth = f32(tonemap_params.burn_in);
                    let max_depth = 32.0;
                    let depth = f32(path.iteration_count);
                    t = clamp((depth - min_depth) / (max_depth - min_depth), 0.0, 1.0);
                } else if (style == 5u) {
                    // OriginRadial: Color by distance from origin
                    // Map from 0 to sqrt(2) ≈ 1.4142 onto 0.0 to 1.0
                    let dist = sqrt(path.initial_x * path.initial_x + path.initial_y * path.initial_y);
                    t = clamp(dist / 1.4142135, 0.0, 1.0);
                } else if (style == 6u) {
                    // OriginHorizontal: Color by X position
                    // Map from -1 to 1 onto 0.0 to 1.0
                    t = clamp((path.initial_x + 1.0) * 0.5, 0.0, 1.0);
                } else {
                    // OriginVertical (style == 7u): Color by Y position
                    // Map from -1 to 1 onto 0.0 to 1.0
                    t = clamp((path.initial_y + 1.0) * 0.5, 0.0, 1.0);
                }

                // Load palette texture at position t
                // Palette is 256x1 texture - use textureLoad to avoid uniform control flow requirement
                let palette_idx = u32(clamp(t * 255.0, 0.0, 255.0));
                fractal_color = textureLoad(palette_texture, vec2<i32>(i32(palette_idx), 0), 0).rgb;
            }
        }
    }

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

    // Transparent mode: output fractal color with alpha for PNG export
    // Normal mode: composite with background color for display
    var final_color: vec3<f32>;
    var final_alpha: f32;

    if (tonemap_params.transparent_mode != 0u) {
        // Transparent export: output fractal color and alpha directly
        // No background blending - the alpha channel represents transparency
        final_color = fractal_color;
        final_alpha = fractal_alpha;
    } else {
        // Normal display: composite with background, opaque output
        final_color = tonemap_params.background_color * (1.0 - fractal_alpha) + fractal_color * fractal_alpha;
        final_alpha = 1.0;
    }

    // Convert from linear to sRGB for display
    // (Rgba8Unorm is linear, but monitors expect sRGB)
    let srgb_color = pow(final_color, vec3<f32>(1.0 / 2.2));

    return vec4<f32>(srgb_color, final_alpha);
}
