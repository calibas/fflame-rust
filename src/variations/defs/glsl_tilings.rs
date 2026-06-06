//! `glsl_*` family — kaleidoscope + tiling subset (6 variations).
//!
//! Shadertoy-style 2D tilings hand-ported by Jesus Sosa to JWildfire
//! (Oct 2018). Same call shape as the [`glsl_fractals`] sibling — see
//! that module's doc for the shared semantics (`NeedsRng + WritesRgb`
//! features, uniform `[-0.5, 0.5]²` spatial output, per-pixel RGB
//! written to `*vrc`, `gradient`-param mode 0 honored only,
//! `seed`-param CPU-only).
//!
//! Each tiling is independent (no shared helpers across variations)
//! so the WGSL functions are prefixed with the variation name to
//! avoid collisions: `kaleidoscopic_kscope`, `mandala_kscope`,
//! `hoshi_hsv`, etc.
//!
//! Sources:
//! - [`GLSLKaleidoscopicFunc.java`](../../../output/variation-jwf-source/GLSLKaleidoscopicFunc.java)
//! - [`GLSLKaleidoComplexFunc.java`](../../../output/variation-jwf-source/GLSLKaleidoComplexFunc.java)
//! - [`GLSLHyperbolicFunc.java`](../../../output/variation-jwf-source/GLSLHyperbolicFunc.java)
//! - [`GLSLMandalaFunc.java`](../../../output/variation-jwf-source/GLSLMandalaFunc.java)
//! - [`GLSLSquaresFunc.java`](../../../output/variation-jwf-source/GLSLSquaresFunc.java)
//! - [`GLSLHoshiFunc.java`](../../../output/variation-jwf-source/GLSLHoshiFunc.java)
//!
//! # Authors
//! - Jesus Sosa (ports, 2018)
//! - Robert Schütze / trirop (original `glsl_kaleidoscopic` shadertoy
//!   colors, http://glslsandbox.com/e#29611)

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// glsl_kaleidoscopic
// =============================================================================

/// Kaleidoscope fold followed by 44 iterations of the trirop Apollonian
/// fold (`abs(p)/dot(p,p)` with swizzle/scale). Produces colorful
/// shadertoy-style kaleidoscope patterns with a time-driven rotation.
///
/// 7 params: standard 4 + `sides` (kaleidoscope sector count), `zoom`,
/// `p1` (color phase parameter), `radial` (kaleidoscope mode flag).
/// 
/// # Authors
/// - Jesus Sosa
pub static GLSL_KALEIDOSCOPIC: VariationDef = VariationDef {
    name: "glsl_kaleidoscopic",
    aliases: &[],
    display_name: "GLSL Kaleidoscopic",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::WritesRgb],
    parameters: &[
        param!("density_pixels", "Density Pixels", unlimited_int, 1000000.0, 100.0, 10000000.0, "Discrete grid resolution. See module-level doc."),
        param!("seed", "Seed", unlimited_int, 10000.0, 0.0, 10000.0, "JWildfire CPU-only — set `time` directly."),
        param!("time", "Time", unlimited_float, 0.0, -10000.0, 10000.0, "Animation parameter. Drives the kaleidoscope's rotation (angle += 0.1·time)."),
        param!("sides", "Sides", unlimited_int, 8.0, 2.0, 32.0, "Number of kaleidoscope sectors. The kaleidoscope angle is `π / sides`; higher = more rotational symmetry."),
        param!("zoom", "Zoom", unlimited_float, 0.0, 0.0, 1.0, "Pre-fold zoom scalar. Multiplies the input by `0.1 + zoom`, so larger values zoom in. Also fed into the per-fold iteration as the initial Z (depth)."),
        param!("p1", "P1", unlimited_float, 0.0, -2.0, 2.0, "Color phase parameter. Modulates the Apollonian fold's offset vector along the green channel (`p1 · 0.4`). Small changes here shift the visible color regions."),
        param!("radial", "Radial", unlimited_int, 0.0, 0.0, 1.0, "Kaleidoscope mode. 0 = pass uv through unchanged. 1 = apply the kaleidoscope fold (sectors + reflection + rotation)."),
        param!("gradient", "Gradient", unlimited_int, 0.0, 0.0, 1.0, "JWildfire color output mode. **Only mode 0 (direct RGB) honored.**"),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: KALEIDOSCOPIC_2D,
    wgsl_3d: KALEIDOSCOPIC_3D,
};

// Kaleidoscopic per-call: 44 iter × ~10 ops = ~440 ops. Cheap.
const KALEIDOSCOPIC_2D: &str = r#"
fn kaleidoscopic_kscope(uv: vec2<f32>, ka: f32, time: f32) -> vec2<f32> {
    var a = atan2(uv.y, uv.x);
    let two_ka = 2.0 * ka;
    a = a - two_ka * floor(a / two_ka);  // mod into [0, 2·ka]
    a = abs(a - ka) + 0.1 * time;          // reflect + time-rotation
    let r = length(uv);
    return vec2<f32>(cos(a), sin(a)) * r;
}

fn variation_glsl_kaleidoscopic(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vrc: ptr<function, vec3<f32>>) -> vec2<f32> {
    let resolution = max(get_param(xform_id, variation_id, 0u), 100.0);
    let time = get_param(xform_id, variation_id, 2u);
    let sides = max(get_param(xform_id, variation_id, 3u), 1.0);
    let zoom = get_param(xform_id, variation_id, 4u);
    let p1 = get_param(xform_id, variation_id, 5u);
    let radial = u32(get_param(xform_id, variation_id, 6u));

    let i_f = floor(rng_nextf(rng) * resolution);
    let j_f = floor(rng_nextf(rng) * resolution);
    let x = i_f + 0.5;
    let y = j_f + 0.5;

    // Map to [-12, 12]², apply zoom, optionally kaleidoscope-fold.
    var uv = vec2<f32>(2.0 * x / resolution - 1.0, 2.0 * y / resolution - 1.0) * 12.0;
    uv = uv * (0.1 + zoom);
    if (radial == 1u) {
        let ka = 3.14159265 / sides;
        uv = kaleidoscopic_kscope(uv, ka, time);
    }

    // trirop Apollonian-style fold (44 iters). The swizzle p.xzy at the
    // end of each iteration is what gives the kaleidoscopic structure.
    var pp = vec3<f32>(uv.x, uv.y, zoom);
    for (var i: u32 = 0u; i < 44u; i = i + 1u) {
        let d = max(dot(pp, pp), 1.0e-32);
        let t1 = abs(pp) / d;
        let t0 = t1 - vec3<f32>(1.0, 1.02, p1 * 0.4);
        let t2 = vec3<f32>(1.3, 0.999, 0.678) * abs(t0);
        pp = vec3<f32>(t2.x, t2.z, t2.y);  // swizzle
    }
    *vrc = pp;

    return vec2<f32>(i_f / resolution - 0.5, j_f / resolution - 0.5);
}
"#;

