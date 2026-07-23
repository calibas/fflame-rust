//! `jacobian_cubic` — inverse-branch IFS of the Jacobian-conjecture
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
//! every reduction identity below). It is weighted-homogeneous under
//! `(x,y,z) ↦ (λ⁻¹x, λy, λ²z)`, so the whole dynamics descends to the
//! weight-0 coordinates `a = xy`, `b = x²z`: with `c = 2−3a−b` and
//! `V = (1+a)²b + a²(3a+4)`,
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
//! G) is a fractal rim near a = −1. Exists *because* the map fails
//! injectivity: were the Jacobian conjecture true here, one branch and
//! a point attractor.
//!
//! **Forward mode**: G itself (expanding; mix with contracting affines
//! like any polynomial variation).
//!
//! State is (a,b) ∈ ℂ² = 4 real dims. The 3D body carries b honestly in
//! (z, w) (`Feature::NeedsW`, the honest-4D pattern of `polychoron` /
//! `honeycomb4d`); the 2D body rides b in per-thread state slots seeded
//! randomly at spawn. `log|c|` — the Birkhoff cocycle of the lost skew
//! coordinate (`log|x|` accumulates as Σ log|c|) — is the natural
//! direct-color channel.
//!
//! Original construction for this project; map due to the July 2026
//! Jacobian-conjecture counterexample announcement.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

