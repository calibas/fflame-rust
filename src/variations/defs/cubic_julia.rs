//! `cubic_julia` — Julia sets of cubic polynomials via 3-branch inverse
//! iteration (original).
//!
//! The cubic analogue of the classic `julia` variation: where `julia`
//! inverts z² + c by picking a random square-root branch, this inverts
//!
//! ```text
//! f(z) = z³ + a·z + b
//! ```
//!
//! by solving `w³ + a·w + (b − z) = 0` — a depressed cubic in w, so
//! Cardano gives all three inverse branches in closed form and the
//! chaos game picks one per call. Inverse iteration converges onto the
//! Julia set of f: every cubic polynomial is affinely conjugate to the
//! depressed form, and the flame's affine transforms supply that
//! conjugation, so the four coefficient sliders cover the entire cubic
//! family with no redundancy. Two independent critical points
//! (±√(−a/3)) — unlike quadratics — give cubic Julia sets their richer
//! connected/disconnected zoo (default a = −1, b = 0.25: the classic
//! two-critical-point real cubic).
//!
//! Branch weights bias the random branch choice — zeroing one selects
//! a sub-IFS whose attractor is a genuinely different sub-self-similar
//! subset of the Julia set (the strongest shape knob, as in
//! `jacobian_counterexample`).
//!
//! Colorings: Branch (which inverse sheet), Branch Blend (persistent
//! register → IFS itinerary / symbolic address), Log Deriv
//! (log|f′| = log|3z²+a| at the new point — the local contraction rate
//! of the branch, a Lyapunov-style gradient along the Julia set).
//!
//! Forward mode applies f itself (expanding — blend with contracting
//! affines like any polynomial variation).

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

