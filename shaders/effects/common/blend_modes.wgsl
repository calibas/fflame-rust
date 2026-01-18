// Shared Blend Modes for Color Effects
//
// Standard blend modes (0-12):
//   0 = Normal      - Simple alpha/intensity blend
//   1 = Add         - Additive (Linear Dodge), brightens
//   2 = Multiply    - Darkens, good for shadows
//   3 = Screen      - Lightens, inverse of multiply
//   4 = Overlay     - Combines multiply/screen, increases contrast
//   5 = Soft Light  - Gentler version of overlay
//   6 = Hard Light  - Stronger version of overlay
//   7 = Color Dodge - Brightens base, high contrast highlights
//   8 = Color Burn  - Darkens base, high contrast shadows
//   9 = Hue         - Takes hue from effect, sat/lum from original
//  10 = Saturation  - Takes saturation from effect
//  11 = Color       - Takes hue+saturation from effect, luminosity from original
//  12 = Luminosity  - Takes luminosity from effect, hue+sat from original
//
// Usage:
//   let result = apply_blend(original.rgb, effect_color, blend_mode, intensity);

// ============================================================================
// Color Space Conversion
// ============================================================================

fn rgb_to_hsl(c: vec3<f32>) -> vec3<f32> {
    let cmax = max(c.r, max(c.g, c.b));
    let cmin = min(c.r, min(c.g, c.b));
    let delta = cmax - cmin;
    let l = (cmax + cmin) * 0.5;

    if (delta < 0.00001) {
        return vec3<f32>(0.0, 0.0, l);
    }

    let s = select(
        delta / (cmax + cmin),
        delta / (2.0 - cmax - cmin),
        l > 0.5
    );

    var h: f32;
    if (cmax == c.r) {
        h = (c.g - c.b) / delta;
        if (c.g < c.b) { h += 6.0; }
    } else if (cmax == c.g) {
        h = (c.b - c.r) / delta + 2.0;
    } else {
        h = (c.r - c.g) / delta + 4.0;
    }
    h /= 6.0;

    return vec3<f32>(h, s, l);
}

fn hsl_to_rgb(hsl: vec3<f32>) -> vec3<f32> {
    let h = hsl.x;
    let s = hsl.y;
    let l = hsl.z;

    if (s < 0.00001) {
        return vec3<f32>(l, l, l);
    }

    let q = select(l * (1.0 + s), l + s - l * s, l < 0.5);
    let p = 2.0 * l - q;

    var rgb: vec3<f32>;
    rgb.r = hue_to_rgb(p, q, h + 1.0/3.0);
    rgb.g = hue_to_rgb(p, q, h);
    rgb.b = hue_to_rgb(p, q, h - 1.0/3.0);

    return rgb;
}

fn hue_to_rgb(p: f32, q: f32, t_in: f32) -> f32 {
    var t = t_in;
    if (t < 0.0) { t += 1.0; }
    if (t > 1.0) { t -= 1.0; }

    if (t < 1.0/6.0) { return p + (q - p) * 6.0 * t; }
    if (t < 1.0/2.0) { return q; }
    if (t < 2.0/3.0) { return p + (q - p) * (2.0/3.0 - t) * 6.0; }
    return p;
}