const KALEIDOSCOPIC_3D: &str = r#"
fn kaleidoscopic_kscope(uv: vec2<f32>, ka: f32, time: f32) -> vec2<f32> {
    var a = atan2(uv.y, uv.x);
    let two_ka = 2.0 * ka;
    a = a - two_ka * floor(a / two_ka);  // mod into [0, 2·ka]
    a = abs(a - ka) + 0.1 * time;          // reflect + time-rotation
    let r = length(uv);
    return vec2<f32>(cos(a), sin(a)) * r;
}

fn variation_glsl_kaleidoscopic(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vrc: ptr<function, vec3<f32>>) -> vec3<f32> {
    let resolution = max(get_param(xform_id, variation_id, 0u), 100.0);
    let time = get_param(xform_id, variation_id, 2u);
    let sides = max(get_param(xform_id, variation_id, 3u), 1.0);
    let zoom = get_param(xform_id, variation_id, 4u);
    let p1 = get_param(xform_id, variation_id, 5u);
    let radial = u32(get_param(xform_id, variation_id, 6u));

    let i_f = floor(rng_nextf(rng) * resolution);
    let j_f = floor(rng_nextf(rng) * resolution);
    var uv = vec2<f32>(2.0 * (i_f + 0.5) / resolution - 1.0, 2.0 * (j_f + 0.5) / resolution - 1.0) * 12.0;
    uv = uv * (0.1 + zoom);
    if (radial == 1u) {
        let ka = 3.14159265 / sides;
        uv = kaleidoscopic_kscope(uv, ka, time);
    }

    var pp = vec3<f32>(uv.x, uv.y, zoom);
    for (var i: u32 = 0u; i < 44u; i = i + 1u) {
        let d = max(dot(pp, pp), 1.0e-32);
        let t1 = abs(pp) / d;
        let t0 = t1 - vec3<f32>(1.0, 1.02, p1 * 0.4);
        let t2 = vec3<f32>(1.3, 0.999, 0.678) * abs(t0);
        pp = vec3<f32>(t2.x, t2.z, t2.y);
    }
    *vrc = pp;
    return vec3<f32>(i_f / resolution - 0.5, j_f / resolution - 0.5, p.z);
}
"#;

// =============================================================================
// glsl_kaleidocomplex
// =============================================================================

/// Complex-valued kaleidoscope iteration. Iterates a `vec4` state
/// (treating `(x, y)` and `(z, w)` as two paired complex numbers)
/// through abs/shift/complex-multiply/cross-product steps,
/// accumulating per-channel color via `exp(-|...|)` terms. Has a
/// per-channel "color frequency" stage at the end (`cos(col · F*)`).
///
/// 9 params: standard 4 + `i_max` (iter count), `color` (intensity),
/// `fr`/`fg`/`fb` (per-channel frequencies).
/// 
/// # Authors
/// - Jesus Sosa
pub static GLSL_KALEIDOCOMPLEX: VariationDef = VariationDef {
    name: "glsl_kaleidocomplex",
    aliases: &[],
    display_name: "GLSL KaleidoComplex",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::WritesRgb],
    parameters: &[
        param!("density_pixels", "Density Pixels", unlimited_int, 1000000.0, 100.0, 10000000.0, "Discrete grid resolution."),
        param!("seed", "Seed", unlimited_int, 10000.0, 0.0, 10000.0, "JWildfire CPU-only — set `time` directly."),
        param!("time", "Time", unlimited_float, 0.0, -10000.0, 10000.0, "Animation parameter. Embedded into the complex multiply as `cmult(z, vec2(sin(time), cos(time)))`."),
        param!("i_max", "iMax", unlimited_int, 12.0, 1.0, 50.0, "Inner iteration cap. **GPU-clamped to ≤ 50.** Inner loop also breaks early when |z|² > 1e7 or the (z, w) pair magnitudes diverge."),
        param!("color", "Color", unlimited_float, 2.0, -10.0, 10.0, "Color intensity scalar. Multiplies the per-iteration `exp(-|...|)` accumulators (the variable name is `fc` in the JWildfire source — kept the JWF UI label 'Color')."),
        param!("fr", "Red Fac.", unlimited_float, 1.0, -10.0, 10.0, "Red-channel frequency: final red = `cos(col.r · fr)`."),
        param!("fg", "Green Fac.", unlimited_float, 1.0, -10.0, 10.0, "Green-channel frequency."),
        param!("fb", "Blue Fac.", unlimited_float, 1.0, -10.0, 10.0, "Blue-channel frequency."),
        param!("gradient", "Gradient", unlimited_int, 0.0, 0.0, 1.0, "JWildfire color output mode. **Only mode 0 honored.**"),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: KALEIDOCOMPLEX_2D,
    wgsl_3d: KALEIDOCOMPLEX_3D,
};

// Per call: i_max (≤ 50) × ~30 ops = ~1500 worst-case. Within TDR.
const KALEIDOCOMPLEX_2D: &str = r#"
fn kaleidocomplex_cmult(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(a.x * b.x - a.y * b.y, a.x * b.y + a.y * b.x);
}

