//! `glsl_*` family — procedural fields + raymarch subset (6 variations).
//!
//! Final themed group of the `glsl_*` family (after [`glsl_fractals`]
//! and [`glsl_tilings`]). Same call shape as the siblings — see those
//! modules for the shared semantics.
//!
//! **Deferred from this batch**: `glsl_randomoctree`. JWildfire's
//! source is a variable-depth octree raymarcher (~400 lines, recursive
//! voxel subdivision, multiple per-step test paths). The cost is hard
//! to bound — 100-step outer loop with intricate inner work that
//! easily blows the TDR budget when multiplied by 32K threads × 256
//! chaos iters. Worth porting separately with a careful cost model
//! and per-step clamps.
//!
//! **`glsl_circuits` divergence**: JWildfire's source uses a class-
//! level mutable `double S` field that accumulates across every call
//! to the variation's `formula()` method. On JVM this is broken
//! multithread semantics (no synchronization, written from every
//! pixel sample). On GPU each thread has isolated stack, so we
//! cannot replicate cross-thread accumulation even if we wanted to.
//! Our port uses a per-call local `S`, which is deterministic and
//! well-defined. Visual output may differ from JWildfire because of
//! the lost accumulation; the algorithm is the same.
//!
//! Sources:
//! - [`GLSLAcrilicFunc.java`](../../../output/variation-jwf-source/GLSLAcrilicFunc.java)
//! - [`GLSLCirclesBlueFunc.java`](../../../output/variation-jwf-source/GLSLCirclesBlueFunc.java)
//! - [`GLSLCircuitsFunc.java`](../../../output/variation-jwf-source/GLSLCircuitsFunc.java)
//! - [`GLSLFractalDotsFunc.java`](../../../output/variation-jwf-source/GLSLFractalDotsFunc.java)
//! - [`GLSLStarsFieldFunc.java`](../../../output/variation-jwf-source/GLSLStarsFieldFunc.java)
//! - [`GLSLGrid3DFunc.java`](../../../output/variation-jwf-source/GLSLGrid3DFunc.java)
//!
//! # Authors
//! - Jesus Sosa (ports, 2018)
//! - kabuto (original `glsl_grid3D` raymarching shadertoy)

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// glsl_acrilic
// =============================================================================

/// Acrylic-style smudge pattern. Three RGB channels each iterate
/// `(p.x, p.y)` through trig perturbations driven by params p1..p4,
/// then final color comes from `sin(p5·p.x²) + sin(p6·p.y²)` mapped
/// through `cos(col · F*)`.
///
/// 13 params: standard 4 + 6 perturbation knobs (p1..p6) + 3 color
/// frequencies (fr/fg/fb).
/// 
/// # Authors
/// - Jesus Sosa
pub static GLSL_ACRILIC: VariationDef = VariationDef {
    name: "glsl_acrilic",
    aliases: &[],
    display_name: "GLSL Acrilic",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::WritesRgb, Feature::NeverZ],
    parameters: &[
        param!("density_pixels", "Density Pixels", unlimited_int, 1000000.0, 100.0, 10000000.0, "Discrete grid resolution. See module-level doc."),
        param!("seed", "Seed", unlimited_int, 10000.0, 0.0, 10000.0, "JWildfire CPU-only — set `time` directly."),
        param!("time", "Time", unlimited_float, 0.0, -10000.0, 10000.0, "Animation parameter. Embedded into every perturbation step's sin/cos arguments."),
        param!("steps", "Steps", unlimited_int, 30.0, 1.0, 100.0, "Inner perturbation iteration count. **GPU-clamped to ≤ 100.** Higher = more swirling distortion."),
        param!("p1", "p1", unlimited_float, 1.0, -10.0, 10.0, "Perturbation knob 1. Frequency multiplier on `p.y` in the X-update's sin term."),
        param!("p2", "p2", unlimited_float, 1.0, -10.0, 10.0, "Perturbation knob 2. Inverse-time scale (`time / (p2·i)`). p2 = 0 blows up — set away from 0."),
        param!("p3", "p3", unlimited_float, 1.0, -10.0, 10.0, "Perturbation knob 3. Frequency multiplier on `p.x` in the Y-update's cos term."),
        param!("p4", "p4", unlimited_float, 1.0, -10.0, 10.0, "Perturbation knob 4. Inverse-time scale in Y-update."),
        param!("p5", "p5", unlimited_float, 1.0, -10.0, 10.0, "Color formula knob 5. Final per-channel value uses `sin(p5 · p.x²)`."),
        param!("p6", "p6", unlimited_float, 1.0, -10.0, 10.0, "Color formula knob 6. `sin(p6 · p.y²)`."),
        param!("fr", "Red Fac.", unlimited_float, 1.0, -10.0, 10.0, "Red-channel final cosine frequency."),
        param!("fg", "Green Fac.", unlimited_float, 1.0, -10.0, 10.0, "Green-channel frequency."),
        param!("fb", "Blue Fac.", unlimited_float, 1.0, -10.0, 10.0, "Blue-channel frequency."),
        param!("gradient", "Gradient", unlimited_int, 0.0, 0.0, 1.0, "JWildfire color output mode. **Only mode 0 honored.**"),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: ACRILIC_2D,
    wgsl_3d: ACRILIC_3D,
};

// Per call: 3 × steps × ~10 ops. With steps=30 default → ~900 ops.
// Within TDR.
const ACRILIC_2D: &str = r#"
fn variation_glsl_acrilic(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vrc: ptr<function, vec3<f32>>) -> vec2<f32> {
    let resolution = max(get_param(xform_id, variation_id, 0u), 100.0);
    let time = get_param(xform_id, variation_id, 2u);
    let steps = min(u32(get_param(xform_id, variation_id, 3u)), 100u);
    let p1 = get_param(xform_id, variation_id, 4u);
    let p2_raw = get_param(xform_id, variation_id, 5u);
    let p3 = get_param(xform_id, variation_id, 6u);
    let p4_raw = get_param(xform_id, variation_id, 7u);
    let p5 = get_param(xform_id, variation_id, 8u);
    let p6 = get_param(xform_id, variation_id, 9u);
    let fr = get_param(xform_id, variation_id, 10u);
    let fg = get_param(xform_id, variation_id, 11u);
    let fb = get_param(xform_id, variation_id, 12u);

    let p2 = sign(p2_raw) * max(abs(p2_raw), 1.0e-6);
    let p4 = sign(p4_raw) * max(abs(p4_raw), 1.0e-6);

    let i_f = floor(rng_nextf(rng) * resolution);
    let j_f = floor(rng_nextf(rng) * resolution);

    var col = vec3<f32>(0.0);
    for (var j: u32 = 0u; j < 3u; j = j + 1u) {
        var pp = vec2<f32>(0.7 * (i_f + 0.5) / resolution, 0.7 * (j_f + 0.5) / resolution);
        let jf = f32(j);
        for (var i: u32 = 1u; i < steps; i = i + 1u) {
            let ife = f32(i);
            let denom = ife + jf;
            pp.x = pp.x + 0.1 / denom * sin(ife * p1 * pp.y + time + cos((time / (p2 * ife)) * ife + jf));
            pp.y = pp.y + 0.1 / denom * cos(ife * p3 * pp.x + time + sin((time / (p4 * ife)) * ife + jf));
        }
        let v = sin(p5 * pp.x * pp.x) + sin(p6 * pp.y * pp.y);
        if (j == 0u) { col.x = v; }
        else if (j == 1u) { col.y = v; }
        else { col.z = v; }
    }
    *vrc = vec3<f32>(cos(col.x * fr), cos(col.y * fg), cos(col.z * fb));
    return vec2<f32>(i_f / resolution - 0.5, j_f / resolution - 0.5);
}
"#;