pub static JACOBIAN_CUBIC: VariationDef = VariationDef {
    name: "jacobian_cubic",
    aliases: &[],
    display_name: "Jacobian Cubic",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::NeedsW, Feature::WritesColor, Feature::AlwaysZ],
    // Slots 0,1: the b coordinate (re, im) for the 2D body (the 3D body
    // carries b in (z, w) instead and leaves these untouched).
    state_count: 2,
    // Seed b randomly in a small disk so threads explore distinct fibers.
    wgsl_state_init: Some(
        "        let r1 = rng_nextf(&rng);\n\
         \x20       let r2 = rng_nextf(&rng);\n\
         \x20       set_state(xform_id, variation_id, 0u, (r1 - 0.5) * 0.6);\n\
         \x20       set_state(xform_id, variation_id, 1u, (r2 - 0.5) * 0.6);",
    ),
    parameters: &[
        param!("mode", "Mode", enum, 0, &["Inverse IFS", "Forward"], "Inverse IFS: chaos game over the three closed-form inverse branches (random Cardano root per call) — converges onto the fractal attractor, each branch contracting area by 1/|2c²|. Forward: the degree-4/6 endomorphism itself — expanding, so blend it with contracting affines like any polynomial variation."),
        param!("scale", "Scale", float, 1.0, 0.05, 4.0, "Similarity conjugation: world point = math point × scale. The attractor rim sits near a = −1 with span ≈ 0.7, so Scale 1 with a small pan left frames it."),
        param!("dc_mode", "Color Mode", enum, 0, &["Off", "Log C", "Branch"], "Direct-color source (needs the transform's Direct Color > 0). Log C: log|c| at the new point — the Birkhoff cocycle of the skew coordinate (Σ log|c| = log|x| of the lost third dimension) and the local contraction rate (det DG = 2c²). Branch: which of the three inverse branches was taken (Inverse mode)."),
        param!("dc_scale", "Color Scale", float, 1.0, 0.1, 8.0, "Palette-index multiplier for the color modes."),
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

fn jcubic_map(ab: vec4<f32>, mode: u32, k: u32) -> vec4<f32> {
    let A = ab.xy;
    let B = ab.zw;
    let one = vec2<f32>(1.0, 0.0);
    if (mode == 1u) {
        // Forward: G(a,b) = (c(a+3V), c^2(1+a)V).
        let c = vec2<f32>(2.0, 0.0) - 3.0 * A - B;
        let V = jcubic_v(A, B);
        let a2 = cmul(c, A + 3.0 * V);
        let b2 = cmul(cmul(c, c), cmul(A + one, V));
        return vec4<f32>(a2, b2);
    }
    // Inverse branch k: fiber cubic K u^3 + M u - 4B = 0 in u = 1 + a.
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
fn variation_jacobian_cubic(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    let mode = u32(get_param(xform_id, variation_id, 0u));
    let scale = max(get_param(xform_id, variation_id, 1u), 1e-6);
    let dc_mode = u32(get_param(xform_id, variation_id, 2u));
    let dc_scale = get_param(xform_id, variation_id, 3u);

    let a_in = p / scale;
    let b_in = vec2<f32>(get_state(xform_id, variation_id, 0u), get_state(xform_id, variation_id, 1u));
    let k = min(u32(rng_nextf(rng) * 3.0), 2u);
    var ab = jcubic_map(vec4<f32>(a_in, b_in), mode, k);
    // Divergence guard: restart the hidden fiber coordinate (the plotted
    // point is respawned by the framework's bad-value recovery).
    if (!(dot(ab, ab) < 1e12)) {
        ab = vec4<f32>(a_in * 0.1, (rng_nextf(rng) - 0.5) * 0.6, (rng_nextf(rng) - 0.5) * 0.6);
    }
    set_state(xform_id, variation_id, 0u, ab.z);
    set_state(xform_id, variation_id, 1u, ab.w);

    if (dc_mode == 1u) {
        let c = vec2<f32>(2.0, 0.0) - 3.0 * ab.xy - ab.zw;
        *vc = fract(0.5 + 0.25 * dc_scale * log(max(length(c), 1e-20)));
    } else if (dc_mode == 2u) {
        *vc = fract((f32(k) + 0.5) / 3.0 * dc_scale);
    }
    return ab.xy * scale;
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

fn jcubic_map(ab: vec4<f32>, mode: u32, k: u32) -> vec4<f32> {
    let A = ab.xy;
    let B = ab.zw;
    let one = vec2<f32>(1.0, 0.0);
    if (mode == 1u) {
        // Forward: G(a,b) = (c(a+3V), c^2(1+a)V).
        let c = vec2<f32>(2.0, 0.0) - 3.0 * A - B;
        let V = jcubic_v(A, B);
        let a2 = cmul(c, A + 3.0 * V);
        let b2 = cmul(cmul(c, c), cmul(A + one, V));
        return vec4<f32>(a2, b2);
    }
    // Inverse branch k: fiber cubic K u^3 + M u - 4B = 0 in u = 1 + a.
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
fn variation_jacobian_cubic(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    let mode = u32(get_param(xform_id, variation_id, 0u));
    let scale = max(get_param(xform_id, variation_id, 1u), 1e-6);
    let dc_mode = u32(get_param(xform_id, variation_id, 2u));
    let dc_scale = get_param(xform_id, variation_id, 3u);

    // Honest 4D: a rides (x, y), b rides (z, w).
    let a_in = p.xy / scale;
    let b_in = vec2<f32>(p.z, point_w) / scale;
    let k = min(u32(rng_nextf(rng) * 3.0), 2u);
    var ab = jcubic_map(vec4<f32>(a_in, b_in), mode, k);
    if (!(dot(ab, ab) < 1e12)) {
        ab = vec4<f32>(a_in * 0.1, (rng_nextf(rng) - 0.5) * 0.6, (rng_nextf(rng) - 0.5) * 0.6);
    }

    if (dc_mode == 1u) {
        let c = vec2<f32>(2.0, 0.0) - 3.0 * ab.xy - ab.zw;
        *vc = fract(0.5 + 0.25 * dc_scale * log(max(length(c), 1e-20)));
    } else if (dc_mode == 2u) {
        *vc = fract((f32(k) + 0.5) / 3.0 * dc_scale);
    }
    point_w_out = ab.w * scale;
    return vec3<f32>(ab.xy, ab.z) * scale;
}
"#;
