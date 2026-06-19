//! `glsl_*` family — iterative-fractal subset (4 variations).
//!
//! Shadertoy-style 2D fractals hand-ported by Jesus Sosa to JWildfire
//! (Oct 2018). All four share the same call shape — uniform random
//! `(i, j)` sample in `[0, density_pixels)²`, mapped into the
//! algorithm's input space, iterated, then converted to RGB. Spatial
//! output is always the uniform `[-0.5, 0.5]²` sample (same for every
//! `glsl_*` variation in this file and in the sibling tilings/fields
//! files); the visual content is entirely in the per-pixel direct-RGB
//! color computation, written to `*vrc` via `Feature::WritesRgb`.
//!
//! **`gradient` param semantics**: JWildfire flames using these
//! variations carry a `gradient` parameter. Mode 0 (direct RGB) is
//! what we honor — `*vrc = computed_color`. Mode 1 (palette index)
//! would require both `WritesColor` and `WritesRgb`, and the
//! unwritten register's sentinel-init darkens the plot-time blend
//! (see `Feature::WritesRgb` doc). Value accepted for round-trip but
//! mode 1 falls through to mode 0 in our port. Document per-variation.
//!
//! **CPU-only params** (`seed`, sometimes others): JWildfire seeds a
//! Java `Random` and uses it to derive `time` (or other animation
//! params) at flame-load. We don't replicate Java's RNG; the
//! corresponding params are accepted for `.flame` XML round-trip but
//! the user must set the derived value (typically `time`) directly.
//!
//! Sources:
//! - [`GLSLMandelBox2DFunc.java`](../../../output/variation-jwf-source/GLSLMandelBox2DFunc.java)
//! - [`GLSLKaliSetFunc.java`](../../../output/variation-jwf-source/GLSLKaliSetFunc.java)
//! - [`GLSLKaliSet2Func.java`](../../../output/variation-jwf-source/GLSLKaliSet2Func.java)
//! - [`GLSLApollonianFunc.java`](../../../output/variation-jwf-source/GLSLApollonianFunc.java)

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// glsl_mandelbox2D
// =============================================================================