const ACRILIC_3D: &str = r#"
fn variation_glsl_acrilic(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vrc: ptr<function, vec3<f32>>) -> vec3<f32> {
    let resolution = max(get_param(xform_id, variation_id, 0u), 100.0);
    let time = get_param(xform_id, variation_id, 2u);
    let steps = min(u32(get_param(xform_id, variation_id, 3u)), 100u);
    let p1 = get_param(xform_id, variation_id, 4u);
    let p2_raw = get_param(xform_id, variation_id, 5u);
    let p3 = get_param(xform_id, variation_id, 6u);
    let p4_raw = get_param(xform_id, variation_id, 7u);
    let p5 = get_param(xform_id, variation_id, 8u);
    let p6 = get_param(xform_id, variation_id, 9u);
    let fr = get_param(xform_id, variation_id, 10u);
    let fg = get_param(xform_id, variation_id, 11u);
    let fb = get_param(xform_id, variation_id, 12u);
    let p2 = sign(p2_raw) * max(abs(p2_raw), 1.0e-6);
    let p4 = sign(p4_raw) * max(abs(p4_raw), 1.0e-6);
    let i_f = floor(rng_nextf(rng) * resolution);
    let j_f = floor(rng_nextf(rng) * resolution);
    var col = vec3<f32>(0.0);
    for (var j: u32 = 0u; j < 3u; j = j + 1u) {
        var pp = vec2<f32>(0.7 * (i_f + 0.5) / resolution, 0.7 * (j_f + 0.5) / resolution);
        let jf = f32(j);
        for (var i: u32 = 1u; i < steps; i = i + 1u) {
            let ife = f32(i);
            let denom = ife + jf;
            pp.x = pp.x + 0.1 / denom * sin(ife * p1 * pp.y + time + cos((time / (p2 * ife)) * ife + jf));
            pp.y = pp.y + 0.1 / denom * cos(ife * p3 * pp.x + time + sin((time / (p4 * ife)) * ife + jf));
        }
        let v = sin(p5 * pp.x * pp.x) + sin(p6 * pp.y * pp.y);
        if (j == 0u) { col.x = v; }
        else if (j == 1u) { col.y = v; }
        else { col.z = v; }
    }
    *vrc = vec3<f32>(cos(col.x * fr), cos(col.y * fg), cos(col.z * fb));
    return vec3<f32>(i_f / resolution - 0.5, j_f / resolution - 0.5, p.z);
}
"#;

// =============================================================================
// glsl_circlesblue
// =============================================================================

/// Animated bubble field. Renders `bubles` circular bubbles with
/// per-bubble pseudo-random position, size, and color, all driven by
/// `time`. Bubbles fall from the top of the unit square with vertical
/// scrolling.
///
/// 6 params: standard 4 (sans gradient — `seed` and `time` are
/// included) + `radius` (bubble base radius) + `bubles` (count).
/// 
/// # Authors
/// - Jesus Sosa
pub static GLSL_CIRCLESBLUE: VariationDef = VariationDef {
    name: "glsl_circlesblue",
    aliases: &[],
    display_name: "GLSL CirclesBlue",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::WritesRgb, Feature::NeverZ],
    parameters: &[
        param!("density_pixels", "Density Pixels", unlimited_int, 1000000.0, 100.0, 10000000.0, "Discrete grid resolution."),
        param!("seed", "Seed", unlimited_int, 10000.0, 0.0, 10000.0, "JWildfire CPU-only — set `time` directly."),
        param!("time", "Time", unlimited_float, 1.0, -10000.0, 10000.0, "Animation parameter. Drives the vertical scrolling (`time/5 · 0.1` per step) and the per-bubble color/size modulation."),
        param!("radius", "Radius", unlimited_float, 0.04, 0.0, 1.0, "Base bubble radius — each bubble's actual radius adds `sin(i)·0.12 + 0.08` on top. JWildfire's UI label is 'Radiusy' (typo in source kept for round-trip)."),
        param!("bubles", "Bubles", unlimited_int, 40.0, 1.0, 200.0, "Bubble count. **GPU-clamped to ≤ 200.** JWildfire's UI label has the typo 'Bubles'; kept for round-trip."),
        param!("gradient", "Gradient", unlimited_int, 0.0, 0.0, 1.0, "JWildfire color output mode. **Only mode 0 honored.**"),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: CIRCLESBLUE_2D,
    wgsl_3d: CIRCLESBLUE_3D,
};

const CIRCLESBLUE_2D: &str = r#"
fn variation_glsl_circlesblue(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vrc: ptr<function, vec3<f32>>) -> vec2<f32> {
    let resolution = max(get_param(xform_id, variation_id, 0u), 100.0);
    let time = get_param(xform_id, variation_id, 2u);
    let f_radius = get_param(xform_id, variation_id, 3u);
    let bubles = min(u32(get_param(xform_id, variation_id, 4u)), 200u);

    let i_f = floor(rng_nextf(rng) * resolution);
    let j_f = floor(rng_nextf(rng) * resolution);
    let uv = vec2<f32>(2.0 * (i_f + 0.5) / resolution - 1.0, 2.0 * (j_f + 0.5) / resolution - 1.0);

    var color = vec3<f32>(0.0);
    let col_a = vec3<f32>(0.1, 0.2, 0.8);
    let col_b = vec3<f32>(0.2, 0.8, 0.6);
    for (var i: u32 = 0u; i < bubles; i = i + 1u) {
        let i_fl = f32(i);
        let pha = tan(i_fl * 6.0 + 1.0) * 0.5 + 0.5;
        let siz = pow(cos(i_fl * 2.4 + 5.0) * 0.5 + 0.5, 4.0);
        let pox = cos(i_fl * 3.55 + 4.1);
        let rad = f_radius + sin(i_fl) * 0.12 + 0.08;
        let scroll = pha + 0.1 * (time / 5.0) * (0.2 + 0.8 * siz);
        let scroll_mod = scroll - floor(scroll);
        let pos = vec2<f32>(pox + sin(time / 15.0 + pha + siz), -1.0 - rad + (2.0 + 2.0 * rad) * scroll_mod);
        let dis = length(uv - pos);
        let col = mix(col_a, col_b, vec3<f32>(0.5 + 0.5 * sin(i_fl * sin(time * pox * 0.03) + 1.9)));
        color = color + col * (1.0 - smoothstep(rad * (0.65 + 0.20 * sin(pox * time)), rad, dis)) * (1.0 - cos(pox * time));
    }
    *vrc = color * 0.3;
    return vec2<f32>(i_f / resolution - 0.5, j_f / resolution - 0.5);
}
"#;