pub static CUBIC_JULIA: VariationDef = VariationDef {
    name: "cubic_julia",
    aliases: &[],
    display_name: "Cubic Julia",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::WritesColor],
    // Slot 0: color register for the Branch Blend mode.
    state_count: 1,
    wgsl_state_init: None,
    parameters: &[
        param!("mode", "Mode", enum, 0, &["Inverse (Julia)", "Forward"], "Inverse: chaos game over the three closed-form inverse branches of f(z) = z³ + a·z + b (random Cardano root per call) — converges onto the Julia set of f. Forward: f itself — expanding, blend with contracting affines."),
        param!("a_re", "A re", unlimited_float, -1.0, -3.0, 3.0, "Real part of the linear coefficient a in f(z) = z³ + a·z + b. Every cubic is affinely conjugate to this depressed form, so a and b cover the whole cubic family (the flame's affines supply the conjugation). a controls the two critical points ±√(−a/3)."),
        param!("a_im", "A im", unlimited_float, 0.0, -3.0, 3.0, "Imaginary part of a."),
        param!("b_re", "B re", unlimited_float, 0.25, -3.0, 3.0, "Real part of the constant b — the Julia parameter (the c of z²+c, one degree up). Small |b| keeps the set connected-ish; large |b| shatters it into dust."),
        param!("b_im", "B im", unlimited_float, 0.0, -3.0, 3.0, "Imaginary part of b."),
        param!("w1", "Branch 1", float, 1.0, 0.0, 1.0, "Selection weight of inverse branch 1 (Cardano root ω⁰). Biasing or zeroing branch weights selects sub-IFS attractors — genuinely different sub-self-similar subsets of the Julia set."),
        param!("w2", "Branch 2", float, 1.0, 0.0, 1.0, "Selection weight of inverse branch 2 (Cardano root ω¹)."),
        param!("w3", "Branch 3", float, 1.0, 0.0, 1.0, "Selection weight of inverse branch 3 (Cardano root ω²)."),
        param!("dc_mode", "Color Mode", enum, 0, &["Off", "Branch", "Branch Blend", "Log Deriv"], "Direct-color source (needs the transform's Direct Color > 0). Branch: which inverse sheet was taken this call. Branch Blend: persistent register pulled toward each branch's palette slot — colors the Julia set by IFS itinerary (symbolic address), segmenting its self-similar pieces. Log Deriv: log|f′(z)| = log|3z²+a| at the new point — the local contraction rate, a smooth Lyapunov-style gradient."),
        param!("dc_scale", "Color Scale", float, 1.0, 0.1, 8.0, "Palette-index multiplier for the color modes."),
        param!("color_speed", "Color Speed", float, 0.3, 0.01, 1.0, "Blend rate of the persistent register in Branch Blend: low = deep itinerary history (coarse self-similar pieces), high = recent branches only (fine detail)."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

// The Cardano step is duplicated verbatim into both bodies (only one is
// compiled per flame; the builder dedups identical helper fns anyway).
const WGSL_2D: &str = r#"
fn cjulia_cbrt(z: vec2<f32>) -> vec2<f32> {
    let r = length(z);
    if (r < 1e-30) { return vec2<f32>(0.0, 0.0); }
    let th = atan2(z.y, z.x) / 3.0;
    return pow(r, 0.3333333333) * vec2<f32>(cos(th), sin(th));
}

// Branch k of the inverse of f(w) = w^3 + a w + b at z: the depressed
// cubic w^3 + a w + (b - z) = 0 via Cardano.
fn cjulia_inverse(z: vec2<f32>, a: vec2<f32>, b: vec2<f32>, k: u32) -> vec2<f32> {
    let q = b - z;
    if (dot(a, a) < 1e-24 && dot(q, q) < 1e-24) { return vec2<f32>(0.0, 0.0); }
    let D = csqrt(0.25 * cmul(q, q) + cmul(cmul(a, a), a) / 27.0);
    var w = cjulia_cbrt(-0.5 * q + D);
    if (dot(w, w) < 1e-16) { w = cjulia_cbrt(-0.5 * q - D); }
    var om = vec2<f32>(1.0, 0.0);
    if (k == 1u) { om = vec2<f32>(-0.5, 0.86602540); }
    else if (k == 2u) { om = vec2<f32>(-0.5, -0.86602540); }
    let wk = cmul(w, om);
    if (dot(wk, wk) < 1e-24) { return vec2<f32>(0.0, 0.0); }
    return wk - cdiv(a, 3.0 * wk);
}

fn cjulia_pick(r: f32, w1: f32, w2: f32, w3: f32) -> u32 {
    let tot = max(w1 + w2 + w3, 1e-9);
    let x = r * tot;
    if (x < w1) { return 0u; }
    if (x < w1 + w2) { return 1u; }
    return 2u;
}

fn variation_cubic_julia(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    let mode = u32(get_param(xform_id, variation_id, 0u));
    let a = vec2<f32>(get_param(xform_id, variation_id, 1u), get_param(xform_id, variation_id, 2u));
    let b = vec2<f32>(get_param(xform_id, variation_id, 3u), get_param(xform_id, variation_id, 4u));
    let w1 = get_param(xform_id, variation_id, 5u);
    let w2 = get_param(xform_id, variation_id, 6u);
    let w3 = get_param(xform_id, variation_id, 7u);
    let dc_mode = u32(get_param(xform_id, variation_id, 8u));
    let dc_scale = get_param(xform_id, variation_id, 9u);
    let cspeed = get_param(xform_id, variation_id, 10u);

    var out: vec2<f32>;
    var k = 0u;
    if (mode == 1u) {
        out = cmul(cmul(p, p), p) + cmul(a, p) + b;
    } else {
        k = cjulia_pick(rng_nextf(rng), w1, w2, w3);
        out = cjulia_inverse(p, a, b, k);
    }

    if (dc_mode == 1u) {
        *vc = fract((f32(k) + 0.5) / 3.0 * dc_scale);
    } else if (dc_mode == 2u) {
        var creg = get_state(xform_id, variation_id, 0u);
        creg = mix(creg, (f32(k) + 0.5) / 3.0, cspeed);
        set_state(xform_id, variation_id, 0u, creg);
        *vc = fract(creg * dc_scale);
    } else if (dc_mode == 3u) {
        let d = 3.0 * cmul(out, out) + a;
        *vc = fract(0.5 + 0.25 * dc_scale * log(max(length(d), 1e-20)));
    }
    return out;
}
"#;

const WGSL_3D: &str = r#"
fn cjulia_cbrt(z: vec2<f32>) -> vec2<f32> {
    let r = length(z);
    if (r < 1e-30) { return vec2<f32>(0.0, 0.0); }
    let th = atan2(z.y, z.x) / 3.0;
    return pow(r, 0.3333333333) * vec2<f32>(cos(th), sin(th));
}

// Branch k of the inverse of f(w) = w^3 + a w + b at z: the depressed
// cubic w^3 + a w + (b - z) = 0 via Cardano.
fn cjulia_inverse(z: vec2<f32>, a: vec2<f32>, b: vec2<f32>, k: u32) -> vec2<f32> {
    let q = b - z;
    if (dot(a, a) < 1e-24 && dot(q, q) < 1e-24) { return vec2<f32>(0.0, 0.0); }
    let D = csqrt(0.25 * cmul(q, q) + cmul(cmul(a, a), a) / 27.0);
    var w = cjulia_cbrt(-0.5 * q + D);
    if (dot(w, w) < 1e-16) { w = cjulia_cbrt(-0.5 * q - D); }
    var om = vec2<f32>(1.0, 0.0);
    if (k == 1u) { om = vec2<f32>(-0.5, 0.86602540); }
    else if (k == 2u) { om = vec2<f32>(-0.5, -0.86602540); }
    let wk = cmul(w, om);
    if (dot(wk, wk) < 1e-24) { return vec2<f32>(0.0, 0.0); }
    return wk - cdiv(a, 3.0 * wk);
}

fn cjulia_pick(r: f32, w1: f32, w2: f32, w3: f32) -> u32 {
    let tot = max(w1 + w2 + w3, 1e-9);
    let x = r * tot;
    if (x < w1) { return 0u; }
    if (x < w1 + w2) { return 1u; }
    return 2u;
}

fn variation_cubic_julia(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    let mode = u32(get_param(xform_id, variation_id, 0u));
    let a = vec2<f32>(get_param(xform_id, variation_id, 1u), get_param(xform_id, variation_id, 2u));
    let b = vec2<f32>(get_param(xform_id, variation_id, 3u), get_param(xform_id, variation_id, 4u));
    let w1 = get_param(xform_id, variation_id, 5u);
    let w2 = get_param(xform_id, variation_id, 6u);
    let w3 = get_param(xform_id, variation_id, 7u);
    let dc_mode = u32(get_param(xform_id, variation_id, 8u));
    let dc_scale = get_param(xform_id, variation_id, 9u);
    let cspeed = get_param(xform_id, variation_id, 10u);

    var out: vec2<f32>;
    var k = 0u;
    if (mode == 1u) {
        out = cmul(cmul(p.xy, p.xy), p.xy) + cmul(a, p.xy) + b;
    } else {
        k = cjulia_pick(rng_nextf(rng), w1, w2, w3);
        out = cjulia_inverse(p.xy, a, b, k);
    }

    if (dc_mode == 1u) {
        *vc = fract((f32(k) + 0.5) / 3.0 * dc_scale);
    } else if (dc_mode == 2u) {
        var creg = get_state(xform_id, variation_id, 0u);
        creg = mix(creg, (f32(k) + 0.5) / 3.0, cspeed);
        set_state(xform_id, variation_id, 0u, creg);
        *vc = fract(creg * dc_scale);
    } else if (dc_mode == 3u) {
        let d = 3.0 * cmul(out, out) + a;
        *vc = fract(0.5 + 0.25 * dc_scale * log(max(length(d), 1e-20)));
    }
    return vec3<f32>(out, p.z);
}
"#;
