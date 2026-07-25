//! `jacobian_counterexample` — inverse-branch IFS of the Jacobian-conjecture
//! counterexample map (original).
//!
//! The polynomial map F: ℂ³→ℂ³
//!
//! ```text
//! P = (1+xy)³z + y²(1+xy)(4+3xy)
//! Q = y + 3x(1+xy)²z + 3xy²(4+3xy)
//! R = 2x − 3x²y − x³z
//! ```
//!
//! has det DF ≡ −2 (constant!) yet is generically 3-to-1 — a
//! counterexample to the Jacobian conjecture (announced July 2026;
//! verified symbolically here: the determinant, the 3 preimages, and
//! every reduction identity below — see
//! `scripts/verify_jacobian_counterexample.py`). It is weighted-homogeneous
//! under `(x,y,z) ↦ (λ⁻¹x, λy, λ²z)`, so the whole dynamics descends
//! to the weight-0 coordinates `a = xy`, `b = x²z`: with `c = 2−3a−b`
//! and `V = (1+a)²b + a²(3a+4)`,
//!
//! ```text
//! a' = c·(a+3V)      b' = c²·(1+a)·V      (identically a' = RQ, b' = R²P)
//! ```
//!
//! a degree-4/6 endomorphism G of ℂ² with det DG = 2c² and the whole
//! line c = 0 contracted to the origin (the non-properness that lets an
//! étale map be 3-to-1). The third coordinate is a pure skew product
//! `R = x·c(a,b)`.
//!
//! **Inverse IFS mode** (the point): G's three inverse branches are
//! closed-form. Eliminating b from the fiber system, the resultant
//! factors as an extraneous (a+1)³ times a cubic that is *already
//! depressed* in u = 1+a:
//!
//! ```text
//! K·u³ + M·u − 4B = 0,   K = A³−A²−18AB+27B²+16B,   M = A²−12B
//! ```
//!
//! Cardano gives the three roots; b follows from a quadratic (E1 is
//! quadratic in b, the correct root selected by the second fiber
//! equation), giving a genuine 3-map IFS — the chaos game picks a
//! random branch each call, exactly `julian`'s random-kth-root pattern.
//! Each branch contracts area by 1/|2c²| (non-constant!), so the
//! invariant measure is singular — real density contrast, unlike the
//! constant-Jacobian full map. The attractor (the Julia-set analogue of
//! G) is a fractal rim pinned at a = −1: as |B| grows the cubic forces
//! u = 1+a → 0, so the a-projection cannot migrate — a structural
//! rigidity of the counterexample map. It exists *because* the map
//! fails injectivity: were the Jacobian conjecture true here, one
//! branch and a point attractor.
//!
//! **Tunable structure** (everything that preserves the fiber cubic):
//! - **Julia offset** j ∈ ℂ²: inverse-iterate `G + j` (the z²+c
//!   pattern) — the cubic's target just shifts. The rim stays anchored
//!   at a = −1 but its internal structure transforms dramatically
//!   (large b-offsets turn the thin rim into gear-toothed annuli with
//!   holes), and the fiber attractor moves (3D z/w, Fiber Phase).
//! - **Branch weights**: biasing/disabling branches selects
//!   sub-semigroups of the inverse monoid — genuinely different
//!   sub-self-similar limit sets (the strongest shape knob).
//! - **Steps**: inverse applications per call; >1 pulls stray
//!   trajectories onto the attractor harder (sharpens the 2D halo).
//!
//! **Forward mode**: G + j itself (expanding; mix with contracting
//! affines like any polynomial variation).
//!
//! State is (a,b) ∈ ℂ² = 4 real dims. The 3D body carries b honestly in
//! (z, w) (`Feature::NeedsW`, the honest-4D pattern of `polychoron` /
//! `honeycomb4d`) — note the fiber attractor sits near b ≈ 3.8, so the
//! 3D cloud lives at z ≈ 3.8·scale; the 2D body rides b in per-thread
//! state slots seeded randomly at spawn. `log|c|` — the Birkhoff
//! cocycle of the lost skew coordinate (`log|x|` accumulates as
//! Σ log|c|) — is the natural direct-color channel; Branch Blend colors
//! by the IFS itinerary (symbolic address), Lyapunov by the running
//! contraction average, Fiber Phase by arg(b) (the hidden dimension).
//!
//! Original construction for this project; map due to the July 2026
//! Jacobian-conjecture counterexample announcement.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Inverse-branch IFS of the 2026 Jacobian-conjecture counterexample
/// map (closed-form 3-sheet inverse).
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5
pub static JACOBIAN_COUNTEREXAMPLE: VariationDef = VariationDef {
    name: "jacobian_counterexample",
    // Shipped briefly as `jacobian_cubic`; the map is degree 4/6 (the
    // "cubic" was the 3-sheeted fiber), so the honest name won.
    aliases: &["jacobian_cubic"],
    display_name: "Jacobian Counterexample",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::NeedsW, Feature::WritesColor, Feature::AlwaysZ],
    // Slots 0,1: the b coordinate (re, im) for the 2D body (the 3D body
    // carries b in (z, w) instead and leaves these untouched). Slot 2:
    // color register for the Branch Blend / Lyapunov modes.
    state_count: 3,
    // Seed b randomly in a small disk so threads explore distinct fibers.
    wgsl_state_init: Some(
        "        let r1 = rng_nextf(&rng);\n\
         \x20       let r2 = rng_nextf(&rng);\n\
         \x20       set_state(xform_id, variation_id, 0u, (r1 - 0.5) * 0.6);\n\
         \x20       set_state(xform_id, variation_id, 1u, (r2 - 0.5) * 0.6);\n\
         \x20       set_state(xform_id, variation_id, 2u, 0.5);",
    ),
    parameters: &[
        param!("mode", "Mode", enum, 0, &["Inverse IFS", "Forward"], "Inverse IFS: chaos game over the three closed-form inverse branches (random Cardano root per call) — converges onto the fractal attractor, each branch contracting area by 1/|2c²|. Forward: the degree-4/6 endomorphism itself — expanding, so blend it with contracting affines like any polynomial variation."),
        param!("scale", "Scale", float, 1.0, 0.05, 4.0, "Similarity conjugation: world point = (math point − center) × scale."),
        param!("center_x", "Center re", unlimited_float, -1.0, -4.0, 4.0, "Real part of the view center in math coordinates, subtracted before plotting. The attractor rim is pinned at a = −1 (a structural rigidity of the map), so the default centers it on the origin."),
        param!("center_y", "Center im", unlimited_float, 0.0, -4.0, 4.0, "Imaginary part of the view center."),
        param!("steps", "Steps", int, 1.0, 1.0, 8.0, "Inverse applications per call (fresh random branch each). Higher values pull stray trajectories onto the attractor harder — sharpens the halo the 2D hidden-fiber desync paints — at proportional GPU cost. Forward mode: leave at 1 (degree 6 per step compounds fast)."),
        param!("w1", "Branch 1", float, 1.0, 0.0, 1.0, "Selection weight of inverse branch 1 (Cardano root ω⁰). Biasing or zeroing branch weights selects sub-semigroups of the inverse monoid — genuinely different sub-self-similar limit sets. The strongest shape knob this map has (the map itself has no free constants)."),
        param!("w2", "Branch 2", float, 1.0, 0.0, 1.0, "Selection weight of inverse branch 2 (Cardano root ω¹)."),
        param!("w3", "Branch 3", float, 1.0, 0.0, 1.0, "Selection weight of inverse branch 3 (Cardano root ω²)."),
        param!("julia_a_re", "Julia A re", unlimited_float, 0.0, -8.0, 8.0, "Julia offset on the a-equation: the chaos game inverts G + j instead of G (the z²+c pattern — the fiber cubic's target just shifts, so the closed form survives). The rim stays anchored at a = −1 (structural rigidity of the map) but its internal texture deforms."),
        param!("julia_a_im", "Julia A im", unlimited_float, 0.0, -8.0, 8.0, "Imaginary part of the a-offset."),
        param!("julia_b_re", "Julia B re", unlimited_float, 0.0, -8.0, 8.0, "Julia offset on the b-equation — the stronger of the two: large values transform the thin rim into thick gear-toothed annuli with holes punched through (try −8), and move the fiber attractor visibly in 3D z/w and Fiber Phase coloring."),
        param!("julia_b_im", "Julia B im", unlimited_float, 0.0, -8.0, 8.0, "Imaginary part of the b-offset."),
        param!("dc_mode", "Color Mode", enum, 0, &["Off", "Log C", "Branch", "Branch Blend", "Lyapunov", "Fiber Phase"], "Direct-color source (needs the transform's Direct Color > 0). Log C: instantaneous log|c| (the Birkhoff cocycle of the skew coordinate; local contraction rate det DG = 2c²). Branch: which branch was taken this call. Branch Blend: persistent register pulled toward each branch's palette slot — colors the attractor by IFS itinerary (symbolic address), revealing the self-similar pieces. Lyapunov: running average of log|c| — smooth contraction-rate gradient along the attractor. Fiber Phase: arg(b), the hidden fiber dimension the a-plane projection discards."),
        param!("dc_scale", "Color Scale", float, 1.0, 0.1, 8.0, "Palette-index multiplier for the color modes."),
        param!("color_speed", "Color Speed", float, 0.3, 0.01, 1.0, "Blend rate of the persistent register in Branch Blend / Lyapunov: low = deep itinerary history (coarse self-similar pieces), high = recent branches only (fine detail)."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

// The Cardano step block is duplicated verbatim into both bodies (only
// one body is compiled per flame; the shader builder would dedup the
// helper fns by name anyway).
const WGSL_2D: &str = r#"
fn jcubic_cbrt(z: vec2<f32>) -> vec2<f32> {
    let r = length(z);
    if (r < 1e-30) { return vec2<f32>(0.0, 0.0); }
    let th = atan2(z.y, z.x) / 3.0;
    let rc = pow(r, 0.3333333333);
    return rc * vec2<f32>(cos(th), sin(th));
}

fn jcubic_v(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    let one = vec2<f32>(1.0, 0.0);
    let ap1 = a + one;
    return cmul(cmul(ap1, ap1), b) + cmul(cmul(a, a), 3.0 * a + vec2<f32>(4.0, 0.0));
}

// |E2(a, b) - B|^2 — residual of the second fiber equation, used to pick
// the correct root of the b-quadratic.
fn jcubic_e2res(a: vec2<f32>, b: vec2<f32>, B: vec2<f32>) -> f32 {
    let c = vec2<f32>(2.0, 0.0) - 3.0 * a - b;
    let e = cmul(cmul(c, c), cmul(a + vec2<f32>(1.0, 0.0), jcubic_v(a, b))) - B;
    return dot(e, e);
}

// Weighted 3-way branch pick.
fn jcubic_pick(r: f32, w1: f32, w2: f32, w3: f32) -> u32 {
    let tot = max(w1 + w2 + w3, 1e-9);
    let x = r * tot;
    if (x < w1) { return 0u; }
    if (x < w1 + w2) { return 1u; }
    return 2u;
}

// One application: forward G + j, or inverse branch k of G + j (solve
// G(p) = z - j; the fiber cubic's target just shifts, closed form intact).
fn jcubic_map(ab: vec4<f32>, mode: u32, k: u32, jul: vec4<f32>) -> vec4<f32> {
    let one = vec2<f32>(1.0, 0.0);
    if (mode == 1u) {
        // Forward: G(a,b) + j.
        let c = vec2<f32>(2.0, 0.0) - 3.0 * ab.xy - ab.zw;
        let V = jcubic_v(ab.xy, ab.zw);
        let a2 = cmul(c, ab.xy + 3.0 * V) + jul.xy;
        let b2 = cmul(cmul(c, c), cmul(ab.xy + one, V)) + jul.zw;
        return vec4<f32>(a2, b2);
    }
    // Inverse branch k: fiber cubic K u^3 + M u - 4B = 0 in u = 1 + a.
    let A = ab.xy - jul.xy;
    let B = ab.zw - jul.zw;
    let A2 = cmul(A, A);
    let A3 = cmul(A2, A);
    let K = A3 - A2 - 18.0 * cmul(A, B) + 27.0 * cmul(B, B) + 16.0 * B;
    let M = A2 - 12.0 * B;
    var u: vec2<f32>;
    if (dot(K, K) < 1e-24) {
        // Degenerate leading coefficient: cubic collapses to M u = 4B.
        if (dot(M, M) < 1e-24) { u = vec2<f32>(0.0, 0.0); }
        else { u = cdiv(4.0 * B, M); }
    } else {
        // Depressed Cardano: u^3 + p u + q = 0.
        let p = cdiv(M, K);
        let q = cdiv(-4.0 * B, K);
        let D = csqrt(0.25 * cmul(q, q) + cmul(cmul(p, p), p) / 27.0);
        var w = jcubic_cbrt(-0.5 * q + D);
        if (dot(w, w) < 1e-16) { w = jcubic_cbrt(-0.5 * q - D); }
        // Rotate to branch k: omega^k with omega = e^{2 pi i / 3}.
        var om = vec2<f32>(1.0, 0.0);
        if (k == 1u) { om = vec2<f32>(-0.5, 0.86602540); }
        else if (k == 2u) { om = vec2<f32>(-0.5, -0.86602540); }
        let wk = cmul(w, om);
        if (dot(wk, wk) < 1e-24) { u = vec2<f32>(0.0, 0.0); }
        else { u = wk - cdiv(p, 3.0 * wk); }
    }
    let a = u - one;
    // b from the E1 quadratic: q2 b^2 + q1 b + q0 = 0 with
    // s = a + 12a^2 + 9a^3, t = 3(1+a)^2, tm = 2 - 3a,
    // q2 = -t, q1 = tm*t - s, q0 = tm*s - A.
    let aa = cmul(a, a);
    let s = a + 12.0 * aa + 9.0 * cmul(aa, a);
    let t = 3.0 * cmul(a + one, a + one);
    let tm = vec2<f32>(2.0, 0.0) - 3.0 * a;
    let q1c = cmul(tm, t) - s;
    let q0c = cmul(tm, s) - A;
    var b: vec2<f32>;
    if (dot(t, t) < 1e-24) {
        // a = -1 (extraneous locus): quadratic degenerates to linear.
        b = cdiv(-q0c, q1c);
    } else {
        let disc = csqrt(cmul(q1c, q1c) + 4.0 * cmul(t, q0c));
        let b1 = cdiv(-q1c + disc, -2.0 * t);
        let b2 = cdiv(-q1c - disc, -2.0 * t);
        b = select(b2, b1, jcubic_e2res(a, b1, B) < jcubic_e2res(a, b2, B));
    }
    return vec4<f32>(a, b);
}

fn variation_jacobian_counterexample(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    let mode = u32(get_param(xform_id, variation_id, 0u));
    let scale = max(get_param(xform_id, variation_id, 1u), 1e-6);
    let ctr = vec2<f32>(get_param(xform_id, variation_id, 2u), get_param(xform_id, variation_id, 3u));
    let steps = i32(get_param(xform_id, variation_id, 4u));
    let w1 = get_param(xform_id, variation_id, 5u);
    let w2 = get_param(xform_id, variation_id, 6u);
    let w3 = get_param(xform_id, variation_id, 7u);
    let jul = vec4<f32>(
        get_param(xform_id, variation_id, 8u), get_param(xform_id, variation_id, 9u),
        get_param(xform_id, variation_id, 10u), get_param(xform_id, variation_id, 11u));
    let dc_mode = u32(get_param(xform_id, variation_id, 12u));
    let dc_scale = get_param(xform_id, variation_id, 13u);
    let cspeed = get_param(xform_id, variation_id, 14u);

    var ab = vec4<f32>(
        p / scale + ctr,
        vec2<f32>(get_state(xform_id, variation_id, 0u), get_state(xform_id, variation_id, 1u)));
    var creg = get_state(xform_id, variation_id, 2u);
    var k = 0u;
    for (var i = 0; i < steps; i = i + 1) {
        k = jcubic_pick(rng_nextf(rng), w1, w2, w3);
        ab = jcubic_map(ab, mode, k, jul);
        // Divergence guard: restart the hidden fiber coordinate (the
        // plotted point is respawned by the framework's bad-value recovery).
        if (!(dot(ab, ab) < 1e12)) {
            ab = vec4<f32>(ab.xy * 0.0, (rng_nextf(rng) - 0.5) * 0.6, (rng_nextf(rng) - 0.5) * 0.6);
        }
        if (dc_mode == 3u) {
            creg = mix(creg, (f32(k) + 0.5) / 3.0, cspeed);
        } else if (dc_mode == 4u) {
            let cc = vec2<f32>(2.0, 0.0) - 3.0 * ab.xy - ab.zw;
            creg = mix(creg, clamp(log(max(length(cc), 1e-20)), -4.0, 4.0), cspeed);
        }
    }
    set_state(xform_id, variation_id, 0u, ab.z);
    set_state(xform_id, variation_id, 1u, ab.w);

    if (dc_mode == 1u) {
        let c = vec2<f32>(2.0, 0.0) - 3.0 * ab.xy - ab.zw;
        *vc = fract(0.5 + 0.25 * dc_scale * log(max(length(c), 1e-20)));
    } else if (dc_mode == 2u) {
        *vc = fract((f32(k) + 0.5) / 3.0 * dc_scale);
    } else if (dc_mode == 3u) {
        set_state(xform_id, variation_id, 2u, creg);
        *vc = fract(creg * dc_scale);
    } else if (dc_mode == 4u) {
        set_state(xform_id, variation_id, 2u, creg);
        *vc = fract(0.5 + 0.25 * dc_scale * creg);
    } else if (dc_mode == 5u) {
        *vc = fract((atan2(ab.w, ab.z) * 0.15915494 + 0.5) * dc_scale);
    }
    return (ab.xy - ctr) * scale;
}
"#;

const WGSL_3D: &str = r#"
fn jcubic_cbrt(z: vec2<f32>) -> vec2<f32> {
    let r = length(z);
    if (r < 1e-30) { return vec2<f32>(0.0, 0.0); }
    let th = atan2(z.y, z.x) / 3.0;
    let rc = pow(r, 0.3333333333);
    return rc * vec2<f32>(cos(th), sin(th));
}

fn jcubic_v(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    let one = vec2<f32>(1.0, 0.0);
    let ap1 = a + one;
    return cmul(cmul(ap1, ap1), b) + cmul(cmul(a, a), 3.0 * a + vec2<f32>(4.0, 0.0));
}

// |E2(a, b) - B|^2 — residual of the second fiber equation, used to pick
// the correct root of the b-quadratic.
fn jcubic_e2res(a: vec2<f32>, b: vec2<f32>, B: vec2<f32>) -> f32 {
    let c = vec2<f32>(2.0, 0.0) - 3.0 * a - b;
    let e = cmul(cmul(c, c), cmul(a + vec2<f32>(1.0, 0.0), jcubic_v(a, b))) - B;
    return dot(e, e);
}

// Weighted 3-way branch pick.
fn jcubic_pick(r: f32, w1: f32, w2: f32, w3: f32) -> u32 {
    let tot = max(w1 + w2 + w3, 1e-9);
    let x = r * tot;
    if (x < w1) { return 0u; }
    if (x < w1 + w2) { return 1u; }
    return 2u;
}

// One application: forward G + j, or inverse branch k of G + j (solve
// G(p) = z - j; the fiber cubic's target just shifts, closed form intact).
fn jcubic_map(ab: vec4<f32>, mode: u32, k: u32, jul: vec4<f32>) -> vec4<f32> {
    let one = vec2<f32>(1.0, 0.0);
    if (mode == 1u) {
        // Forward: G(a,b) + j.
        let c = vec2<f32>(2.0, 0.0) - 3.0 * ab.xy - ab.zw;
        let V = jcubic_v(ab.xy, ab.zw);
        let a2 = cmul(c, ab.xy + 3.0 * V) + jul.xy;
        let b2 = cmul(cmul(c, c), cmul(ab.xy + one, V)) + jul.zw;
        return vec4<f32>(a2, b2);
    }
    // Inverse branch k: fiber cubic K u^3 + M u - 4B = 0 in u = 1 + a.
    let A = ab.xy - jul.xy;
    let B = ab.zw - jul.zw;
    let A2 = cmul(A, A);
    let A3 = cmul(A2, A);
    let K = A3 - A2 - 18.0 * cmul(A, B) + 27.0 * cmul(B, B) + 16.0 * B;
    let M = A2 - 12.0 * B;
    var u: vec2<f32>;
    if (dot(K, K) < 1e-24) {
        // Degenerate leading coefficient: cubic collapses to M u = 4B.
        if (dot(M, M) < 1e-24) { u = vec2<f32>(0.0, 0.0); }
        else { u = cdiv(4.0 * B, M); }
    } else {
        // Depressed Cardano: u^3 + p u + q = 0.
        let p = cdiv(M, K);
        let q = cdiv(-4.0 * B, K);
        let D = csqrt(0.25 * cmul(q, q) + cmul(cmul(p, p), p) / 27.0);
        var w = jcubic_cbrt(-0.5 * q + D);
        if (dot(w, w) < 1e-16) { w = jcubic_cbrt(-0.5 * q - D); }
        // Rotate to branch k: omega^k with omega = e^{2 pi i / 3}.
        var om = vec2<f32>(1.0, 0.0);
        if (k == 1u) { om = vec2<f32>(-0.5, 0.86602540); }
        else if (k == 2u) { om = vec2<f32>(-0.5, -0.86602540); }
        let wk = cmul(w, om);
        if (dot(wk, wk) < 1e-24) { u = vec2<f32>(0.0, 0.0); }
        else { u = wk - cdiv(p, 3.0 * wk); }
    }
    let a = u - one;
    // b from the E1 quadratic: q2 b^2 + q1 b + q0 = 0 with
    // s = a + 12a^2 + 9a^3, t = 3(1+a)^2, tm = 2 - 3a,
    // q2 = -t, q1 = tm*t - s, q0 = tm*s - A.
    let aa = cmul(a, a);
    let s = a + 12.0 * aa + 9.0 * cmul(aa, a);
    let t = 3.0 * cmul(a + one, a + one);
    let tm = vec2<f32>(2.0, 0.0) - 3.0 * a;
    let q1c = cmul(tm, t) - s;
    let q0c = cmul(tm, s) - A;
    var b: vec2<f32>;
    if (dot(t, t) < 1e-24) {
        // a = -1 (extraneous locus): quadratic degenerates to linear.
        b = cdiv(-q0c, q1c);
    } else {
        let disc = csqrt(cmul(q1c, q1c) + 4.0 * cmul(t, q0c));
        let b1 = cdiv(-q1c + disc, -2.0 * t);
        let b2 = cdiv(-q1c - disc, -2.0 * t);
        b = select(b2, b1, jcubic_e2res(a, b1, B) < jcubic_e2res(a, b2, B));
    }
    return vec4<f32>(a, b);
}

fn variation_jacobian_counterexample(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    let mode = u32(get_param(xform_id, variation_id, 0u));
    let scale = max(get_param(xform_id, variation_id, 1u), 1e-6);
    let ctr = vec2<f32>(get_param(xform_id, variation_id, 2u), get_param(xform_id, variation_id, 3u));
    let steps = i32(get_param(xform_id, variation_id, 4u));
    let w1 = get_param(xform_id, variation_id, 5u);
    let w2 = get_param(xform_id, variation_id, 6u);
    let w3 = get_param(xform_id, variation_id, 7u);
    let jul = vec4<f32>(
        get_param(xform_id, variation_id, 8u), get_param(xform_id, variation_id, 9u),
        get_param(xform_id, variation_id, 10u), get_param(xform_id, variation_id, 11u));
    let dc_mode = u32(get_param(xform_id, variation_id, 12u));
    let dc_scale = get_param(xform_id, variation_id, 13u);
    let cspeed = get_param(xform_id, variation_id, 14u);

    // Honest 4D: a rides (x, y), b rides (z, w).
    var ab = vec4<f32>(p.xy / scale + ctr, vec2<f32>(p.z, point_w) / scale);
    var creg = get_state(xform_id, variation_id, 2u);
    var k = 0u;
    for (var i = 0; i < steps; i = i + 1) {
        k = jcubic_pick(rng_nextf(rng), w1, w2, w3);
        ab = jcubic_map(ab, mode, k, jul);
        if (!(dot(ab, ab) < 1e12)) {
            ab = vec4<f32>(ab.xy * 0.0, (rng_nextf(rng) - 0.5) * 0.6, (rng_nextf(rng) - 0.5) * 0.6);
        }
        if (dc_mode == 3u) {
            creg = mix(creg, (f32(k) + 0.5) / 3.0, cspeed);
        } else if (dc_mode == 4u) {
            let cc = vec2<f32>(2.0, 0.0) - 3.0 * ab.xy - ab.zw;
            creg = mix(creg, clamp(log(max(length(cc), 1e-20)), -4.0, 4.0), cspeed);
        }
    }

    if (dc_mode == 1u) {
        let c = vec2<f32>(2.0, 0.0) - 3.0 * ab.xy - ab.zw;
        *vc = fract(0.5 + 0.25 * dc_scale * log(max(length(c), 1e-20)));
    } else if (dc_mode == 2u) {
        *vc = fract((f32(k) + 0.5) / 3.0 * dc_scale);
    } else if (dc_mode == 3u) {
        set_state(xform_id, variation_id, 2u, creg);
        *vc = fract(creg * dc_scale);
    } else if (dc_mode == 4u) {
        set_state(xform_id, variation_id, 2u, creg);
        *vc = fract(0.5 + 0.25 * dc_scale * creg);
    } else if (dc_mode == 5u) {
        *vc = fract((atan2(ab.w, ab.z) * 0.15915494 + 0.5) * dc_scale);
    }
    point_w_out = ab.w * scale;
    return vec3<f32>((ab.xy - ctr) * scale, ab.z * scale);
}
"#;