const CIRCLESBLUE_3D: &str = r#"
fn variation_glsl_circlesblue(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vrc: ptr<function, vec3<f32>>) -> vec3<f32> {
    let resolution = max(get_param(xform_id, variation_id, 0u), 100.0);
    let time = get_param(xform_id, variation_id, 2u);
    let f_radius = get_param(xform_id, variation_id, 3u);
    let bubles = min(u32(get_param(xform_id, variation_id, 4u)), 200u);
    let i_f = floor(rng_nextf(rng) * resolution);
    let j_f = floor(rng_nextf(rng) * resolution);
    let uv = vec2<f32>(2.0 * (i_f + 0.5) / resolution - 1.0, 2.0 * (j_f + 0.5) / resolution - 1.0);
    var color = vec3<f32>(0.0);
    let col_a = vec3<f32>(0.1, 0.2, 0.8);
    let col_b = vec3<f32>(0.2, 0.8, 0.6);
    for (var i: u32 = 0u; i < bubles; i = i + 1u) {
        let i_fl = f32(i);
        let pha = tan(i_fl * 6.0 + 1.0) * 0.5 + 0.5;
        let siz = pow(cos(i_fl * 2.4 + 5.0) * 0.5 + 0.5, 4.0);
        let pox = cos(i_fl * 3.55 + 4.1);
        let rad = f_radius + sin(i_fl) * 0.12 + 0.08;
        let scroll = pha + 0.1 * (time / 5.0) * (0.2 + 0.8 * siz);
        let scroll_mod = scroll - floor(scroll);
        let pos = vec2<f32>(pox + sin(time / 15.0 + pha + siz), -1.0 - rad + (2.0 + 2.0 * rad) * scroll_mod);
        let dis = length(uv - pos);
        let col = mix(col_a, col_b, vec3<f32>(0.5 + 0.5 * sin(i_fl * sin(time * pox * 0.03) + 1.9)));
        color = color + col * (1.0 - smoothstep(rad * (0.65 + 0.20 * sin(pox * time)), rad, dis)) * (1.0 - cos(pox * time));
    }
    *vrc = color * 0.3;
    return vec3<f32>(i_f / resolution - 0.5, j_f / resolution - 0.5, p.z);
}
"#;

// =============================================================================
// glsl_circuits
// =============================================================================

/// Circuit-board-style fractal pattern. Layered fractal-fold formula
/// applied 1..24 times (per `loops`) and accumulated.
///
/// **Divergence from JWildfire**: JWildfire's `formula` writes to a
/// class-level `double S` field that persists across every call to
/// every transform's variation (across millions of chaos-game iters).
/// This is broken multithread semantics — `S` is initialized once at
/// flame load and grows without bound. Our port uses a per-call local
/// `S` so the algorithm is well-defined; visual output may differ
/// from JWildfire because the global accumulation is not replicated
/// (and cannot be on GPU).
///
/// 9 params: standard 4 (sans seed — none in source) + `rate`,
/// `intensity`, `focus`, `pulse`, `glow`, `loops`, `zoom`.
/// 
/// # Authors
/// - Jesus Sosa
pub static GLSL_CIRCUITS: VariationDef = VariationDef {
    name: "glsl_circuits",
    aliases: &[],
    display_name: "GLSL Circuits",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::WritesRgb, Feature::NeverZ],
    parameters: &[
        param!("density_pixels", "Density Pixels", unlimited_int, 1000000.0, 100.0, 10000000.0, "Discrete grid resolution."),
        param!("time", "Time", unlimited_float, 0.0, -10000.0, 10000.0, "Animation parameter. Drives the per-iteration rotation matrices and the circuit phase."),
        param!("rate", "Rate", unlimited_float, 0.8, 0.0, 5.0, "Time scaling. Multiplies `time · 0.01` to derive the inner phase N."),
        param!("intensity", "Intensity", unlimited_float, 0.9, 0.0, 5.0, "Brightness scalar applied to the per-formula color mix."),
        param!("focus", "Focus", unlimited_float, 0.6, 0.0, 5.0, "Depth-of-field-like blur. Multiplies the per-sample pixel offset; higher = more visible per-pixel jitter."),
        param!("pulse", "Pulse", unlimited_float, 1.5, 0.0, 10.0, "Color cycle rate. Multiplies the time term in the per-iteration color mod."),
        param!("glow", "Glow", unlimited_float, 0.0, 0.0, 5.0, "Initial value of the local `S` accumulator (`S₀ = (101 + glow) · intensity`)."),
        param!("loops", "Loops", unlimited_int, 1.0, 1.0, 10.0, "Outer (anti-alias) loop count. **GPU-clamped to ≤ 10.** Inner `formula` loop count also follows: each AA sample re-runs the 11-iter formula with K = floor(loops/4 + floor(5·zoom)) inner iters."),
        param!("zoom", "Zoom", unlimited_float, 0.0, -1.0, 1.0, "Zoom factor. Also gates the inner formula loop count (`K = floor(loops/4 + floor(5·zoom))`)."),
        param!("gradient", "Gradient", unlimited_int, 0.0, 0.0, 1.0, "JWildfire color output mode. **Only mode 0 honored.**"),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: CIRCUITS_2D,
    wgsl_3d: CIRCUITS_3D,
};

