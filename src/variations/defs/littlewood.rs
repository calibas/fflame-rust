//! `littlewood` — Littlewood-polynomial IFS attractor (Barnsley–
//! Harrington / Bandt / Calegari–Koch–Walker).
//!
//! Littlewood polynomials have all coefficients ±1; the famous fractal
//! portraits plot the roots of all 2ⁿ of them. The chaos-game-native
//! reformulation: λ is a root of a ±1 power series **iff 0 lies in the
//! attractor A(λ)** of the two-map affine IFS
//!
//! ```text
//! f₊(z) = (z + 1)/λ        f₋(z) = (z − 1)/λ
//! ```
//!
//! (a ±1 series Σaᵢλ⁻ⁱ⁻¹... every choice of sign sequence is one orbit
//! of the pair, so the attractor is the closure of all coefficient
//! sequences at once). This variation renders A(λ) — the *dynamical
//! space* side of the picture. The brute-force root cloud is the
//! *parameter space* side ({λ : 0 ∈ A(λ)}, Barnsley–Harrington's
//! "Mandelbrot set for pairs of linear maps"); it is root-finding, not
//! a chaos game, and deliberately out of scope — explore it by
//! animating λ and watching when the attractor captures the origin.
//!
//! Contracting for |λ| > 1 (ratio 1/|λ|); the root portrait's λ ↔ 1/λ
//! reciprocal symmetry maps this form onto the `λz ± 1` convention
//! used elsewhere in the literature. Landmarks: λ = 1+i is the twin
//! dragon (the default); real λ collapses to Bernoulli-convolution
//! Cantor sets / intervals on the axis (golden ratio λ is the classic
//! singular case); |λ| near 1 fills densely; larger |λ| shatters into
//! dust — the connected/disconnected boundary as λ varies is Bandt's
//! connectedness locus M.
//!
//! `coeffs` extends the digit set per the same literature: Borwein
//! polynomials use {0, ±1} (three maps), and the Gaussian set
//! {±1, ±i} (four maps) gives the quaternion-free complex analogue.
//! Branch weights bias the digit distribution — unequal weights render
//! the multifractal measure instead of the uniform one, and zeroing a
//! digit selects sub-self-affine subsets.
//!
//! Purely a planar complex IFS; the 3D body applies it in xy and
//! passes z through.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