/// 2D Mandelbox with shadertoy-style direct-RGB color output. Picks a
/// uniform random point, maps to `[-7, 7]²`, runs 20 iterations of
/// boxfold + ballfold + scaling with `time`-derived scale factor, then
/// derives RGB from the final distance estimate via interleaved
/// sin/cos.
///
/// 4 params: `density_pixels`, `seed` (CPU-only), `time`, `gradient`
/// (mode 0 honored only).
///
/// # Authors
/// - Jesus Sosa
pub static GLSL_MANDELBOX2D: VariationDef = VariationDef {
    name: "glsl_mandelbox2D",
    aliases: &[],
    display_name: "GLSL MandelBox2D",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::WritesRgb, Feature::NeverZ],
    parameters: &[
        param!("density_pixels", "Density Pixels", unlimited_int, 1000000.0, 100.0, 10000000.0, "Discrete grid resolution for sampling. Higher = smoother continuous output; lower = visible pixelation. JWildfire's 'Density Pixels' slider; clamped at runtime to `[100, 10000000]`."),
        param!("seed", "Seed", unlimited_int, 10000.0, 0.0, 10000.0, "JWildfire CPU-only: seeds a Java `Random` and assigns `time = rand(0, 10000)` as a side effect. **Not honored on GPU** — set `time` directly. Value accepted for round-trip."),
        param!("time", "Time", unlimited_float, 0.0, -10000.0, 10000.0, "Animation parameter. Embedded in the Mandelbox iteration as `O.z = -5 · cos(time · 0.1)`. Animate this for a flowing/morphing pattern."),
        param!("gradient", "Gradient", unlimited_int, 0.0, 0.0, 1.0, "JWildfire color output mode. **Only mode 0 (direct RGB) is honored.** Value accepted for round-trip; see module-level doc."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: MANDELBOX_2D,
    wgsl_3d: MANDELBOX_3D,
};

// Mandelbox per-call: 20 inner iterations × ~15 ops = ~300 ops. With
// 32K × 256 = ~2.5B ops/dispatch. Well within TDR. No clamps needed.
const MANDELBOX_2D: &str = r#"
fn variation_glsl_mandelbox2D(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vrc: ptr<function, vec3<f32>>) -> vec2<f32> {
    let resolution = max(get_param(xform_id, variation_id, 0u), 100.0);
    let time = get_param(xform_id, variation_id, 2u);

    let i_f = floor(rng_nextf(rng) * resolution);
    let j_f = floor(rng_nextf(rng) * resolution);
    let x = i_f + 0.5;
    let y = j_f + 0.5;

    var i_vec = vec2<f32>(
        7.0 * (x + x - resolution) / resolution,
        7.0 * (y + y - resolution) / resolution,
    );
    // O.xy = original I (added each iter as the +c offset). O.z = scaling
    // factor (time-derived, fixed). O.w = scratch (set to length(I) inside
    // the loop for the ballfold test).
    var o = vec4<f32>(i_vec.x, i_vec.y, -5.0 * cos(time * 0.1), 1.0);
    var d: f32 = 1.0;
    for (var k: u32 = 0u; k < 20u; k = k + 1u) {
        i_vec = clamp(i_vec, vec2<f32>(-1.0), vec2<f32>(1.0)) * 2.0 - i_vec;
        o.w = length(i_vec);
        var b: f32;
        if (o.w < 0.5) { b = 4.0; }
        else if (o.w < 1.0) { b = 1.0 / o.w; }
        else { b = 1.0; }
        i_vec = i_vec * (o.z * b) + vec2<f32>(o.x, o.y);
        d = b * d * abs(o.z) + 1.0;
    }
    let d_final = pow(length(i_vec) / d, 0.1) * 5.0;
    let color = vec3<f32>(cos(d_final), sin(10.0 * d_final + 1.0), cos(3.0 * d_final + 1.0)) * 0.5 + 0.5;
    *vrc = color;
    return vec2<f32>(i_f / resolution - 0.5, j_f / resolution - 0.5);
}
"#;

const MANDELBOX_3D: &str = r#"
fn variation_glsl_mandelbox2D(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vrc: ptr<function, vec3<f32>>) -> vec3<f32> {
    let resolution = max(get_param(xform_id, variation_id, 0u), 100.0);
    let time = get_param(xform_id, variation_id, 2u);
    let i_f = floor(rng_nextf(rng) * resolution);
    let j_f = floor(rng_nextf(rng) * resolution);
    var i_vec = vec2<f32>(
        7.0 * ((i_f + 0.5) + (i_f + 0.5) - resolution) / resolution,
        7.0 * ((j_f + 0.5) + (j_f + 0.5) - resolution) / resolution,
    );
    var o = vec4<f32>(i_vec.x, i_vec.y, -5.0 * cos(time * 0.1), 1.0);
    var d: f32 = 1.0;
    for (var k: u32 = 0u; k < 20u; k = k + 1u) {
        i_vec = clamp(i_vec, vec2<f32>(-1.0), vec2<f32>(1.0)) * 2.0 - i_vec;
        o.w = length(i_vec);
        var b: f32;
        if (o.w < 0.5) { b = 4.0; }
        else if (o.w < 1.0) { b = 1.0 / o.w; }
        else { b = 1.0; }
        i_vec = i_vec * (o.z * b) + vec2<f32>(o.x, o.y);
        d = b * d * abs(o.z) + 1.0;
    }
    let d_final = pow(length(i_vec) / d, 0.1) * 5.0;
    *vrc = vec3<f32>(cos(d_final), sin(10.0 * d_final + 1.0), cos(3.0 * d_final + 1.0)) * 0.5 + 0.5;
    return vec3<f32>(i_f / resolution - 0.5, j_f / resolution - 0.5, p.z);
}
"#;

// =============================================================================
// glsl_kaliset
// =============================================================================

/// "KaliSet" iterative fractal (Mikael Hvidtfeldt Christensen / Kali).
/// Maps the sample point into `[-10, 10]²`, then iterates a sphere
/// inversion followed by a time-rotated scale-and-shift, accumulating
/// the running squared-radius into `rsum`. RGB output is
/// `(cos(col), cos(2col), cos(4col))` where `col = rsum/2`.
///
/// 7 params: standard 4 + `N` (iter count), `shift_x`/`shift_y`
/// (per-step offset).
/// 
/// # Authors
/// - Jesus Sosa
pub static GLSL_KALISET: VariationDef = VariationDef {
    name: "glsl_kaliset",
    aliases: &[],
    display_name: "GLSL KaliSet",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::WritesRgb, Feature::NeverZ],
    parameters: &[
        param!("density_pixels", "Density Pixels", unlimited_int, 1000000.0, 100.0, 10000000.0, "Discrete grid resolution for sampling. See module-level doc for shared semantics."),
        param!("seed", "Seed", unlimited_int, 10000.0, 0.0, 10000.0, "JWildfire CPU-only — accepted for round-trip; set `time` directly."),
        param!("time", "Time", unlimited_float, 200.0, 1.0, 1000.0, "Animation parameter. Drives the rotation angle (`time/603 · 2π` for cos, `time/407.4 · 2π` for sin) and the zoom factor (`time/184 + 1`). JWildfire clamps to `[1, 1000]`; we follow suit."),
        param!("n_iters", "N", unlimited_int, 60.0, 1.0, 100.0, "Inner iteration count. **GPU-clamped to ≤ 100.** Higher = more detail but linearly more cost. JWildfire's default is 60."),
        param!("shift_x", "Shift X", unlimited_float, -0.22, -5.0, 5.0, "Per-iteration X offset added after rotation and zoom. Small changes here dramatically alter the fractal's structure."),
        param!("shift_y", "Shift Y", unlimited_float, -0.21, -5.0, 5.0, "Per-iteration Y offset. Same sensitivity as shift_x."),
        param!("gradient", "Gradient", unlimited_int, 0.0, 0.0, 1.0, "JWildfire color output mode. **Only mode 0 (direct RGB) honored.** Value accepted for round-trip."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: KALISET_2D,
    wgsl_3d: KALISET_3D,
};

// KaliSet per-call: N (≤ 100) × ~10 ops = ~1000 worst-case ops.
// With 32K × 256 ≈ 8B ops/dispatch. Within TDR. N clamp keeps it safe.
const KALISET_2D: &str = r#"
fn variation_glsl_kaliset(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vrc: ptr<function, vec3<f32>>) -> vec2<f32> {
    let resolution = max(get_param(xform_id, variation_id, 0u), 100.0);
    let time = clamp(get_param(xform_id, variation_id, 2u), 1.0, 1000.0);
    let n_iters = min(u32(get_param(xform_id, variation_id, 3u)), 100u);
    let shift_x = get_param(xform_id, variation_id, 4u);
    let shift_y = get_param(xform_id, variation_id, 5u);

    let i_f = floor(rng_nextf(rng) * resolution);
    let j_f = floor(rng_nextf(rng) * resolution);
    let x = i_f + 0.5;
    let y = j_f + 0.5;

    // Map cell center to [-10, 10]² (20·(uv - 0.5)).
    var v = vec2<f32>(x / resolution - 0.5, y / resolution - 0.5) * 20.0;

    let pi2 = 7.2;  // = 3.6 · 2 (JWF magic constant — half of 2π · 2.292)
    let c_cos = cos(time / 603.0 * pi2);
    let s_sin = sin(time / 407.4 * pi2);
    let shift = vec2<f32>(shift_x, shift_y);
    let zoom = time / 184.0 + 1.0;

    var rsum: f32 = 0.0;
    for (var k: u32 = 0u; k < n_iters; k = k + 1u) {
        var rr = v.x * v.x + v.y * v.y;
        if (rr > 1.0) {
            rr = 1.0 / rr;
            v = v * rr;
        }
        rsum = rsum * 0.99 + rr;
        v = vec2<f32>(c_cos * v.x - s_sin * v.y, s_sin * v.x + c_cos * v.y) * zoom + shift;
    }
    let col = rsum * 0.5;
    *vrc = vec3<f32>(cos(col * 1.0), cos(col * 2.0), cos(col * 4.0));
    return vec2<f32>(i_f / resolution - 0.5, j_f / resolution - 0.5);
}
"#;

const KALISET_3D: &str = r#"
fn variation_glsl_kaliset(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vrc: ptr<function, vec3<f32>>) -> vec3<f32> {
    let resolution = max(get_param(xform_id, variation_id, 0u), 100.0);
    let time = clamp(get_param(xform_id, variation_id, 2u), 1.0, 1000.0);
    let n_iters = min(u32(get_param(xform_id, variation_id, 3u)), 100u);
    let shift_x = get_param(xform_id, variation_id, 4u);
    let shift_y = get_param(xform_id, variation_id, 5u);

    let i_f = floor(rng_nextf(rng) * resolution);
    let j_f = floor(rng_nextf(rng) * resolution);
    var v = vec2<f32>((i_f + 0.5) / resolution - 0.5, (j_f + 0.5) / resolution - 0.5) * 20.0;

    let pi2 = 7.2;
    let c_cos = cos(time / 603.0 * pi2);
    let s_sin = sin(time / 407.4 * pi2);
    let shift = vec2<f32>(shift_x, shift_y);
    let zoom = time / 184.0 + 1.0;

    var rsum: f32 = 0.0;
    for (var k: u32 = 0u; k < n_iters; k = k + 1u) {
        var rr = v.x * v.x + v.y * v.y;
        if (rr > 1.0) {
            rr = 1.0 / rr;
            v = v * rr;
        }
        rsum = rsum * 0.99 + rr;
        v = vec2<f32>(c_cos * v.x - s_sin * v.y, s_sin * v.x + c_cos * v.y) * zoom + shift;
    }
    let col = rsum * 0.5;
    *vrc = vec3<f32>(cos(col * 1.0), cos(col * 2.0), cos(col * 4.0));
    return vec3<f32>(i_f / resolution - 0.5, j_f / resolution - 0.5, p.z);
}
"#;

// =============================================================================
// glsl_kaliset2
// =============================================================================

/// Second variant of KaliSet (Mikael Hvidtfeldt Christensen, ported by
/// Jesus Sosa). Same family as `glsl_kaliset` but with:
///   - An explicit rotation angle (`shift_x` × 2π) instead of two
///     time-driven trig terms
///   - A user-controlled inversion radius (`radio`)
///   - Per-channel "color frequency" knobs (`fr`, `fg`, `fb`) feeding
///     the cos-based RGB
///   - `rsum` accumulates via `max` instead of a damped sum
///
/// 11 params (most knobs of any of the four).
/// 
/// # Authors
/// - Jesus Sosa
pub static GLSL_KALISET2: VariationDef = VariationDef {
    name: "glsl_kaliset2",
    aliases: &[],
    display_name: "GLSL KaliSet 2",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::WritesRgb, Feature::NeverZ],
    parameters: &[
        param!("density_pixels", "Density Pixels", unlimited_int, 1000000.0, 100.0, 10000000.0, "Discrete grid resolution. See module-level doc."),
        param!("seed", "Seed", unlimited_int, 5000.0, 0.0, 10000.0, "JWildfire CPU-only — set `time` directly."),
        param!("time", "Time", unlimited_float, 85.5, -10000.0, 10000.0, "Animation parameter. Drives the per-step Y shift via `2 + 2 · sin(0.03 · time)`."),
        param!("n_iters", "N", unlimited_int, 100.0, 1.0, 100.0, "Inner iteration count. **GPU-clamped to ≤ 100.**"),
        param!("radio", "Radio", unlimited_float, 1.0, -10.0, 10.0, "Inversion radius squared (`rad²` in the cpp). Points outside this radius get folded inward via `r² → rad²/r²`. JWildfire calls this 'radio'; we keep the name for round-trip."),
        param!("shift_x", "Shift X", unlimited_float, -0.356, -5.0, 5.0, "Rotation angle in units of full turns (multiplied by `2π` internally). Small values produce slow rotation; 1.0 = full revolution per step. Despite the name 'Shift', this is angular, not linear."),
        param!("shift_y", "Shift Y", unlimited_float, 0.686, -5.0, 5.0, "Zoom scale: actual per-step zoom is `2 + 5 · shift_y`. JWildfire reuses the 'Shift' label."),
        param!("fr", "Red Fac.", unlimited_float, 1.8, -10.0, 10.0, "Red-channel frequency multiplier on the final `col` value: `red = cos(col · fr)`."),
        param!("fg", "Green Fac.", unlimited_float, 1.9, -10.0, 10.0, "Green-channel frequency."),
        param!("fb", "Blue Fac.", unlimited_float, 2.2, -10.0, 10.0, "Blue-channel frequency."),
        param!("gradient", "Gradient", unlimited_int, 0.0, 0.0, 1.0, "JWildfire color output mode. **Only mode 0 honored.**"),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: KALISET2_2D,
    wgsl_3d: KALISET2_3D,
};

const KALISET2_2D: &str = r#"
fn variation_glsl_kaliset2(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vrc: ptr<function, vec3<f32>>) -> vec2<f32> {
    let resolution = max(get_param(xform_id, variation_id, 0u), 100.0);
    let time = get_param(xform_id, variation_id, 2u);
    let n_iters = min(u32(get_param(xform_id, variation_id, 3u)), 100u);
    let rad2 = get_param(xform_id, variation_id, 4u);
    let shift_x = get_param(xform_id, variation_id, 5u);
    let shift_y = get_param(xform_id, variation_id, 6u);
    let fr = get_param(xform_id, variation_id, 7u);
    let fg = get_param(xform_id, variation_id, 8u);
    let fb = get_param(xform_id, variation_id, 9u);

    let i_f = floor(rng_nextf(rng) * resolution);
    let j_f = floor(rng_nextf(rng) * resolution);
    var v = vec2<f32>((i_f + 0.5) / resolution - 0.5, (j_f + 0.5) / resolution - 0.5) * 20.0;

    let two_pi = 6.283185307;
    let angle = two_pi * shift_x;
    let c_cos = cos(angle);
    let s_sin = sin(angle);
    let shift = vec2<f32>(0.0, 2.0 + 2.0 * sin(0.03 * time));
    let zoom = 2.0 + 5.0 * shift_y;
    // Guard against divide-by-zero when the user dials radio to 0.
    let rad2_safe = max(abs(rad2), 1.0e-32);

    var rsum: f32 = 0.0;
    for (var k: u32 = 0u; k < n_iters; k = k + 1u) {
        var rr = v.x * v.x + v.y * v.y;
        if (rr > rad2) {
            rr = rad2_safe / rr;
            v = v * rr;
        }
        rsum = max(rsum, rr);
        v = vec2<f32>(c_cos * v.x - s_sin * v.y, s_sin * v.x + c_cos * v.y) * zoom + shift;
    }
    let col = rsum * rsum * (500.0 / f32(n_iters) / rad2_safe);
    *vrc = vec3<f32>(cos(col * fr), cos(col * fg), cos(col * fb));
    return vec2<f32>(i_f / resolution - 0.5, j_f / resolution - 0.5);
}
"#;

const KALISET2_3D: &str = r#"
fn variation_glsl_kaliset2(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vrc: ptr<function, vec3<f32>>) -> vec3<f32> {
    let resolution = max(get_param(xform_id, variation_id, 0u), 100.0);
    let time = get_param(xform_id, variation_id, 2u);
    let n_iters = min(u32(get_param(xform_id, variation_id, 3u)), 100u);
    let rad2 = get_param(xform_id, variation_id, 4u);
    let shift_x = get_param(xform_id, variation_id, 5u);
    let shift_y = get_param(xform_id, variation_id, 6u);
    let fr = get_param(xform_id, variation_id, 7u);
    let fg = get_param(xform_id, variation_id, 8u);
    let fb = get_param(xform_id, variation_id, 9u);

    let i_f = floor(rng_nextf(rng) * resolution);
    let j_f = floor(rng_nextf(rng) * resolution);
    var v = vec2<f32>((i_f + 0.5) / resolution - 0.5, (j_f + 0.5) / resolution - 0.5) * 20.0;

    let two_pi = 6.283185307;
    let angle = two_pi * shift_x;
    let c_cos = cos(angle);
    let s_sin = sin(angle);
    let shift = vec2<f32>(0.0, 2.0 + 2.0 * sin(0.03 * time));
    let zoom = 2.0 + 5.0 * shift_y;
    let rad2_safe = max(abs(rad2), 1.0e-32);

    var rsum: f32 = 0.0;
    for (var k: u32 = 0u; k < n_iters; k = k + 1u) {
        var rr = v.x * v.x + v.y * v.y;
        if (rr > rad2) {
            rr = rad2_safe / rr;
            v = v * rr;
        }
        rsum = max(rsum, rr);
        v = vec2<f32>(c_cos * v.x - s_sin * v.y, s_sin * v.x + c_cos * v.y) * zoom + shift;
    }
    let col = rsum * rsum * (500.0 / f32(n_iters) / rad2_safe);
    *vrc = vec3<f32>(cos(col * fr), cos(col * fg), cos(col * fb));
    return vec3<f32>(i_f / resolution - 0.5, j_f / resolution - 0.5, p.z);
}
"#;

// =============================================================================
// glsl_apollonian
// =============================================================================

/// Apollonian gasket fractal — sphere packing pattern.
///
/// **Implementation note**: JWildfire's source delegates to `js.glsl.G.rot`
/// (Euler-angle rotation matrix from a `vec3`) and `js.glsl.G.app`
/// (Apollonian gasket iterator). Neither helper is in the source
/// snapshot we have. We use:
///   - `G.rot(a)` → ZYX Euler rotation matrix
///   - `G.app(p, k, m)` → the canonical Iñigo Quílez Apollonian fold:
///     8 iterations of `p = -1 + 2·fract(0.5 + 0.5·p); s = k/dot(p,p);
///     p *= s`, with `m * p` as the initial transform
///
/// These are the standard shadertoy formulas and produce visibly
/// Apollonian-like output, but bit-for-bit visual identity with
/// JWildfire would need the actual `js.glsl.G` source. If a future
/// audit finds JWildfire's helpers differ, the bodies here are the
/// only thing to change — the framework integration is fine.
///
/// 4 params: `density_pixels`, `seed` (CPU-only), `time`, `gradient`.
/// 
/// # Authors
/// - Jesus Sosa
pub static GLSL_APOLLONIAN: VariationDef = VariationDef {
    name: "glsl_apollonian",
    aliases: &[],
    display_name: "GLSL Apollonian",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::WritesRgb, Feature::NeverZ],
    parameters: &[
        param!("density_pixels", "Density Pixels", unlimited_int, 1000000.0, 100.0, 10000000.0, "Discrete grid resolution."),
        param!("seed", "Seed", unlimited_int, 10000.0, 0.0, 10000.0, "JWildfire CPU-only — set `time` directly."),
        param!("time", "Time", unlimited_float, 0.0, -10000.0, 10000.0, "Animation parameter. Drives the rotation matrix (Euler angles `t + (1,2,3)`), the fold factor `k = 1.2 + 0.1·sin(0.1·time)`, and the initial scale `f1 = 2 + 0.25·sin(0.3·time)`."),
        param!("gradient", "Gradient", unlimited_int, 0.0, 0.0, 1.0, "JWildfire color output mode. **Only mode 0 honored.**"),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: APOLLONIAN_2D,
    wgsl_3d: APOLLONIAN_3D,
};

// Helpers + body. `apollonian_rot` and `apollonian_fold` are defined
// once; the variation calls them. The shader-builder's per-fn dedupe
// ensures they're only emitted in the compiled WGSL once even though
// they appear twice (2D + 3D bodies).
//
// Per call: 1 rotation matrix construction + 8 fold iterations × ~10
// ops ≈ ~100 ops. Cheap. No clamps needed.
const APOLLONIAN_2D: &str = r#"
fn apollonian_rot(a: vec3<f32>) -> mat3x3<f32> {
    // ZYX Euler rotation (standard shadertoy convention). Column-major
    // (WGSL mat3x3 is column-major).
    let sx = sin(a.x); let cx = cos(a.x);
    let sy = sin(a.y); let cy = cos(a.y);
    let sz = sin(a.z); let cz = cos(a.z);
    return mat3x3<f32>(
        vec3<f32>(cy * cz,                 cy * sz,                 -sy),
        vec3<f32>(sx * sy * cz - cx * sz,  sx * sy * sz + cx * cz,  sx * cy),
        vec3<f32>(cx * sy * cz + sx * sz,  cx * sy * sz - sx * cz,  cx * cy),
    );
}

fn apollonian_fold(p_in: vec3<f32>, k: f32, m: mat3x3<f32>) -> vec3<f32> {
    var pp = m * p_in;
    for (var i: u32 = 0u; i < 8u; i = i + 1u) {
        pp = -1.0 + 2.0 * fract(0.5 + 0.5 * pp);
        let r2 = max(dot(pp, pp), 1.0e-32);
        pp = pp * (k / r2);
    }
    return pp;
}

fn variation_glsl_apollonian(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vrc: ptr<function, vec3<f32>>) -> vec2<f32> {
    let resolution = max(get_param(xform_id, variation_id, 0u), 100.0);
    let time = get_param(xform_id, variation_id, 2u);

    let i_f = floor(rng_nextf(rng) * resolution);
    let j_f = floor(rng_nextf(rng) * resolution);
    let x = i_f + 0.5;
    let y = j_f + 0.5;
    // Map to [-1, 1]² as in the cpp.
    let uv = vec2<f32>(2.0 * x / resolution - 1.0, 2.0 * y / resolution - 1.0);

    let t = 0.05 * time;
    let m = apollonian_rot(vec3<f32>(t + 1.0, t + 2.0, t + 3.0));
    let k = 1.2 + 0.1 * sin(0.1 * time);
    let f1 = 2.0 + 0.25 * sin(0.3 * time);
    let v2 = uv * 2.0;
    let v = m * vec3<f32>(v2.x, v2.y, 0.0) * f1;
    let v3 = sin(apollonian_fold(v, k, m));
    *vrc = v3 * 0.6 + 0.5;

    return vec2<f32>(i_f / resolution - 0.5, j_f / resolution - 0.5);
}
"#;

const APOLLONIAN_3D: &str = r#"
fn apollonian_rot(a: vec3<f32>) -> mat3x3<f32> {
    // ZYX Euler rotation (standard shadertoy convention). Column-major
    // (WGSL mat3x3 is column-major).
    let sx = sin(a.x); let cx = cos(a.x);
    let sy = sin(a.y); let cy = cos(a.y);
    let sz = sin(a.z); let cz = cos(a.z);
    return mat3x3<f32>(
        vec3<f32>(cy * cz,                 cy * sz,                 -sy),
        vec3<f32>(sx * sy * cz - cx * sz,  sx * sy * sz + cx * cz,  sx * cy),
        vec3<f32>(cx * sy * cz + sx * sz,  cx * sy * sz - sx * cz,  cx * cy),
    );
}

fn apollonian_fold(p_in: vec3<f32>, k: f32, m: mat3x3<f32>) -> vec3<f32> {
    var pp = m * p_in;
    for (var i: u32 = 0u; i < 8u; i = i + 1u) {
        pp = -1.0 + 2.0 * fract(0.5 + 0.5 * pp);
        let r2 = max(dot(pp, pp), 1.0e-32);
        pp = pp * (k / r2);
    }
    return pp;
}

fn variation_glsl_apollonian(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vrc: ptr<function, vec3<f32>>) -> vec3<f32> {
    let resolution = max(get_param(xform_id, variation_id, 0u), 100.0);
    let time = get_param(xform_id, variation_id, 2u);
    let i_f = floor(rng_nextf(rng) * resolution);
    let j_f = floor(rng_nextf(rng) * resolution);
    let uv = vec2<f32>(2.0 * (i_f + 0.5) / resolution - 1.0, 2.0 * (j_f + 0.5) / resolution - 1.0);

    let t = 0.05 * time;
    let m = apollonian_rot(vec3<f32>(t + 1.0, t + 2.0, t + 3.0));
    let k = 1.2 + 0.1 * sin(0.1 * time);
    let f1 = 2.0 + 0.25 * sin(0.3 * time);
    let v2 = uv * 2.0;
    let v = m * vec3<f32>(v2.x, v2.y, 0.0) * f1;
    let v3 = sin(apollonian_fold(v, k, m));
    *vrc = v3 * 0.6 + 0.5;
    return vec3<f32>(i_f / resolution - 0.5, j_f / resolution - 0.5, p.z);
}
"#;