// Cost: loops (≤ 10) × 11 = ~110 inner iters × ~25 ops = ~2750 ops per call.
// Well within TDR. Original cap was 24 AA samples × 11 formula iters which
// would have been ~265 inner iters. We clamp loops to ≤ 10 instead because
// the visual quality stops improving past ~5 and the JWF cap of `loops`
// being a tunable knob (not a hard cap on AA samples) was misleading.
const CIRCUITS_2D: &str = r#"
fn circuits_formula(z_in: vec2<f32>, t: f32, intensity: f32, zoom: f32, pulse: f32, loops: f32, s_accum_in: ptr<function, f32>) -> vec3<f32> {
    var z = z_in;
    var m: f32 = 0.0;
    var o: f32 = 0.0;
    var ot: f32 = 1000.0;
    var ot2: f32 = 1000.0;
    var k = floor(loops / 4.0 + floor(5.0 * zoom));
    var color = vec3<f32>(0.0);
    for (var i: u32 = 0u; i < 11u; i = i + 1u) {
        let d = dot(z, z);
        let c_d = clamp(d, 0.1, 0.5);
        z = abs(z) / c_d - t;
        let l = length(z);
        o = min(max(abs(min(z.x, z.y)), -l + 0.25), abs(l - 0.25));
        ot = min(ot, o);
        ot2 = min(l * 0.1, ot2);
        m = max(m, f32(i) * (1.0 - abs(sign(ot - o))));
        if (k <= 0.0) { break; }
        k = k - 1.0;
    }
    m = m + 1.0;
    let w = (intensity * zoom) * m;
    let safe_w = sign(w) * max(abs(w), 1.0e-32);
    let circ = pow(max(0.0, w - ot2) / safe_w, 6.0);
    *s_accum_in = *s_accum_in + max(pow(max(0.0, w - ot) / safe_w, 0.25), circ);
    let t1 = vec3<f32>(0.1) + vec3<f32>(0.45, 0.75, m * 0.1);
    let col_n = normalize(t1);
    let arg = m / 9.0 - t * pulse + ot2 * 2.0;
    let arg_mod = arg - floor(arg);
    let t2 = vec3<f32>(0.4 + arg_mod);
    color = color + col_n * t2;
    let f1 = circ * (10.0 - m) * 3.0;
    return color + vec3<f32>(1.0, 0.7, 0.3) * f1;
}

fn variation_glsl_circuits(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vrc: ptr<function, vec3<f32>>) -> vec2<f32> {
    let resolution = max(get_param(xform_id, variation_id, 0u), 100.0);
    let time = get_param(xform_id, variation_id, 1u);
    let rate = get_param(xform_id, variation_id, 2u);
    let intensity = get_param(xform_id, variation_id, 3u);
    let focus = get_param(xform_id, variation_id, 4u);
    let pulse = get_param(xform_id, variation_id, 5u);
    let glow = get_param(xform_id, variation_id, 6u);
    let loops_f = max(get_param(xform_id, variation_id, 7u), 1.0);
    let loops = min(u32(loops_f), 10u);
    let zoom = get_param(xform_id, variation_id, 8u);

    let i_f = floor(rng_nextf(rng) * resolution);
    let j_f = floor(rng_nextf(rng) * resolution);
    let pos = vec2<f32>((i_f + 0.5) / resolution - 0.5, (j_f + 0.5) / resolution - 0.5);

    // Time phase computation (mirrors JWF's branching).
    var n = time * 0.01 * rate;
    var r: f32 = 0.0;
    if (n > 6.0 * rate) { r = r + 1.0; n = n - r * 8.0 * rate; }
    var t: f32 = 2.0 * rate;
    if (n < 4.0 * rate) { t = t + n; } else { t = 8.0 * rate - n; }
    let z_param = 1.05 - zoom;

    var uv = pos;
    let sph_raw = length(uv) * 0.1;
    let sph = sqrt(max(1.0 - sph_raw * sph_raw, 0.0)) * 2.0;
    let safe_sph = max(sph, 1.0e-32);
    let a = t * 3.14159265;
    let b = a + t;
    let c = cos(a) + sin(b);
    // 2D rotation matrices (mat2 from JWF, but treating uv as vec2).
    let cb = cos(b); let sb = sin(b);
    let ca = cos(a); let sa = sin(a);
    uv = vec2<f32>(uv.x * cb - uv.y * sb, uv.x * sb + uv.y * cb);
    uv = vec2<f32>(uv.x * ca + uv.y * sa, -uv.x * sa + uv.y * ca);
    uv = uv - vec2<f32>(sin(c), cos(c)) / 3.14159265;
    uv = uv * z_param;
    let pix = 0.5 / resolution * z_param / safe_sph;
    let dof = (zoom * focus) + (t * 0.25);

    var color = vec3<f32>(0.0);
    var s_accum: f32 = (101.0 + glow) * intensity;  // local per-call init
    for (var aa: u32 = 0u; aa < loops; aa = aa + 1u) {
        let aa_uv = vec2<f32>(floor(f32(aa) / 6.0), f32(aa) - 6.0 * floor(f32(aa) / 6.0));
        color = circuits_formula(uv + aa_uv * pix * dof, t, intensity, zoom, pulse, loops_f, &s_accum);
    }
    let safe_loops = max(f32(loops), 1.0);
    s_accum = s_accum / safe_loops;
    color = color / safe_loops;
    let mixed = mix(vec3<f32>(0.15), color, vec3<f32>(s_accum)) * (1.0 - length(pos));
    *vrc = mixed * vec3<f32>(1.2, 1.1, 1.0);
    return vec2<f32>(i_f / resolution - 0.5, j_f / resolution - 0.5);
}
"#;

const CIRCUITS_3D: &str = r#"
fn circuits_formula(z_in: vec2<f32>, t: f32, intensity: f32, zoom: f32, pulse: f32, loops: f32, s_accum_in: ptr<function, f32>) -> vec3<f32> {
    var z = z_in;
    var m: f32 = 0.0;
    var o: f32 = 0.0;
    var ot: f32 = 1000.0;
    var ot2: f32 = 1000.0;
    var k = floor(loops / 4.0 + floor(5.0 * zoom));
    var color = vec3<f32>(0.0);
    for (var i: u32 = 0u; i < 11u; i = i + 1u) {
        let d = dot(z, z);
        let c_d = clamp(d, 0.1, 0.5);
        z = abs(z) / c_d - t;
        let l = length(z);
        o = min(max(abs(min(z.x, z.y)), -l + 0.25), abs(l - 0.25));
        ot = min(ot, o);
        ot2 = min(l * 0.1, ot2);
        m = max(m, f32(i) * (1.0 - abs(sign(ot - o))));
        if (k <= 0.0) { break; }
        k = k - 1.0;
    }
    m = m + 1.0;
    let w = (intensity * zoom) * m;
    let safe_w = sign(w) * max(abs(w), 1.0e-32);
    let circ = pow(max(0.0, w - ot2) / safe_w, 6.0);
    *s_accum_in = *s_accum_in + max(pow(max(0.0, w - ot) / safe_w, 0.25), circ);
    let t1 = vec3<f32>(0.1) + vec3<f32>(0.45, 0.75, m * 0.1);
    let col_n = normalize(t1);
    let arg = m / 9.0 - t * pulse + ot2 * 2.0;
    let arg_mod = arg - floor(arg);
    let t2 = vec3<f32>(0.4 + arg_mod);
    color = color + col_n * t2;
    let f1 = circ * (10.0 - m) * 3.0;
    return color + vec3<f32>(1.0, 0.7, 0.3) * f1;
}