fn variation_glsl_kaleidocomplex(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vrc: ptr<function, vec3<f32>>) -> vec2<f32> {
    let resolution = max(get_param(xform_id, variation_id, 0u), 100.0);
    let time = get_param(xform_id, variation_id, 2u);
    let i_max = min(i32(get_param(xform_id, variation_id, 3u)), 50);
    let fc = get_param(xform_id, variation_id, 4u);
    let fr = get_param(xform_id, variation_id, 5u);
    let fg = get_param(xform_id, variation_id, 6u);
    let fb = get_param(xform_id, variation_id, 7u);

    let i_f = floor(rng_nextf(rng) * resolution);
    let j_f = floor(rng_nextf(rng) * resolution);
    let uv = vec2<f32>((i_f + 0.5) / resolution - 0.5, (j_f + 0.5) / resolution - 0.5);

    // z = (x, y, z, w) packed (x, y) and (z, w) as two complex.
    // of = absolute(uv) / 0.125 — sets initial z.xy.
    var z = vec4<f32>(abs(uv.x) / 0.125, abs(uv.y) / 0.125, 0.0, 0.0);
    var col = vec3<f32>(0.0);
    var dist = vec2<f32>(0.0);
    var ii: i32 = -1;
    var r: f32 = 1.0;

    let st = vec2<f32>(sin(time), cos(time));
    let one_one = vec2<f32>(1.0, 1.0);

    // Loop variable i starts at -1 in the JWF source (so it ranges -1..i_max-1).
    // We use a >= -1 < i_max loop; ii increments to 0..i_max-1 inside.
    for (var i: i32 = -1; i < i_max; i = i + 1) {
        r = r * -1.0;
        ii = ii + 1;
        // z.xy = cmult(z.xy, (1, 1))  — equivalent to a fixed complex multiply.
        var t1 = kaleidocomplex_cmult(vec2<f32>(z.x, z.y), one_one);
        z.x = t1.x; z.y = t1.y;
        // z.xy = abs(z.xy) + (r - 10.5)
        t1 = abs(vec2<f32>(z.x, z.y)) + (r - 10.5);
        z.x = t1.x; z.y = t1.y;
        // z.xy = cmult(z.xy, (sin(time), cos(time)))
        t1 = kaleidocomplex_cmult(vec2<f32>(z.x, z.y), st);
        z.x = t1.x; z.y = t1.y;
        // z.zw cross-product style update.
        let zz_new = fc * (z.x * z.z - z.y * z.w);
        let zw_new = fc * (z.y * z.z - z.x * z.w);
        z.z = zz_new; z.w = zw_new;
        dist = vec2<f32>(
            z.x * z.x + z.y * z.y,
            z.z * z.z + z.w * z.w,
        );
        if (ii > 0 && (abs(z.x) < 0.51 || abs(z.y) < 0.51)) {
            col.x = exp(-abs(z.x * z.x * z.y));
            col.y = exp(-abs(z.y * z.x * z.y));
            col.z = exp(-abs(min(z.x, z.y)));
            break;
        }
        let ii_f = f32(ii);
        let safe_x = sign(z.x) * max(abs(z.x), 1.0e-32);
        let safe_y = sign(z.y) * max(abs(z.y), 1.0e-32);
        let xy_sum = z.x + z.y;
        let safe_xy = sign(xy_sum) * max(abs(xy_sum), 1.0e-32);
        col.x = col.x + fc * exp(-abs(-z.x / max(ii_f, 1.0e-32) + ii_f / safe_x));
        col.y = col.y + fc * exp(-abs(-z.y / max(ii_f, 1.0e-32) + ii_f / safe_y));
        col.z = col.z + fc * exp(-abs(-xy_sum / max(ii_f, 1.0e-32) + ii_f / safe_xy));
        if (dist.x > 1.0e7 || dist.y > 1.0e11) {
            break;
        }
    }
    *vrc = vec3<f32>(cos(col.x * fr), cos(col.y * fg), cos(col.z * fb));
    return vec2<f32>(i_f / resolution - 0.5, j_f / resolution - 0.5);
}
"#;

const KALEIDOCOMPLEX_3D: &str = r#"
fn kaleidocomplex_cmult(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(a.x * b.x - a.y * b.y, a.x * b.y + a.y * b.x);
}

fn variation_glsl_kaleidocomplex(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vrc: ptr<function, vec3<f32>>) -> vec3<f32> {
    let resolution = max(get_param(xform_id, variation_id, 0u), 100.0);
    let time = get_param(xform_id, variation_id, 2u);
    let i_max = min(i32(get_param(xform_id, variation_id, 3u)), 50);
    let fc = get_param(xform_id, variation_id, 4u);
    let fr = get_param(xform_id, variation_id, 5u);
    let fg = get_param(xform_id, variation_id, 6u);
    let fb = get_param(xform_id, variation_id, 7u);

    let i_f = floor(rng_nextf(rng) * resolution);
    let j_f = floor(rng_nextf(rng) * resolution);
    let uv = vec2<f32>((i_f + 0.5) / resolution - 0.5, (j_f + 0.5) / resolution - 0.5);

    var z = vec4<f32>(abs(uv.x) / 0.125, abs(uv.y) / 0.125, 0.0, 0.0);
    var col = vec3<f32>(0.0);
    var dist = vec2<f32>(0.0);
    var ii: i32 = -1;
    var r: f32 = 1.0;
    let st = vec2<f32>(sin(time), cos(time));
    let one_one = vec2<f32>(1.0, 1.0);

    for (var i: i32 = -1; i < i_max; i = i + 1) {
        r = r * -1.0;
        ii = ii + 1;
        var t1 = kaleidocomplex_cmult(vec2<f32>(z.x, z.y), one_one);
        z.x = t1.x; z.y = t1.y;
        t1 = abs(vec2<f32>(z.x, z.y)) + (r - 10.5);
        z.x = t1.x; z.y = t1.y;
        t1 = kaleidocomplex_cmult(vec2<f32>(z.x, z.y), st);
        z.x = t1.x; z.y = t1.y;
        let zz_new = fc * (z.x * z.z - z.y * z.w);
        let zw_new = fc * (z.y * z.z - z.x * z.w);
        z.z = zz_new; z.w = zw_new;
        dist = vec2<f32>(z.x * z.x + z.y * z.y, z.z * z.z + z.w * z.w);
        if (ii > 0 && (abs(z.x) < 0.51 || abs(z.y) < 0.51)) {
            col.x = exp(-abs(z.x * z.x * z.y));
            col.y = exp(-abs(z.y * z.x * z.y));
            col.z = exp(-abs(min(z.x, z.y)));
            break;
        }
        let ii_f = f32(ii);
        let safe_x = sign(z.x) * max(abs(z.x), 1.0e-32);
        let safe_y = sign(z.y) * max(abs(z.y), 1.0e-32);
        let xy_sum = z.x + z.y;
        let safe_xy = sign(xy_sum) * max(abs(xy_sum), 1.0e-32);
        col.x = col.x + fc * exp(-abs(-z.x / max(ii_f, 1.0e-32) + ii_f / safe_x));
        col.y = col.y + fc * exp(-abs(-z.y / max(ii_f, 1.0e-32) + ii_f / safe_y));
        col.z = col.z + fc * exp(-abs(-xy_sum / max(ii_f, 1.0e-32) + ii_f / safe_xy));
        if (dist.x > 1.0e7 || dist.y > 1.0e11) { break; }
    }
    *vrc = vec3<f32>(cos(col.x * fr), cos(col.y * fg), cos(col.z * fb));
    return vec3<f32>(i_f / resolution - 0.5, j_f / resolution - 0.5, p.z);
}
"#;