pub static LITTLEWOOD: VariationDef = VariationDef {
    name: "littlewood",
    aliases: &[],
    display_name: "Littlewood",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::WritesColor],
    // Slot 0: color register for the Branch Blend mode.
    state_count: 1,
    wgsl_state_init: None,
    parameters: &[
        param!("lambda_re", "Lambda re", unlimited_float, 1.0, -3.0, 3.0, "Real part of λ in the digit maps z ↦ (z + digit)/λ. Contracting (a true attractor) for |λ| > 1, ratio 1/|λ|. Default λ = 1+i is the twin dragon; real λ gives Bernoulli-convolution Cantor sets on the axis; |λ| → 1 fills densely, large |λ| shatters to dust. λ is a root of a ±1 power series exactly when the attractor contains the origin — animate λ to walk the Littlewood root portrait."),
        param!("lambda_im", "Lambda im", unlimited_float, 1.0, -3.0, 3.0, "Imaginary part of λ."),
        param!("coeffs", "Coefficients", enum, 0, &["±1 (Littlewood)", "0, ±1 (Borwein)", "±1, ±i (Gaussian)"], "The polynomial coefficient set = the IFS digit set. ±1 is the Littlewood pair (2 maps); 0,±1 the Borwein triple (3 maps); ±1,±i the Gaussian quadruple (4 maps, 4-fold symmetric attractors)."),
        param!("w1", "Digit 1", float, 1.0, 0.0, 1.0, "Selection weight of digit +1. Unequal weights render the multifractal (biased-coin) measure on the same attractor; zeroing a digit selects a sub-self-affine subset."),
        param!("w2", "Digit 2", float, 1.0, 0.0, 1.0, "Selection weight of digit −1."),
        param!("w3", "Digit 3", float, 1.0, 0.0, 1.0, "Selection weight of digit 0 (Borwein) or +i (Gaussian). Ignored by the ±1 set."),
        param!("w4", "Digit 4", float, 1.0, 0.0, 1.0, "Selection weight of digit −i (Gaussian only)."),
        param!("dc_mode", "Color Mode", enum, 0, &["Off", "Branch", "Branch Blend"], "Direct-color source (needs the transform's Direct Color > 0). Branch: which digit map was applied this call. Branch Blend: persistent register pulled toward each digit's palette slot — colors the attractor by its digit itinerary (the binary/ternary address), the classic two-tone dragon dissection."),
        param!("dc_scale", "Color Scale", float, 1.0, 0.1, 8.0, "Palette-index multiplier for the color modes."),
        param!("color_speed", "Color Speed", float, 0.3, 0.01, 1.0, "Blend rate of the Branch Blend register: low = deep address history (coarse self-affine pieces), high = recent digits only (fine dissection)."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

// Digit table + weighted pick, duplicated into both bodies (one
// compiles per flame).
const WGSL_2D: &str = r#"
fn lw_digit(dset: u32, k: u32) -> vec2<f32> {
    if (dset == 1u) {
        // Borwein: +1, -1, 0
        if (k == 0u) { return vec2<f32>(1.0, 0.0); }
        if (k == 1u) { return vec2<f32>(-1.0, 0.0); }
        return vec2<f32>(0.0, 0.0);
    }
    if (dset == 2u) {
        // Gaussian: +1, -1, +i, -i
        if (k == 0u) { return vec2<f32>(1.0, 0.0); }
        if (k == 1u) { return vec2<f32>(-1.0, 0.0); }
        if (k == 2u) { return vec2<f32>(0.0, 1.0); }
        return vec2<f32>(0.0, -1.0);
    }
    // Littlewood: +1, -1
    if (k == 0u) { return vec2<f32>(1.0, 0.0); }
    return vec2<f32>(-1.0, 0.0);
}

fn lw_pick(r: f32, w1: f32, w2: f32, w3: f32, w4: f32, cnt: u32) -> u32 {
    var tot = w1 + w2;
    if (cnt >= 3u) { tot = tot + w3; }
    if (cnt >= 4u) { tot = tot + w4; }
    let x = r * max(tot, 1e-9);
    if (x < w1) { return 0u; }
    if (x < w1 + w2) { return 1u; }
    if (cnt >= 4u && x >= w1 + w2 + w3) { return 3u; }
    return 2u;
}

fn variation_littlewood(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    let lam = vec2<f32>(get_param(xform_id, variation_id, 0u), get_param(xform_id, variation_id, 1u));
    let dset = u32(get_param(xform_id, variation_id, 2u));
    let w1 = get_param(xform_id, variation_id, 3u);
    let w2 = get_param(xform_id, variation_id, 4u);
    let w3 = get_param(xform_id, variation_id, 5u);
    let w4 = get_param(xform_id, variation_id, 6u);
    let dc_mode = u32(get_param(xform_id, variation_id, 7u));
    let dc_scale = get_param(xform_id, variation_id, 8u);
    let cspeed = get_param(xform_id, variation_id, 9u);

    var cnt = 2u;
    if (dset == 1u) { cnt = 3u; }
    else if (dset == 2u) { cnt = 4u; }
    let k = lw_pick(rng_nextf(rng), w1, w2, w3, w4, cnt);
    let out = cdiv(p + lw_digit(dset, k), lam);

    if (dc_mode == 1u) {
        *vc = fract((f32(k) + 0.5) / f32(cnt) * dc_scale);
    } else if (dc_mode == 2u) {
        var creg = get_state(xform_id, variation_id, 0u);
        creg = mix(creg, (f32(k) + 0.5) / f32(cnt), cspeed);
        set_state(xform_id, variation_id, 0u, creg);
        *vc = fract(creg * dc_scale);
    }
    return out;
}
"#;

const WGSL_3D: &str = r#"
fn lw_digit(dset: u32, k: u32) -> vec2<f32> {
    if (dset == 1u) {
        if (k == 0u) { return vec2<f32>(1.0, 0.0); }
        if (k == 1u) { return vec2<f32>(-1.0, 0.0); }
        return vec2<f32>(0.0, 0.0);
    }
    if (dset == 2u) {
        if (k == 0u) { return vec2<f32>(1.0, 0.0); }
        if (k == 1u) { return vec2<f32>(-1.0, 0.0); }
        if (k == 2u) { return vec2<f32>(0.0, 1.0); }
        return vec2<f32>(0.0, -1.0);
    }
    if (k == 0u) { return vec2<f32>(1.0, 0.0); }
    return vec2<f32>(-1.0, 0.0);
}

fn lw_pick(r: f32, w1: f32, w2: f32, w3: f32, w4: f32, cnt: u32) -> u32 {
    var tot = w1 + w2;
    if (cnt >= 3u) { tot = tot + w3; }
    if (cnt >= 4u) { tot = tot + w4; }
    let x = r * max(tot, 1e-9);
    if (x < w1) { return 0u; }
    if (x < w1 + w2) { return 1u; }
    if (cnt >= 4u && x >= w1 + w2 + w3) { return 3u; }
    return 2u;
}

fn variation_littlewood(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    let lam = vec2<f32>(get_param(xform_id, variation_id, 0u), get_param(xform_id, variation_id, 1u));
    let dset = u32(get_param(xform_id, variation_id, 2u));
    let w1 = get_param(xform_id, variation_id, 3u);
    let w2 = get_param(xform_id, variation_id, 4u);
    let w3 = get_param(xform_id, variation_id, 5u);
    let w4 = get_param(xform_id, variation_id, 6u);
    let dc_mode = u32(get_param(xform_id, variation_id, 7u));
    let dc_scale = get_param(xform_id, variation_id, 8u);
    let cspeed = get_param(xform_id, variation_id, 9u);

    var cnt = 2u;
    if (dset == 1u) { cnt = 3u; }
    else if (dset == 2u) { cnt = 4u; }
    let k = lw_pick(rng_nextf(rng), w1, w2, w3, w4, cnt);
    let out = cdiv(p.xy + lw_digit(dset, k), lam);

    if (dc_mode == 1u) {
        *vc = fract((f32(k) + 0.5) / f32(cnt) * dc_scale);
    } else if (dc_mode == 2u) {
        var creg = get_state(xform_id, variation_id, 0u);
        creg = mix(creg, (f32(k) + 0.5) / f32(cnt), cspeed);
        set_state(xform_id, variation_id, 0u, creg);
        *vc = fract(creg * dc_scale);
    }
    return vec3<f32>(out, p.z);
}
"#;