fn variation_glsl_circuits(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vrc: ptr<function, vec3<f32>>) -> vec3<f32> {
    let resolution = max(get_param(xform_id, variation_id, 0u), 100.0);
    let time = get_param(xform_id, variation_id, 1u);
    let rate = get_param(xform_id, variation_id, 2u);
    let intensity = get_param(xform_id, variation_id, 3u);
    let focus = get_param(xform_id, variation_id, 4u);
    let pulse = get_param(xform_id, variation_id, 5u);
    let glow = get_param(xform_id, variation_id, 6u);
    let loops_f = max(get_param(xform_id, variation_id, 7u), 1.0);
    let loops = min(u32(loops_f), 10u);
    let zoom = get_param(xform_id, variation_id, 8u);
    let i_f = floor(rng_nextf(rng) * resolution);
    let j_f = floor(rng_nextf(rng) * resolution);
    let pos = vec2<f32>((i_f + 0.5) / resolution - 0.5, (j_f + 0.5) / resolution - 0.5);
    var n = time * 0.01 * rate;
    var r: f32 = 0.0;
    if (n > 6.0 * rate) { r = r + 1.0; n = n - r * 8.0 * rate; }
    var t: f32 = 2.0 * rate;
    if (n < 4.0 * rate) { t = t + n; } else { t = 8.0 * rate - n; }
    let z_param = 1.05 - zoom;
    var uv = pos;
    let sph_raw = length(uv) * 0.1;
    let sph = sqrt(max(1.0 - sph_raw * sph_raw, 0.0)) * 2.0;
    let safe_sph = max(sph, 1.0e-32);
    let a = t * 3.14159265;
    let b = a + t;
    let c = cos(a) + sin(b);
    let cb = cos(b); let sb = sin(b);
    let ca = cos(a); let sa = sin(a);
    uv = vec2<f32>(uv.x * cb - uv.y * sb, uv.x * sb + uv.y * cb);
    uv = vec2<f32>(uv.x * ca + uv.y * sa, -uv.x * sa + uv.y * ca);
    uv = uv - vec2<f32>(sin(c), cos(c)) / 3.14159265;
    uv = uv * z_param;
    let pix = 0.5 / resolution * z_param / safe_sph;
    let dof = (zoom * focus) + (t * 0.25);
    var color = vec3<f32>(0.0);
    var s_accum: f32 = (101.0 + glow) * intensity;
    for (var aa: u32 = 0u; aa < loops; aa = aa + 1u) {
        let aa_uv = vec2<f32>(floor(f32(aa) / 6.0), f32(aa) - 6.0 * floor(f32(aa) / 6.0));
        color = circuits_formula(uv + aa_uv * pix * dof, t, intensity, zoom, pulse, loops_f, &s_accum);
    }
    let safe_loops = max(f32(loops), 1.0);
    s_accum = s_accum / safe_loops;
    color = color / safe_loops;
    let mixed = mix(vec3<f32>(0.15), color, vec3<f32>(s_accum)) * (1.0 - length(pos));
    *vrc = mixed * vec3<f32>(1.2, 1.1, 1.0);
    return vec3<f32>(i_f / resolution - 0.5, j_f / resolution - 0.5, p.z);
}
"#;

// =============================================================================
// glsl_fractaldots
// =============================================================================