// =============================================================================
// glsl_hyperbolictile
// =============================================================================

/// Hyperbolic tiling via Möbius reflections in the Poincaré disc.
/// Reflects the sample point across three hyperbolic lines (one
/// straight, two circular arcs at `π/8` and a `√(√2 + 1)` radius
/// circle perpendicular to the unit disc), counting reflections to
/// produce a black/white tile pattern.
///
/// 2 params: `density_pixels`, `gradient`. Output is grayscale (the
/// JWildfire source returns `vec3(color, color, color)`).
///
/// **Note on control flow**: JWildfire's source uses nested loops
/// with `continue` and early-termination flags. We mirror the
/// structure exactly so the output matches.
/// 
/// # Authors
/// - Jesus Sosa
pub static GLSL_HYPERBOLICTILE: VariationDef = VariationDef {
    name: "glsl_hyperbolictile",
    aliases: &[],
    display_name: "GLSL Hyperbolic Tile",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::WritesRgb],
    parameters: &[
        param!("density_pixels", "Density Pixels", unlimited_int, 1000000.0, 100.0, 10000000.0, "Discrete grid resolution."),
        param!("gradient", "Gradient", unlimited_int, 0.0, 0.0, 1.0, "JWildfire color output mode. **Only mode 0 honored.** This variation produces grayscale output regardless of palette."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: HYPERBOLICTILE_2D,
    wgsl_3d: HYPERBOLICTILE_3D,
};

// Per call: outer 7 × inner 8 = 56 mirror() calls × ~20 ops = ~1100 ops.
// Within TDR.
const HYPERBOLICTILE_2D: &str = r#"
fn hyperbolictile_tocart(polar: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(polar.x * cos(polar.y), polar.x * sin(polar.y));
}

fn hyperbolictile_topolar(c: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(sqrt(c.x * c.x + c.y * c.y), atan2(c.y, c.x));
}

// Möbius reflection across a hyperbolic line. The line is encoded as
// polar (radius, angle): radius < 100 means a circular arc (inverted
// circle), radius >= 100 means a straight line through the origin.
fn hyperbolictile_mirror(line: vec2<f32>, point: vec2<f32>) -> vec2<f32> {
    if (line.x < 100.0) {
        let linecenter = hyperbolictile_tocart(line);
        let cart = hyperbolictile_tocart(point);
        let dx = cart.x - linecenter.x;
        let dy = cart.y - linecenter.y;
        let r1 = line.x * line.x - 1.0;
        let r2 = max(dx * dx + dy * dy, 1.0e-32);
        let rr = r1 / r2;
        return hyperbolictile_topolar(linecenter + vec2<f32>(dx * rr, dy * rr));
    } else {
        return vec2<f32>(point.x, -point.y + 2.0 * line.y + 3.14159265);
    }
}

fn variation_glsl_hyperbolictile(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vrc: ptr<function, vec3<f32>>) -> vec2<f32> {
    let resolution = max(get_param(xform_id, variation_id, 0u), 100.0);
    let i_f = floor(rng_nextf(rng) * resolution);
    let j_f = floor(rng_nextf(rng) * resolution);
    let x = i_f + 0.5;
    let y = j_f + 0.5;
    let position = vec2<f32>(2.0 * x / resolution - 1.0, 2.0 * y / resolution - 1.0);

    let pi = 3.14159265;
    let pi_2 = pi * 0.5;
    let pi_3_4 = pi * 0.75;
    let l1 = vec2<f32>(1000.0, 0.0);
    let l2 = vec2<f32>(1000.0, pi / 8.0);
    let l3 = vec2<f32>(sqrt(sqrt(2.0) + 1.0), pi_2);

    var pp = hyperbolictile_topolar(position);
    var color: f32 = 0.0;
    if (pp.x > 1.0) {
        color = 0.0;
    } else {
        var g2: i32 = 0;
        var k1: i32 = 1;
        for (var i: i32 = 0; i < 7; i = i + 1) {
            if (k1 == 0) { continue; }
            var k: i32 = 1;
            for (var j: i32 = 1; j < 8; j = j + 1) {
                // Note: Java precedence is `k == 0 || (...) && (...)`
                // which is `k == 0 || ((py in band))`. Mirrors that.
                if (k == 0 || (pp.y >= pi_2 && pp.y < pi_3_4)) {
                    k = 0;
                } else {
                    g2 = g2 + 1;
                    pp = hyperbolictile_mirror(l2, hyperbolictile_mirror(l1, pp));
                }
            }
            let p1 = hyperbolictile_mirror(l3, pp);
            if (p1.y >= pi_2 && p1.y < pi_3_4) {
                k1 = 0;
            }
            pp = p1;
        }
        color = f32(g2 - 2 * (g2 / 2));  // g2 mod 2 (i32 truncation)
    }
    *vrc = vec3<f32>(color, color, color);
    return vec2<f32>(i_f / resolution - 0.5, j_f / resolution - 0.5);
}
"#;

const HYPERBOLICTILE_3D: &str = r#"
fn hyperbolictile_tocart(polar: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(polar.x * cos(polar.y), polar.x * sin(polar.y));
}

fn hyperbolictile_topolar(c: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(sqrt(c.x * c.x + c.y * c.y), atan2(c.y, c.x));
}

// Möbius reflection across a hyperbolic line. The line is encoded as
// polar (radius, angle): radius < 100 means a circular arc (inverted
// circle), radius >= 100 means a straight line through the origin.
fn hyperbolictile_mirror(line: vec2<f32>, point: vec2<f32>) -> vec2<f32> {
    if (line.x < 100.0) {
        let linecenter = hyperbolictile_tocart(line);
        let cart = hyperbolictile_tocart(point);
        let dx = cart.x - linecenter.x;
        let dy = cart.y - linecenter.y;
        let r1 = line.x * line.x - 1.0;
        let r2 = max(dx * dx + dy * dy, 1.0e-32);
        let rr = r1 / r2;
        return hyperbolictile_topolar(linecenter + vec2<f32>(dx * rr, dy * rr));
    } else {
        return vec2<f32>(point.x, -point.y + 2.0 * line.y + 3.14159265);
    }
}

fn variation_glsl_hyperbolictile(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vrc: ptr<function, vec3<f32>>) -> vec3<f32> {
    let resolution = max(get_param(xform_id, variation_id, 0u), 100.0);
    let i_f = floor(rng_nextf(rng) * resolution);
    let j_f = floor(rng_nextf(rng) * resolution);
    let position = vec2<f32>(2.0 * (i_f + 0.5) / resolution - 1.0, 2.0 * (j_f + 0.5) / resolution - 1.0);

    let pi = 3.14159265;
    let pi_2 = pi * 0.5;
    let pi_3_4 = pi * 0.75;
    let l1 = vec2<f32>(1000.0, 0.0);
    let l2 = vec2<f32>(1000.0, pi / 8.0);
    let l3 = vec2<f32>(sqrt(sqrt(2.0) + 1.0), pi_2);

    var pp = hyperbolictile_topolar(position);
    var color: f32 = 0.0;
    if (pp.x > 1.0) {
        color = 0.0;
    } else {
        var g2: i32 = 0;
        var k1: i32 = 1;
        for (var i: i32 = 0; i < 7; i = i + 1) {
            if (k1 == 0) { continue; }
            var k: i32 = 1;
            for (var j: i32 = 1; j < 8; j = j + 1) {
                if (k == 0 || (pp.y >= pi_2 && pp.y < pi_3_4)) {
                    k = 0;
                } else {
                    g2 = g2 + 1;
                    pp = hyperbolictile_mirror(l2, hyperbolictile_mirror(l1, pp));
                }
            }
            let p1 = hyperbolictile_mirror(l3, pp);
            if (p1.y >= pi_2 && p1.y < pi_3_4) { k1 = 0; }
            pp = p1;
        }
        color = f32(g2 - 2 * (g2 / 2));
    }
    *vrc = vec3<f32>(color, color, color);
    return vec3<f32>(i_f / resolution - 0.5, j_f / resolution - 0.5, p.z);
}
"#;

// =============================================================================
// glsl_mandala
// =============================================================================

/// Mandala pattern — kaleidoscope fold (twice, with cross-fold mix)
/// followed by an Apollonian-style iteration (up to `loops` iters,
/// default 64). Per-channel "invert" flags can flip each RGB component.
///
/// 10 params: `density_pixels` + 9 algorithm controls (no `seed` or
/// `time` here — JWildfire's source omits the seed-based time
/// randomization).
/// 
/// # Authors
/// - Jesus Sosa
pub static GLSL_MANDALA: VariationDef = VariationDef {
    name: "glsl_mandala",
    aliases: &[],
    display_name: "GLSL Mandala",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::WritesRgb],
    parameters: &[
        param!("density_pixels", "Density Pixels", unlimited_int, 1000000.0, 100.0, 10000000.0, "Discrete grid resolution."),
        param!("m_x", "mX", unlimited_float, 0.025, -1.0, 1.0, "Z-component scalar fed into the per-fold iteration as the initial depth (`p.z = mx · v`)."),
        param!("m_y", "mY", unlimited_float, -0.001245675, -1.0, 1.0, "Apollonian-fold offset along the blue channel (`mY / π`)."),
        param!("scale", "Scale", unlimited_float, 2.0, 0.0, 5.5, "Inverse zoom — uv is scaled by `5.5 - scale`. Higher = zoom in. JWildfire's UI label is 'scale' but the effect is inverted from the name."),
        param!("sides", "Sides", unlimited_float, 12.0, 2.0, 64.0, "Kaleidoscope sector count. The kaleidoscope angle is `π / floor(sides)`."),
        param!("multiply", "Multiply", unlimited_float, 1.5, 0.0, 10.0, "When > 0.001, a 'multiply' post-fold step is applied (mod the kaleidoscoped uv by `floor(multiply)`)."),
        param!("loops", "Loops", unlimited_float, 64.0, 1.0, 73.0, "Apollonian iteration count. **GPU-clamped to ≤ 73** (matches JWildfire's hard limit)."),
        param!("invert_r", "Invert R", unlimited_int, 0.0, 0.0, 1.0, "0 = pass red channel through; 1 = invert (`1 - r`)."),
        param!("invert_g", "Invert G", unlimited_int, 0.0, 0.0, 1.0, "Same for green."),
        param!("invert_b", "Invert B", unlimited_int, 1.0, 0.0, 1.0, "Same for blue. Default 1 — JWildfire ships this on."),
        param!("gradient", "Gradient", unlimited_int, 0.0, 0.0, 1.0, "JWildfire color output mode. **Only mode 0 honored.**"),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: MANDALA_2D,
    wgsl_3d: MANDALA_3D,
};

const MANDALA_2D: &str = r#"
fn mandala_kscope(v: vec2<f32>, k: f32) -> vec2<f32> {
    var a = atan2(v.y, v.x);
    let two_k = 2.0 * k;
    a = a - two_k * floor(a / two_k);   // mod into [0, 2k]
    a = abs(a - k);                      // reflect to [0, k]
    let r = length(v);
    return vec2<f32>(r * cos(a), r * sin(a));
}

fn variation_glsl_mandala(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vrc: ptr<function, vec3<f32>>) -> vec2<f32> {
    let resolution = max(get_param(xform_id, variation_id, 0u), 100.0);
    let mx = get_param(xform_id, variation_id, 1u);
    let my = get_param(xform_id, variation_id, 2u);
    let scale = get_param(xform_id, variation_id, 3u);
    let sides = max(get_param(xform_id, variation_id, 4u), 1.0);
    let multiply = get_param(xform_id, variation_id, 5u);
    let loops = min(u32(get_param(xform_id, variation_id, 6u)), 73u);
    let inv_r = u32(get_param(xform_id, variation_id, 7u));
    let inv_g = u32(get_param(xform_id, variation_id, 8u));
    let inv_b = u32(get_param(xform_id, variation_id, 9u));

    let i_f = floor(rng_nextf(rng) * resolution);
    let j_f = floor(rng_nextf(rng) * resolution);
    let xt = i_f + 0.5;
    let yt = j_f + 0.5;

    let rcpi = 0.318309886183791;  // 1/π
    let zoom = 5.5 - scale;
    let uv = vec2<f32>(zoom * (2.0 * xt / resolution - 1.0), zoom * (2.0 * yt / resolution - 1.0));

    let k = 3.14159265 / floor(sides);
    let s = mandala_kscope(uv, k);
    let t = mandala_kscope(s, k);
    let v = dot(t, s);
    var u = mix(s, t, vec2<f32>(cos(v)));

    if (multiply > 0.001) {
        let m_int = floor(multiply);
        let m_safe = max(m_int, 1.0e-32);
        let t1 = vec2<f32>(u.y, u.x);
        let t2 = t1 - m_safe * floor(t1 / m_safe);
        let t3 = vec2<f32>(-u.x, -u.y);
        let t4 = vec2<f32>(u.y, u.x);
        // mod(t2, t3) — for negative divisors we want the math mod
        let t3_safe = sign(t3) * max(abs(t3), vec2<f32>(1.0e-32));
        let t5_mod = t2 - t3_safe * floor(t2 / t3_safe);
        let t5 = t4 + t5_mod;
        u = vec2<f32>(t5.y, t5.x);
    }

    var pp = vec3<f32>(u.x, u.y, mx * v);
    for (var l: u32 = 0u; l < loops; l = l + 1u) {
        let d = max(dot(pp, pp), 1.0e-32);
        let t1 = vec3<f32>(1.3, 0.999, 0.678);
        let t2 = abs(pp) / d - vec3<f32>(1.0, 1.02, my * rcpi);
        let t3 = abs(t2);
        let t4 = t1 * t3;
        pp = vec3<f32>(t4.x, t4.z, t4.y);
    }
    if (inv_r == 1u) { pp.x = 1.0 - pp.x; }
    if (inv_g == 1u) { pp.y = 1.0 - pp.y; }
    if (inv_b == 1u) { pp.z = 1.0 - pp.z; }
    *vrc = pp;

    return vec2<f32>(i_f / resolution - 0.5, j_f / resolution - 0.5);
}
"#;

const MANDALA_3D: &str = r#"
fn mandala_kscope(v: vec2<f32>, k: f32) -> vec2<f32> {
    var a = atan2(v.y, v.x);
    let two_k = 2.0 * k;
    a = a - two_k * floor(a / two_k);   // mod into [0, 2k]
    a = abs(a - k);                      // reflect to [0, k]
    let r = length(v);
    return vec2<f32>(r * cos(a), r * sin(a));
}

fn variation_glsl_mandala(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vrc: ptr<function, vec3<f32>>) -> vec3<f32> {
    let resolution = max(get_param(xform_id, variation_id, 0u), 100.0);
    let mx = get_param(xform_id, variation_id, 1u);
    let my = get_param(xform_id, variation_id, 2u);
    let scale = get_param(xform_id, variation_id, 3u);
    let sides = max(get_param(xform_id, variation_id, 4u), 1.0);
    let multiply = get_param(xform_id, variation_id, 5u);
    let loops = min(u32(get_param(xform_id, variation_id, 6u)), 73u);
    let inv_r = u32(get_param(xform_id, variation_id, 7u));
    let inv_g = u32(get_param(xform_id, variation_id, 8u));
    let inv_b = u32(get_param(xform_id, variation_id, 9u));

    let i_f = floor(rng_nextf(rng) * resolution);
    let j_f = floor(rng_nextf(rng) * resolution);
    let zoom = 5.5 - scale;
    let uv = vec2<f32>(zoom * (2.0 * (i_f + 0.5) / resolution - 1.0), zoom * (2.0 * (j_f + 0.5) / resolution - 1.0));

    let rcpi = 0.318309886183791;
    let k = 3.14159265 / floor(sides);
    let s = mandala_kscope(uv, k);
    let t = mandala_kscope(s, k);
    let v = dot(t, s);
    var u = mix(s, t, vec2<f32>(cos(v)));

    if (multiply > 0.001) {
        let m_int = floor(multiply);
        let m_safe = max(m_int, 1.0e-32);
        let t1 = vec2<f32>(u.y, u.x);
        let t2 = t1 - m_safe * floor(t1 / m_safe);
        let t3 = vec2<f32>(-u.x, -u.y);
        let t4 = vec2<f32>(u.y, u.x);
        let t3_safe = sign(t3) * max(abs(t3), vec2<f32>(1.0e-32));
        let t5_mod = t2 - t3_safe * floor(t2 / t3_safe);
        let t5 = t4 + t5_mod;
        u = vec2<f32>(t5.y, t5.x);
    }

    var pp = vec3<f32>(u.x, u.y, mx * v);
    for (var l: u32 = 0u; l < loops; l = l + 1u) {
        let d = max(dot(pp, pp), 1.0e-32);
        let t1 = vec3<f32>(1.3, 0.999, 0.678);
        let t2 = abs(pp) / d - vec3<f32>(1.0, 1.02, my * rcpi);
        let t3 = abs(t2);
        let t4 = t1 * t3;
        pp = vec3<f32>(t4.x, t4.z, t4.y);
    }
    if (inv_r == 1u) { pp.x = 1.0 - pp.x; }
    if (inv_g == 1u) { pp.y = 1.0 - pp.y; }
    if (inv_b == 1u) { pp.z = 1.0 - pp.z; }
    *vrc = pp;
    return vec3<f32>(i_f / resolution - 0.5, j_f / resolution - 0.5, p.z);
}
"#;

// =============================================================================
// glsl_squares
// =============================================================================

/// Sierpinski-carpet-style square tiling. Tests whether the sample
/// point falls in a "carpet" (an N-deep Sierpinski subdivision of the
/// unit square minus the center cell), running the test 4 times with
/// small offsets to give a multi-layer effect. Time-driven color
/// inversion via the `cos`/`tan`/`sin` of `time`.
///
/// 9 params: standard 4 + `n` (recursion depth), `dir` (zoom-in vs
/// zoom-out), `fr`/`fg`/`fb` (color frequencies).
/// 
/// # Authors
/// - Jesus Sosa
pub static GLSL_SQUARES: VariationDef = VariationDef {
    name: "glsl_squares",
    aliases: &[],
    display_name: "GLSL Squares",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::WritesRgb],
    parameters: &[
        param!("density_pixels", "Density Pixels", unlimited_int, 1000000.0, 100.0, 10000000.0, "Discrete grid resolution."),
        param!("seed", "Seed", unlimited_int, 5000.0, 0.0, 10000.0, "JWildfire CPU-only — set `time` directly."),
        param!("time", "Time", unlimited_float, 85.5, -10000.0, 10000.0, "Animation parameter. Drives both the per-iteration zoom (`pow(0.5, mod(direction · time, 1))`) and the initial color (`(cos(time), tan(time), sin(time))`)."),
        param!("n", "N", unlimited_int, 3.0, 1.0, 8.0, "Carpet recursion depth. **GPU-clamped to ≤ 8** — each level costs `mod` + branch. Default 3."),
        param!("direction", "Direction", unlimited_int, 1.0, 0.0, 1.0, "Zoom direction. 1 = zoom in over time (`+0.1·time`); 0 = zoom out (`-1·time`)."),
        param!("fr", "Red Fac.", unlimited_float, 1.8, -10.0, 10.0, "Red-channel final-cosine frequency."),
        param!("fg", "Green Fac.", unlimited_float, 1.9, -10.0, 10.0, "Green-channel frequency."),
        param!("fb", "Blue Fac.", unlimited_float, 2.2, -10.0, 10.0, "Blue-channel frequency."),
        param!("gradient", "Gradient", unlimited_int, 0.0, 0.0, 1.0, "JWildfire color output mode. **Only mode 0 honored.**"),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: SQUARES_2D,
    wgsl_3d: SQUARES_3D,
};

const SQUARES_2D: &str = r#"
fn squares_hit(p_in: vec2<f32>, n: u32, time: f32, dir: u32) -> bool {
    var direction: f32;
    if (dir == 1u) { direction = 0.1; } else { direction = -1.0; }
    let arg = direction * time;
    let arg_mod = arg - floor(arg);
    let scale = pow(0.5, arg_mod);
    var coord_iter = p_in / scale;
    for (var i: u32 = 0u; i < n; i = i + 1u) {
        let sectors = floor(coord_iter * 3.0);
        if (sectors.x == 1.0 && sectors.y == 1.0) {
            return false;
        }
        coord_iter = coord_iter * 3.0 - sectors;
    }
    return true;
}

fn variation_glsl_squares(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vrc: ptr<function, vec3<f32>>) -> vec2<f32> {
    let resolution = max(get_param(xform_id, variation_id, 0u), 100.0);
    let time = get_param(xform_id, variation_id, 2u);
    let n = min(u32(get_param(xform_id, variation_id, 3u)), 8u);
    let dir = u32(get_param(xform_id, variation_id, 4u));
    let fr = get_param(xform_id, variation_id, 5u);
    let fg = get_param(xform_id, variation_id, 6u);
    let fb = get_param(xform_id, variation_id, 7u);

    let i_f = floor(rng_nextf(rng) * resolution);
    let j_f = floor(rng_nextf(rng) * resolution);
    let x = i_f + 0.5;
    let y = j_f + 0.5;

    var coord_orig = vec2<f32>(abs(x / resolution - 0.5), abs(y / resolution - 0.5));
    coord_orig = coord_orig - floor(coord_orig);  // mod 1.0
    var color = vec3<f32>(cos(time), tan(time), sin(time));
    for (var i: u32 = 0u; i < 4u; i = i + 1u) {
        let offset = vec2<f32>(f32(i) * 0.1);
        if (squares_hit(offset + coord_orig, n, time, dir)) {
            color = vec3<f32>(1.0) - color;
        }
    }
    *vrc = vec3<f32>(cos(color.x * fr), cos(color.y * fg), cos(color.z * fb));
    return vec2<f32>(i_f / resolution - 0.5, j_f / resolution - 0.5);
}
"#;

const SQUARES_3D: &str = r#"
fn squares_hit(p_in: vec2<f32>, n: u32, time: f32, dir: u32) -> bool {
    var direction: f32;
    if (dir == 1u) { direction = 0.1; } else { direction = -1.0; }
    let arg = direction * time;
    let arg_mod = arg - floor(arg);
    let scale = pow(0.5, arg_mod);
    var coord_iter = p_in / scale;
    for (var i: u32 = 0u; i < n; i = i + 1u) {
        let sectors = floor(coord_iter * 3.0);
        if (sectors.x == 1.0 && sectors.y == 1.0) {
            return false;
        }
        coord_iter = coord_iter * 3.0 - sectors;
    }
    return true;
}

fn variation_glsl_squares(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vrc: ptr<function, vec3<f32>>) -> vec3<f32> {
    let resolution = max(get_param(xform_id, variation_id, 0u), 100.0);
    let time = get_param(xform_id, variation_id, 2u);
    let n = min(u32(get_param(xform_id, variation_id, 3u)), 8u);
    let dir = u32(get_param(xform_id, variation_id, 4u));
    let fr = get_param(xform_id, variation_id, 5u);
    let fg = get_param(xform_id, variation_id, 6u);
    let fb = get_param(xform_id, variation_id, 7u);

    let i_f = floor(rng_nextf(rng) * resolution);
    let j_f = floor(rng_nextf(rng) * resolution);
    var coord_orig = vec2<f32>(abs((i_f + 0.5) / resolution - 0.5), abs((j_f + 0.5) / resolution - 0.5));
    coord_orig = coord_orig - floor(coord_orig);
    var color = vec3<f32>(cos(time), tan(time), sin(time));
    for (var i: u32 = 0u; i < 4u; i = i + 1u) {
        let offset = vec2<f32>(f32(i) * 0.1);
        if (squares_hit(offset + coord_orig, n, time, dir)) {
            color = vec3<f32>(1.0) - color;
        }
    }
    *vrc = vec3<f32>(cos(color.x * fr), cos(color.y * fg), cos(color.z * fb));
    return vec3<f32>(i_f / resolution - 0.5, j_f / resolution - 0.5, p.z);
}
"#;

// =============================================================================
// glsl_hoshi
// =============================================================================

/// Star/hoshi tiling — iterative fold + rotate (steps iterations,
/// default 28) with HSV color derived from the final point's angle.
/// Time-driven per-iteration rotation makes the pattern animate.
///
/// 6 params: `density_pixels`, `seed` (CPU-only), `time`, `steps`,
/// `scale`, `translate`.
/// 
/// # Authors
/// - Jesus Sosa
pub static GLSL_HOSHI: VariationDef = VariationDef {
    name: "glsl_hoshi",
    aliases: &[],
    display_name: "GLSL Hoshi",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::WritesRgb],
    parameters: &[
        param!("density_pixels", "Density Pixels", unlimited_int, 1000000.0, 100.0, 10000000.0, "Discrete grid resolution."),
        param!("seed", "Seed", unlimited_int, 100000.0, 0.0, 1000000.0, "JWildfire CPU-only — set `time` directly."),
        param!("time", "Time", unlimited_float, 10.0, 0.0001, 10000.0, "Animation parameter. Drives per-iteration rotation angles. Avoid 0 — `10/time` divides by it. Clamped to ≥ 1e-4 at runtime."),
        param!("steps", "Steps", unlimited_int, 28.0, 1.0, 50.0, "Iteration count. **GPU-clamped to ≤ 50.** Default 28."),
        param!("scale", "Scale", unlimited_float, 1.25, 0.0, 5.0, "Per-iteration scale multiplier applied after the fold."),
        param!("translate", "Translate", unlimited_float, 1.5, -5.0, 5.0, "Per-iteration translation along both x and y (applied as `p -= vec2(translate, translate)` after scaling)."),
        param!("gradient", "Gradient", unlimited_int, 0.0, 0.0, 1.0, "JWildfire color output mode. **Only mode 0 honored.**"),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: HOSHI_2D,
    wgsl_3d: HOSHI_3D,
};

const HOSHI_2D: &str = r#"
fn hoshi_hsv(h_in: f32, s: f32, v: f32) -> vec3<f32> {
    var t1 = vec3<f32>(3.0, 2.0, 1.0);
    t1 = (t1 + h_in) / 3.0;
    var t2 = (t1 - floor(t1)) * 6.0 - 3.0;
    t2 = abs(t2) - 1.0;
    t2 = clamp(t2, vec3<f32>(0.0), vec3<f32>(1.0));
    return mix(vec3<f32>(3.1), t2, vec3<f32>(s)) * v;
}

fn hoshi_rotate(p: vec2<f32>, a: f32) -> vec2<f32> {
    let c = cos(a);
    let s = sin(a);
    return vec2<f32>(p.x * c - p.y * s, p.x * s + p.y * c);
}

fn variation_glsl_hoshi(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vrc: ptr<function, vec3<f32>>) -> vec2<f32> {
    let resolution = max(get_param(xform_id, variation_id, 0u), 100.0);
    let time = max(get_param(xform_id, variation_id, 2u), 1.0e-4);
    let steps = min(u32(get_param(xform_id, variation_id, 3u)), 50u);
    let scale = get_param(xform_id, variation_id, 4u);
    let translate = get_param(xform_id, variation_id, 5u);

    let i_f = floor(rng_nextf(rng) * resolution);
    let j_f = floor(rng_nextf(rng) * resolution);

    var pp = vec2<f32>(2.0 * i_f / resolution - 1.0, 2.0 * j_f / resolution - 1.0);
    pp = pp * 0.182;
    let x_init = pp.y;
    pp = abs((pp - 4.0 * floor(pp / 4.0)) - 2.0);  // abs(mod(p, 4) - 2)

    let fold = vec2<f32>(-0.5);
    let trans = vec2<f32>(translate);
    for (var k: u32 = 0u; k < steps; k = k + 1u) {
        // Java loop is `for (int i = steps; i > 0; i--)` — equivalent
        // count but the `i` value used in the rotation differs. We
        // mirror the JWF body by deriving the original i value.
        let orig_i = f32(steps) - f32(k);
        pp = abs(pp - fold) + fold;
        pp = pp * scale - trans;
        let denom = 0.10 + sin(time * 0.0005 + orig_i * 0.5000001) * 0.4999 + 0.5 + (10.0 / time) + sin(time) / 100.0;
        pp = hoshi_rotate(pp, 3.14159 / max(denom, 1.0e-32));
    }
    let ii = x_init * x_init + atan2(pp.y, pp.x) + time * 0.02;
    var h = floor(ii * 4.0) / 8.0 + 1.107;
    let m = ii * 2.0 / 5.0;
    let m_mod = m - 0.25 * floor(m / 0.25);  // mod(m, 1/4)
    h = h + smoothstep(-0.1, 0.8, m_mod * 900.0) / 0.010 - 0.5;
    let v = smoothstep(-3.0, 3.0, length(pp));
    *vrc = hoshi_hsv(h, 1.0, v);

    return vec2<f32>(i_f / resolution - 0.5, j_f / resolution - 0.5);
}
"#;

const HOSHI_3D: &str = r#"
fn hoshi_hsv(h_in: f32, s: f32, v: f32) -> vec3<f32> {
    var t1 = vec3<f32>(3.0, 2.0, 1.0);
    t1 = (t1 + h_in) / 3.0;
    var t2 = (t1 - floor(t1)) * 6.0 - 3.0;
    t2 = abs(t2) - 1.0;
    t2 = clamp(t2, vec3<f32>(0.0), vec3<f32>(1.0));
    return mix(vec3<f32>(3.1), t2, vec3<f32>(s)) * v;
}

fn hoshi_rotate(p: vec2<f32>, a: f32) -> vec2<f32> {
    let c = cos(a);
    let s = sin(a);
    return vec2<f32>(p.x * c - p.y * s, p.x * s + p.y * c);
}

fn variation_glsl_hoshi(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vrc: ptr<function, vec3<f32>>) -> vec3<f32> {
    let resolution = max(get_param(xform_id, variation_id, 0u), 100.0);
    let time = max(get_param(xform_id, variation_id, 2u), 1.0e-4);
    let steps = min(u32(get_param(xform_id, variation_id, 3u)), 50u);
    let scale = get_param(xform_id, variation_id, 4u);
    let translate = get_param(xform_id, variation_id, 5u);

    let i_f = floor(rng_nextf(rng) * resolution);
    let j_f = floor(rng_nextf(rng) * resolution);
    var pp = vec2<f32>(2.0 * i_f / resolution - 1.0, 2.0 * j_f / resolution - 1.0) * 0.182;
    let x_init = pp.y;
    pp = abs((pp - 4.0 * floor(pp / 4.0)) - 2.0);

    let fold = vec2<f32>(-0.5);
    let trans = vec2<f32>(translate);
    for (var k: u32 = 0u; k < steps; k = k + 1u) {
        let orig_i = f32(steps) - f32(k);
        pp = abs(pp - fold) + fold;
        pp = pp * scale - trans;
        let denom = 0.10 + sin(time * 0.0005 + orig_i * 0.5000001) * 0.4999 + 0.5 + (10.0 / time) + sin(time) / 100.0;
        pp = hoshi_rotate(pp, 3.14159 / max(denom, 1.0e-32));
    }
    let ii = x_init * x_init + atan2(pp.y, pp.x) + time * 0.02;
    var h = floor(ii * 4.0) / 8.0 + 1.107;
    let m = ii * 2.0 / 5.0;
    let m_mod = m - 0.25 * floor(m / 0.25);
    h = h + smoothstep(-0.1, 0.8, m_mod * 900.0) / 0.010 - 0.5;
    let v = smoothstep(-3.0, 3.0, length(pp));
    *vrc = hoshi_hsv(h, 1.0, v);
    return vec3<f32>(i_f / resolution - 0.5, j_f / resolution - 0.5, p.z);
}
"#;