fn luminance(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

fn set_luminance(c: vec3<f32>, l: f32) -> vec3<f32> {
    let d = l - luminance(c);
    return clamp(c + d, vec3<f32>(0.0), vec3<f32>(1.0));
}

fn saturation(c: vec3<f32>) -> f32 {
    return max(c.r, max(c.g, c.b)) - min(c.r, min(c.g, c.b));
}

// ============================================================================
// Basic Blend Modes
// ============================================================================

fn blend_normal(base: vec3<f32>, blend: vec3<f32>) -> vec3<f32> {
    return blend;
}

fn blend_add(base: vec3<f32>, blend: vec3<f32>) -> vec3<f32> {
    return min(base + blend, vec3<f32>(1.0));
}

fn blend_multiply(base: vec3<f32>, blend: vec3<f32>) -> vec3<f32> {
    return base * blend;
}

fn blend_screen(base: vec3<f32>, blend: vec3<f32>) -> vec3<f32> {
    return 1.0 - (1.0 - base) * (1.0 - blend);
}

fn blend_overlay(base: vec3<f32>, blend: vec3<f32>) -> vec3<f32> {
    return select(
        2.0 * base * blend,
        1.0 - 2.0 * (1.0 - base) * (1.0 - blend),
        base > vec3<f32>(0.5)
    );
}

fn blend_soft_light(base: vec3<f32>, blend: vec3<f32>) -> vec3<f32> {
    return select(
        base - (1.0 - 2.0 * blend) * base * (1.0 - base),
        base + (2.0 * blend - 1.0) * (select(
            ((16.0 * base - 12.0) * base + 4.0) * base,
            sqrt(base),
            base > vec3<f32>(0.25)
        ) - base),
        blend > vec3<f32>(0.5)
    );
}

fn blend_hard_light(base: vec3<f32>, blend: vec3<f32>) -> vec3<f32> {
    return select(
        2.0 * base * blend,
        1.0 - 2.0 * (1.0 - base) * (1.0 - blend),
        blend > vec3<f32>(0.5)
    );
}

fn blend_color_dodge(base: vec3<f32>, blend: vec3<f32>) -> vec3<f32> {
    return select(
        min(vec3<f32>(1.0), base / (1.0 - blend + 0.00001)),
        vec3<f32>(1.0),
        blend >= vec3<f32>(1.0)
    );
}

fn blend_color_burn(base: vec3<f32>, blend: vec3<f32>) -> vec3<f32> {
    return select(
        1.0 - min(vec3<f32>(1.0), (1.0 - base) / (blend + 0.00001)),
        vec3<f32>(0.0),
        blend <= vec3<f32>(0.0)
    );
}

// ============================================================================
// HSL-Based Blend Modes
// ============================================================================

fn blend_hue(base: vec3<f32>, blend: vec3<f32>) -> vec3<f32> {
    let base_hsl = rgb_to_hsl(base);
    let blend_hsl = rgb_to_hsl(blend);
    return hsl_to_rgb(vec3<f32>(blend_hsl.x, base_hsl.y, base_hsl.z));
}

fn blend_saturation(base: vec3<f32>, blend: vec3<f32>) -> vec3<f32> {
    let base_hsl = rgb_to_hsl(base);
    let blend_hsl = rgb_to_hsl(blend);
    return hsl_to_rgb(vec3<f32>(base_hsl.x, blend_hsl.y, base_hsl.z));
}

fn blend_color(base: vec3<f32>, blend: vec3<f32>) -> vec3<f32> {
    let base_hsl = rgb_to_hsl(base);
    let blend_hsl = rgb_to_hsl(blend);
    return hsl_to_rgb(vec3<f32>(blend_hsl.x, blend_hsl.y, base_hsl.z));
}

fn blend_luminosity(base: vec3<f32>, blend: vec3<f32>) -> vec3<f32> {
    let base_hsl = rgb_to_hsl(base);
    let blend_hsl = rgb_to_hsl(blend);
    return hsl_to_rgb(vec3<f32>(base_hsl.x, base_hsl.y, blend_hsl.z));
}

// ============================================================================
// Unified Blend Function
// ============================================================================

fn apply_blend(base: vec3<f32>, effect: vec3<f32>, mode: i32, intensity: f32) -> vec3<f32> {
    var blended: vec3<f32>;

    switch (mode) {
        case 0: { blended = blend_normal(base, effect); }
        case 1: { blended = blend_add(base, effect); }
        case 2: { blended = blend_multiply(base, effect); }
        case 3: { blended = blend_screen(base, effect); }
        case 4: { blended = blend_overlay(base, effect); }
        case 5: { blended = blend_soft_light(base, effect); }
        case 6: { blended = blend_hard_light(base, effect); }
        case 7: { blended = blend_color_dodge(base, effect); }
        case 8: { blended = blend_color_burn(base, effect); }
        case 9: { blended = blend_hue(base, effect); }
        case 10: { blended = blend_saturation(base, effect); }
        case 11: { blended = blend_color(base, effect); }
        case 12: { blended = blend_luminosity(base, effect); }
        default: { blended = blend_normal(base, effect); }
    }

    return mix(base, blended, intensity);
}