/// Sierpinski-like fractal dot pattern. Applies `iterations` fold +
/// rotate + shrink steps to the sample point, then draws a small dot
/// if the final point is inside a circle of `circleSize = dotsize / (3 · 2^maxiter)`.
///
/// 10 params (no `seed` or `time`).
/// 
/// # Authors
/// - Jesus Sosa
pub static GLSL_FRACTALDOTS: VariationDef = VariationDef {
    name: "glsl_fractaldots",
    aliases: &[],
    display_name: "GLSL FractalDots",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::WritesRgb, Feature::NeverZ],
    parameters: &[
        param!("density_pixels", "Density Pixels", unlimited_int, 1000000.0, 100.0, 10000000.0, "Discrete grid resolution."),
        param!("iterations", "Iterations", unlimited_int, 9.0, 1.0, 20.0, "Soft early-exit threshold for the inner loop. The loop runs at most `max_iterations` times but breaks when `i > iterations`. **GPU-clamped to ≤ 20**."),
        param!("dot_size", "Dot Size", unlimited_float, 400.0, 1.0, 1000.0, "Numerator of the circle-test radius: `circleSize = dot_size / (3 · 2^max_iterations)`. Higher = larger visible dots."),
        param!("max_iterations", "Max Iterations", unlimited_int, 5.0, 1.0, 20.0, "Outer iteration cap and exponent in `circleSize`. **GPU-clamped to ≤ 20**."),
        param!("complexity", "Complexity", unlimited_float, 0.5, -5.0, 5.0, "Per-iteration translation vector (`uv -= vec2(complexity)`). Negative values move detail inward."),
        param!("pattern", "Pattern", unlimited_float, 2.0, 0.1, 10.0, "Per-iteration spacing divisor (`s /= pattern`). 1.0 = no spacing change; higher = tighter."),
        param!("spacing", "Spacing", unlimited_float, 0.05, -2.0, 2.0, "Initial fold offset (`uv = abs(uv) - spacing`)."),
        param!("rotate1", "Rotate 1", unlimited_float, 0.0, -6.28, 6.28, "Global pre-iteration rotation angle (radians)."),
        param!("rotate2", "Rotate 2", unlimited_float, 0.0, -6.28, 6.28, "Per-iteration rotation angle (radians)."),
        param!("zoom", "Zoom", unlimited_float, 1.0, 0.1, 10.0, "Global zoom factor (`uv *= zoom`)."),
        param!("gradient", "Gradient", unlimited_int, 0.0, 0.0, 1.0, "JWildfire color output mode. **Only mode 0 honored.** Output is monochrome — `color = vec3(1)` inside the dot, `vec3(0)` outside."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: FRACTALDOTS_2D,
    wgsl_3d: FRACTALDOTS_3D,
};

const FRACTALDOTS_2D: &str = r#"
fn fractaldots_rot(uv: vec2<f32>, a: f32) -> vec2<f32> {
    let c = cos(a);
    let s = sin(a);
    return vec2<f32>(uv.x * c - uv.y * s, uv.y * c + uv.x * s);
}

fn variation_glsl_fractaldots(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vrc: ptr<function, vec3<f32>>) -> vec2<f32> {
    let resolution = max(get_param(xform_id, variation_id, 0u), 100.0);
    let iterations = min(u32(get_param(xform_id, variation_id, 1u)), 20u);
    let dot_size = get_param(xform_id, variation_id, 2u);
    let max_iter = min(u32(get_param(xform_id, variation_id, 3u)), 20u);
    let complexity = get_param(xform_id, variation_id, 4u);
    let pattern_raw = get_param(xform_id, variation_id, 5u);
    let spacing = get_param(xform_id, variation_id, 6u);
    let rotate1 = get_param(xform_id, variation_id, 7u);
    let rotate2 = get_param(xform_id, variation_id, 8u);
    let zoom = get_param(xform_id, variation_id, 9u);

    let pattern = sign(pattern_raw) * max(abs(pattern_raw), 1.0e-6);
    let circle_size = dot_size / (3.0 * pow(2.0, f32(max_iter)));

    let i_f = floor(rng_nextf(rng) * resolution);
    let j_f = floor(rng_nextf(rng) * resolution);
    var uv = vec2<f32>((i_f + 0.5) / resolution - 0.5, (j_f + 0.5) / resolution - 0.5);
    uv = fractaldots_rot(uv, rotate1);
    uv = uv * zoom;

    var s: f32 = spacing;
    for (var i: u32 = 0u; i < max_iter; i = i + 1u) {
        uv = abs(uv) - s;
        uv = uv - vec2<f32>(complexity);
        uv = fractaldots_rot(uv, rotate2);
        s = s / pattern;
        if (iterations < i) { break; }
    }
    let c = select(0.0, 1.0, length(uv) <= circle_size);
    *vrc = vec3<f32>(c);
    return vec2<f32>(i_f / resolution - 0.5, j_f / resolution - 0.5);
}
"#;

const FRACTALDOTS_3D: &str = r#"
fn fractaldots_rot(uv: vec2<f32>, a: f32) -> vec2<f32> {
    let c = cos(a);
    let s = sin(a);
    return vec2<f32>(uv.x * c - uv.y * s, uv.y * c + uv.x * s);
}

fn variation_glsl_fractaldots(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vrc: ptr<function, vec3<f32>>) -> vec3<f32> {
    let resolution = max(get_param(xform_id, variation_id, 0u), 100.0);
    let iterations = min(u32(get_param(xform_id, variation_id, 1u)), 20u);
    let dot_size = get_param(xform_id, variation_id, 2u);
    let max_iter = min(u32(get_param(xform_id, variation_id, 3u)), 20u);
    let complexity = get_param(xform_id, variation_id, 4u);
    let pattern_raw = get_param(xform_id, variation_id, 5u);
    let spacing = get_param(xform_id, variation_id, 6u);
    let rotate1 = get_param(xform_id, variation_id, 7u);
    let rotate2 = get_param(xform_id, variation_id, 8u);
    let zoom = get_param(xform_id, variation_id, 9u);
    let pattern = sign(pattern_raw) * max(abs(pattern_raw), 1.0e-6);
    let circle_size = dot_size / (3.0 * pow(2.0, f32(max_iter)));
    let i_f = floor(rng_nextf(rng) * resolution);
    let j_f = floor(rng_nextf(rng) * resolution);
    var uv = vec2<f32>((i_f + 0.5) / resolution - 0.5, (j_f + 0.5) / resolution - 0.5);
    uv = fractaldots_rot(uv, rotate1);
    uv = uv * zoom;
    var s: f32 = spacing;
    for (var i: u32 = 0u; i < max_iter; i = i + 1u) {
        uv = abs(uv) - s;
        uv = uv - vec2<f32>(complexity);
        uv = fractaldots_rot(uv, rotate2);
        s = s / pattern;
        if (iterations < i) { break; }
    }
    let c = select(0.0, 1.0, length(uv) <= circle_size);
    *vrc = vec3<f32>(c);
    return vec3<f32>(i_f / resolution - 0.5, j_f / resolution - 0.5, p.z);
}
"#;

// =============================================================================
// glsl_starsfield
// =============================================================================

/// Animated star field with rotating layers. Each of 10 layers picks
/// a hash-based offset, rotates the uv, renders a soft-edged "star"
/// point via smoothstep, fading in/out over a triangle wave. Final
/// color adds a `glow` term derived from a 2D hash.
///
/// 5 params.
/// 
/// # Authors
/// - Jesus Sosa
pub static GLSL_STARSFIELD: VariationDef = VariationDef {
    name: "glsl_starsfield",
    aliases: &[],
    display_name: "GLSL StarsField",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::WritesRgb, Feature::NeverZ],
    parameters: &[
        param!("density_pixels", "Density Pixels", unlimited_int, 1000000.0, 100.0, 10000000.0, "Discrete grid resolution."),
        param!("seed", "Seed", unlimited_int, 10000.0, 0.0, 10000.0, "JWildfire CPU-only — set `time` directly."),
        param!("time", "Time", unlimited_float, 0.0, -10000.0, 10000.0, "Animation parameter. Drives the global rotation and the per-layer phase."),
        param!("z_distance", "Z Distance", unlimited_float, 2.0, 0.1, 10.0, "Layer perspective distance. Used in the layer-local `f = fract(uv·5)·z - 1` mapping; lower = closer (larger stars)."),
        param!("glow", "Glow", unlimited_float, 2.0, 0.0, 10.0, "Brightness of the per-pixel hash glow added on top of all layers."),
        param!("gradient", "Gradient", unlimited_int, 0.0, 0.0, 1.0, "JWildfire color output mode. **Only mode 0 honored.**"),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: STARSFIELD_2D,
    wgsl_3d: STARSFIELD_3D,
};

const STARSFIELD_2D: &str = r#"
fn starsfield_hash11(p_in: f32) -> f32 {
    var pp = fract(p_in * 35.35);
    // JWildfire's `js.glsl.G.dot(scalar, scalar)` is `a * b` — the
    // shadertoy idiom of treating scalars as 1D vectors.
    pp = pp + pp * (pp + 45.85);
    return fract(pp * 7858.58);
}

fn starsfield_hash21(p_in: vec2<f32>) -> f32 {
    var pp = fract(p_in * vec2<f32>(451.45, 231.95));
    pp = pp + dot(pp, pp + vec2<f32>(78.78));
    return fract(pp.x * pp.y);
}

fn starsfield_hash22(p_in: vec2<f32>) -> vec2<f32> {
    var t1 = vec3<f32>(p_in.x, p_in.y, p_in.x);
    t1 = t1 * vec3<f32>(451.45, 231.95, 7878.5);
    var q = fract(t1);
    q = q + dot(q, q + vec3<f32>(78.78));
    return fract(vec2<f32>(q.x, q.z) * vec2<f32>(q.y));
}

fn starsfield_layer(uv_in: vec2<f32>, z_distance: f32) -> f32 {
    let uv = uv_in * 5.0;
    let i = floor(uv);
    let f = fract(uv) * z_distance - 1.0;
    let pp = starsfield_hash22(i) * 0.3;
    let d = length(f - pp);
    var c = smoothstep(0.1 + 0.8 * starsfield_hash21(i), 0.01, d);
    let safe_d = max(d, 1.0e-32);
    c = c * (1.0 / safe_d) * 0.2;
    return c;
}

fn starsfield_rotate(uv: vec2<f32>, a: f32) -> vec2<f32> {
    let c = cos(a);
    let s = sin(a);
    return vec2<f32>(uv.x * c - uv.y * s, uv.x * s + uv.y * c);
}

fn variation_glsl_starsfield(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vrc: ptr<function, vec3<f32>>) -> vec2<f32> {
    let resolution = max(get_param(xform_id, variation_id, 0u), 100.0);
    let time = get_param(xform_id, variation_id, 2u);
    let z_distance = get_param(xform_id, variation_id, 3u);
    let glow = get_param(xform_id, variation_id, 4u);

    let i_f = floor(rng_nextf(rng) * resolution);
    let j_f = floor(rng_nextf(rng) * resolution);
    var uv = vec2<f32>((i_f + 0.5) / resolution - 0.5, (j_f + 0.5) / resolution - 0.5);

    var col = vec3<f32>(0.0);
    uv = starsfield_rotate(uv, time);
    uv = uv + vec2<f32>(cos(time), sin(time)) * 2.0;

    // Layer loop: i = 0.0 .. 1.0 stepping 0.1 → 10 iterations.
    for (var ix: u32 = 0u; ix < 10u; ix = ix + 1u) {
        let i = f32(ix) * 0.1;
        uv = starsfield_rotate(uv, starsfield_hash11(i) * 6.28);
        let t_arg = i - time;
        let t = fract(t_arg);
        let s = smoothstep(0.0, 1.0, t);
        var f = smoothstep(0.0, 1.0, t);
        f = f * smoothstep(1.0, 0.0, t);
        let k = starsfield_hash22(vec2<f32>(i, i * 5.0)) * 0.1;
        let l = starsfield_layer((uv - k) * s, z_distance);
        col = col + mix(vec3<f32>(0.0), vec3<f32>(1.0), vec3<f32>(l)) * f;
    }
    let glow_v = glow * starsfield_hash21(uv + time);
    col = col + vec3<f32>(glow_v);
    *vrc = col;
    return vec2<f32>(i_f / resolution - 0.5, j_f / resolution - 0.5);
}
"#;

const STARSFIELD_3D: &str = r#"
fn starsfield_hash11(p_in: f32) -> f32 {
    var pp = fract(p_in * 35.35);
    // JWildfire's `js.glsl.G.dot(scalar, scalar)` is `a * b` — the
    // shadertoy idiom of treating scalars as 1D vectors.
    pp = pp + pp * (pp + 45.85);
    return fract(pp * 7858.58);
}

fn starsfield_hash21(p_in: vec2<f32>) -> f32 {
    var pp = fract(p_in * vec2<f32>(451.45, 231.95));
    pp = pp + dot(pp, pp + vec2<f32>(78.78));
    return fract(pp.x * pp.y);
}

fn starsfield_hash22(p_in: vec2<f32>) -> vec2<f32> {
    var t1 = vec3<f32>(p_in.x, p_in.y, p_in.x);
    t1 = t1 * vec3<f32>(451.45, 231.95, 7878.5);
    var q = fract(t1);
    q = q + dot(q, q + vec3<f32>(78.78));
    return fract(vec2<f32>(q.x, q.z) * vec2<f32>(q.y));
}

fn starsfield_layer(uv_in: vec2<f32>, z_distance: f32) -> f32 {
    let uv = uv_in * 5.0;
    let i = floor(uv);
    let f = fract(uv) * z_distance - 1.0;
    let pp = starsfield_hash22(i) * 0.3;
    let d = length(f - pp);
    var c = smoothstep(0.1 + 0.8 * starsfield_hash21(i), 0.01, d);
    let safe_d = max(d, 1.0e-32);
    c = c * (1.0 / safe_d) * 0.2;
    return c;
}

fn starsfield_rotate(uv: vec2<f32>, a: f32) -> vec2<f32> {
    let c = cos(a);
    let s = sin(a);
    return vec2<f32>(uv.x * c - uv.y * s, uv.x * s + uv.y * c);
}

fn variation_glsl_starsfield(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vrc: ptr<function, vec3<f32>>) -> vec3<f32> {
    let resolution = max(get_param(xform_id, variation_id, 0u), 100.0);
    let time = get_param(xform_id, variation_id, 2u);
    let z_distance = get_param(xform_id, variation_id, 3u);
    let glow = get_param(xform_id, variation_id, 4u);
    let i_f = floor(rng_nextf(rng) * resolution);
    let j_f = floor(rng_nextf(rng) * resolution);
    var uv = vec2<f32>((i_f + 0.5) / resolution - 0.5, (j_f + 0.5) / resolution - 0.5);
    var col = vec3<f32>(0.0);
    uv = starsfield_rotate(uv, time);
    uv = uv + vec2<f32>(cos(time), sin(time)) * 2.0;
    for (var ix: u32 = 0u; ix < 10u; ix = ix + 1u) {
        let i = f32(ix) * 0.1;
        uv = starsfield_rotate(uv, starsfield_hash11(i) * 6.28);
        let t_arg = i - time;
        let t = fract(t_arg);
        let s = smoothstep(0.0, 1.0, t);
        var f = smoothstep(0.0, 1.0, t);
        f = f * smoothstep(1.0, 0.0, t);
        let k = starsfield_hash22(vec2<f32>(i, i * 5.0)) * 0.1;
        let l = starsfield_layer((uv - k) * s, z_distance);
        col = col + mix(vec3<f32>(0.0), vec3<f32>(1.0), vec3<f32>(l)) * f;
    }
    let glow_v = glow * starsfield_hash21(uv + time);
    col = col + vec3<f32>(glow_v);
    *vrc = col;
    return vec3<f32>(i_f / resolution - 0.5, j_f / resolution - 0.5, p.z);
}
"#;

// =============================================================================
// glsl_grid3D
// =============================================================================

/// 3D raymarched grid pattern (kabuto's shadertoy, forked by tigrou
/// and kapsy). Casts a ray through 30 raymarch steps into a procedural
/// 3-iteration field, accumulates color by distance, then maps to RGB.
/// Output is grayscale (the JWildfire source returns `vec3(sum)`).
///
/// 3 params (no algorithm knobs — JWildfire only exposes the
/// standard 3).
/// 
/// # Authors
/// - Jesus Sosa
pub static GLSL_GRID3D: VariationDef = VariationDef {
    name: "glsl_grid3D",
    aliases: &[],
    display_name: "GLSL Grid 3D",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::WritesRgb, Feature::NeverZ],
    parameters: &[
        param!("density_pixels", "Density Pixels", unlimited_int, 1000000.0, 100.0, 10000000.0, "Discrete grid resolution."),
        param!("seed", "Seed", unlimited_int, 10000.0, 0.0, 10000.0, "JWildfire CPU-only — set `time` directly."),
        param!("time", "Time", unlimited_float, 200.0, 1.0, 1000.0, "Animation parameter. Drives the camera rotation (`a = time · 0.021`) and the camera Y position (`pos.y = time · 0.1`)."),
        param!("gradient", "Gradient", unlimited_int, 0.0, 0.0, 1.0, "JWildfire color output mode. **Only mode 0 honored.** Output is grayscale."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: GRID3D_2D,
    wgsl_3d: GRID3D_3D,
};

// Per call: 30 raymarch steps × field() (3 inner iters) = ~90 inner +
// ~30 outer = ~120 op blocks × ~20 ops = ~2400 ops/call. Within TDR.
const GRID3D_2D: &str = r#"
fn grid3d_field(p_in: vec3<f32>) -> vec3<f32> {
    var pp = p_in * 0.1;
    var f: f32 = 0.1;
    for (var i: u32 = 0u; i < 3u; i = i + 1u) {
        pp = vec3<f32>(pp.y, pp.z, pp.x);  // swizzle (matches JWF's "mat3 hack")
        pp = abs(fract(pp) - 0.5);
        pp = pp * 2.0;
        f = f * 2.0;
    }
    let pp_sq = pp * pp;
    let sum = pp_sq + vec3<f32>(pp_sq.y, pp_sq.z, pp_sq.x);
    let safe_f = max(f, 1.0e-32);
    return sqrt(max(sum, vec3<f32>(0.0))) / safe_f - 0.05;
}

fn variation_glsl_grid3D(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vrc: ptr<function, vec3<f32>>) -> vec2<f32> {
    let resolution = max(get_param(xform_id, variation_id, 0u), 100.0);
    let time = get_param(xform_id, variation_id, 2u);

    let i_f = floor(rng_nextf(rng) * resolution);
    let j_f = floor(rng_nextf(rng) * resolution);
    let x = i_f + 0.5;
    let y = j_f + 0.5;

    let max_iter: u32 = 30u;
    var dir = normalize(vec3<f32>(x / resolution - 0.5, y / resolution - 0.5, 1.0));
    let a = time * 0.021;
    var pos = vec3<f32>(0.0, time * 0.1, 0.0);

    // Apply two mat3 rotations (X-axis and Y-axis). WGSL mat3x3 is
    // column-major; we matmul as explicit dot products to avoid
    // confusion about JWF's row-major mat3 constructor.
    let ca = cos(a); let sa = sin(a);
    // X-axis rotation: y' = y·cos - z·sin, z' = y·sin + z·cos
    dir = vec3<f32>(dir.x, dir.y * ca - dir.z * sa, dir.y * sa + dir.z * ca);
    // Y-axis rotation: x' = x·cos - z·sin, z' = x·sin + z·cos
    dir = vec3<f32>(dir.x * ca - dir.z * sa, dir.y, dir.x * sa + dir.z * ca);

    var color = vec3<f32>(0.0);
    for (var i: u32 = 0u; i < max_iter; i = i + 1u) {
        let f2 = grid3d_field(pos);
        let f = min(min(f2.x, f2.y), f2.z);
        pos = pos + dir * f;
        let t0 = vec3<f32>(f32(max_iter - i)) / (f2 + 0.1);
        color = color + t0;
    }
    let denom = 1.0 + color * (0.09 / f32(max_iter * max_iter));
    let safe_denom = sign(denom) * max(abs(denom), vec3<f32>(1.0e-32));
    let color3 = vec3<f32>(1.0) - vec3<f32>(1.0) / safe_denom;
    let sum = color3.x + color3.y + color3.z;
    *vrc = vec3<f32>(sum);
    return vec2<f32>(i_f / resolution - 0.5, j_f / resolution - 0.5);
}
"#;

const GRID3D_3D: &str = r#"
fn grid3d_field(p_in: vec3<f32>) -> vec3<f32> {
    var pp = p_in * 0.1;
    var f: f32 = 0.1;
    for (var i: u32 = 0u; i < 3u; i = i + 1u) {
        pp = vec3<f32>(pp.y, pp.z, pp.x);  // swizzle (matches JWF's "mat3 hack")
        pp = abs(fract(pp) - 0.5);
        pp = pp * 2.0;
        f = f * 2.0;
    }
    let pp_sq = pp * pp;
    let sum = pp_sq + vec3<f32>(pp_sq.y, pp_sq.z, pp_sq.x);
    let safe_f = max(f, 1.0e-32);
    return sqrt(max(sum, vec3<f32>(0.0))) / safe_f - 0.05;
}

fn variation_glsl_grid3D(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vrc: ptr<function, vec3<f32>>) -> vec3<f32> {
    let resolution = max(get_param(xform_id, variation_id, 0u), 100.0);
    let time = get_param(xform_id, variation_id, 2u);
    let i_f = floor(rng_nextf(rng) * resolution);
    let j_f = floor(rng_nextf(rng) * resolution);
    let max_iter: u32 = 30u;
    var dir = normalize(vec3<f32>((i_f + 0.5) / resolution - 0.5, (j_f + 0.5) / resolution - 0.5, 1.0));
    let a = time * 0.021;
    var pos = vec3<f32>(0.0, time * 0.1, 0.0);
    let ca = cos(a); let sa = sin(a);
    dir = vec3<f32>(dir.x, dir.y * ca - dir.z * sa, dir.y * sa + dir.z * ca);
    dir = vec3<f32>(dir.x * ca - dir.z * sa, dir.y, dir.x * sa + dir.z * ca);
    var color = vec3<f32>(0.0);
    for (var i: u32 = 0u; i < max_iter; i = i + 1u) {
        let f2 = grid3d_field(pos);
        let f = min(min(f2.x, f2.y), f2.z);
        pos = pos + dir * f;
        let t0 = vec3<f32>(f32(max_iter - i)) / (f2 + 0.1);
        color = color + t0;
    }
    let denom = 1.0 + color * (0.09 / f32(max_iter * max_iter));
    let safe_denom = sign(denom) * max(abs(denom), vec3<f32>(1.0e-32));
    let color3 = vec3<f32>(1.0) - vec3<f32>(1.0) / safe_denom;
    let sum = color3.x + color3.y + color3.z;
    *vrc = vec3<f32>(sum);
    return vec3<f32>(i_f / resolution - 0.5, j_f / resolution - 0.5, p.z);
}
"#;
