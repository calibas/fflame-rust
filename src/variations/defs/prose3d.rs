//! pRose3D (Larry Berlin) — 3D polar-rose base shape with a per-point
//! Z "waggle/wig" profile and an optional transparent mirror reflection.
//!
//! Draws a polar rose `r = length·cos(k·θ + c)` in XY, then lifts each
//! point along Z by a blend of curve-driven "waggle" terms plus a radial
//! `sin(wig)` ripple and a `dist` offset. With `opt ≤ 0` (and `opt3 ≥ 0`)
//! a fraction of points (`transp²`) are drawn as a downward, `refSc`-scaled
//! mirror reflection.
//!
//! Faithful port of `transform()` from
//!   `output/variation-jwf-source/PRose3DFunc.java`
//! Quirks preserved: the `crvsc` field is exposed under the XML/param name
//! **`srvsc`** (JWildfire's own spelling); two RNG draws per call in the
//! order ghostPrep → posNeg; `pVarTP.z` is written unconditionally
//! (`Feature::AlwaysZ`). `_cycle = 2π/k` (JWF's `init()`) is folded inline.
//!
//! # Authors
//! - Larry Berlin

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

pub static PROSE3D: VariationDef = VariationDef {
    name: "pRose3D",
    aliases: &[],
    display_name: "PRose 3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::AlwaysZ],
    parameters: &[
        param!("l", "Length", unlimited_float, 1.0, 0.0, 3.0, "Rose length / overall scale (`length`). 0 is clamped to a tiny value."),
        param!("k", "Petals", unlimited_float, 6.0, -15.0, 15.0, "Petal count `k` (the polar-rose frequency); may be fractional. Sign flips petal parity."),
        param!("c", "Constant", unlimited_float, 0.0, -3.14159265, 3.14159265, "Angular phase constant added inside `cos(k·θ + c)`."),
        param!("z1", "Scale Z1", unlimited_float, 1.0, -2.0, 2.0, "Z scale of the main (non-reflected) petals."),
        param!("z2", "Scale Z2", unlimited_float, 1.0, -2.0, 2.0, "Z scale of the reflected (ghost) petals."),
        param!("refSc", "Reflection Scale", unlimited_float, 1.0, 0.0, 2.0, "XY scale applied to reflected points."),
        param!("opt", "Opt", unlimited_float, 1.0, -2.0, 2.0, "Waggle blend selector (`smooth12 = |opt|`, capped at 2). Its sign sets the Z direction; `opt > 0` also disables reflections."),
        param!("optSc", "Opt Scale", unlimited_float, 1.0, 0.0, 2.0, "Frequency scale on the curve `sin()` terms (`opScale = |optSc|`)."),
        param!("opt3", "Opt3", unlimited_float, 0.0, -1.0, 1.0, "Blends in the `wag3` waggle term (`smooth3 = |opt3|`, capped at 1). Negative forces the down direction."),
        param!("transp", "Transparency", unlimited_float, 0.5, 0.0, 1.0, "Reflection probability — a point reflects when a random < `transp²` (and `opt ≤ 0`)."),
        param!("dist", "Distance", unlimited_float, 1.0, 0.0, 3.0, "Constant Z offset added to every point."),
        param!("wagsc", "Wag Scale", unlimited_float, 0.0, -2.5, 2.5, "Radial weight of the waggle terms."),
        param!("srvsc", "Curve Scale", unlimited_float, 0.0, 0.0, 3.0, "Weight of the curve3/curve2 cosine/sine terms in the waggle (JWF param name `srvsc`)."),
        param!("f", "Frequency", unlimited_float, 3.0, 0.0, 6.0, "Perimeter ripple frequency (`frequency = f·2π`)."),
        param!("wigsc", "Wig Scale", unlimited_float, 0.0, -2.5, 2.5, "Amplitude of the radial `sin(wig)` Z ripple."),
        param!("offset", "Offset", unlimited_float, 0.0, -2.0, 2.0, "Phase offset added to the ripple argument (`+ offset·cycle`)."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_pRose3D(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    // 2D mode: XY only (no Z output). The two RNG draws and the
    // reflection branch are kept so the stream + shape match the 3D body.
    let l = get_param(xform_id, variation_id, 0u);
    let k = get_param(xform_id, variation_id, 1u);
    let c = get_param(xform_id, variation_id, 2u);
    let refSc = get_param(xform_id, variation_id, 5u);
    let opt = get_param(xform_id, variation_id, 6u);
    let opt3 = get_param(xform_id, variation_id, 8u);
    let transp = get_param(xform_id, variation_id, 9u);

    var length = l;
    if (length == 0.0) { length = 0.000001; }
    let petalsSign = select(1.0, -1.0, k < 0.0);
    var numPetals = k;
    if (abs(numPetals) < 0.0001) { numPetals = 0.0001 * petalsSign; }

    let th = atan2(p.y, p.x);
    let sth = sin(th);
    let cth = cos(th);

    var optDir = select(1.0, -1.0, opt < 0.0);
    if (opt3 < 0.0) { optDir = -1.0; }

    // RNG order: ghostPrep first, then posNeg (matches JWF).
    let ghostPrep = rng_nextf(rng);
    let posNeg_neg = rng_nextf(rng) < 0.5;

    var ghost = transp * transp;
    if (optDir > 0.0) { ghost = 0.0; }

    let rose = length * cos(numPetals * th + c);
    if (ghostPrep < ghost && posNeg_neg) {
        return vec2<f32>(0.5 * refSc * rose * cth, 0.5 * refSc * rose * sth);
    }
    return vec2<f32>(0.5 * rose * cth, 0.5 * rose * sth);
}
"#,
    wgsl_3d: r#"
fn variation_pRose3D(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    const PI: f32 = 3.14159265359;
    const M_2PI: f32 = 6.28318530718;
    let l = get_param(xform_id, variation_id, 0u);
    let k = get_param(xform_id, variation_id, 1u);
    let c = get_param(xform_id, variation_id, 2u);
    let z1 = get_param(xform_id, variation_id, 3u);
    let z2 = get_param(xform_id, variation_id, 4u);
    let refSc = get_param(xform_id, variation_id, 5u);
    let opt = get_param(xform_id, variation_id, 6u);
    let optSc = get_param(xform_id, variation_id, 7u);
    let opt3 = get_param(xform_id, variation_id, 8u);
    let transp = get_param(xform_id, variation_id, 9u);
    let dist = get_param(xform_id, variation_id, 10u);
    let wagsc = get_param(xform_id, variation_id, 11u);
    let srvsc = get_param(xform_id, variation_id, 12u);
    let f = get_param(xform_id, variation_id, 13u);
    let wigsc = get_param(xform_id, variation_id, 14u);
    let offset = get_param(xform_id, variation_id, 15u);

    var length = l;
    if (length == 0.0) { length = 0.000001; }
    let petalsSign = select(1.0, -1.0, k < 0.0);
    var numPetals = k;
    if (abs(numPetals) < 0.0001) { numPetals = 0.0001 * petalsSign; }

    let rad = sqrt(p.x * p.x + p.y * p.y);
    let curve1 = rad / length;
    let curve2 = (rad / length) * (rad / length);
    let curve3 = (rad - length * 0.5) / length;
    let curve4 = curve2 * curve2;

    var smooth12 = abs(opt);
    if (smooth12 > 2.0) { smooth12 = 2.0; }
    var smooth3 = abs(opt3);
    if (smooth3 > 1.0) { smooth3 = 1.0; }
    let opScale = abs(optSc);
    let antiOpt1 = 2.0 - smooth12;
    let wagScale = wagsc;
    let curveScale = srvsc;
    let frequency = f * M_2PI;
    let wigScale = wigsc * 0.5;

    let th = atan2(p.y, p.x);
    let sth = sin(th);
    let cth = cos(th);

    var optDir = select(1.0, -1.0, opt < 0.0);
    if (opt3 < 0.0) { optDir = -1.0; }

    // RNG order: ghostPrep first, then posNeg (matches JWF).
    let ghostPrep = rng_nextf(rng);
    let posNeg_neg = rng_nextf(rng) < 0.5;

    // _cycle = 2*pi / k (JWF init(); k==0 nudged like JWF).
    let kc = select(k, k + 0.000001, k == 0.0);
    let cycle = M_2PI / kc;
    let pang = th / cycle;
    let wig = pang * frequency * 0.5 + offset * cycle;

    var wag: f32;
    var wag3: f32;
    if (optDir < 0.0) {
        wag = sin(curve1 * PI * opScale) + wagScale * 0.4 * rad + curveScale * 0.5 * sin(curve2 * PI);
        wag3 = sin(curve4 * PI * opScale) + wagScale * rad * rad * 0.4 + curveScale * 0.5 * cos(curve3 * PI);
    } else {
        wag = sin(curve1 * PI * opScale) + wagScale * 0.4 * rad + curveScale * 0.5 * cos(curve3 * PI);
        wag3 = sin(curve4 * PI * opScale) + wagScale * rad * rad * 0.4 + curveScale * 0.5 * sin(curve2 * PI);
    }
    let wag2 = sin(curve2 * PI * opScale) + wagScale * 0.4 * rad + curveScale * 0.5 * cos(curve3 * PI);

    // wag12: wag for smooth12<=1, interpolate wag->wag2 over (1,2].
    var wag12 = wag;
    if (smooth12 > 1.0) {
        wag12 = wag2 * (1.0 - antiOpt1) + wag * antiOpt1;
    }
    var waggle = wag12;
    if (smooth3 > 0.0) {
        waggle = wag12 * (1.0 - smooth3) + wag3 * smooth3;
    }

    var ghost = transp * transp;
    if (optDir > 0.0) { ghost = 0.0; }

    let rose = length * cos(numPetals * th + c);
    let zwig = (rad * 0.5) * (rad * 0.5) * sin(wig) * wigScale;

    if (ghostPrep < ghost && posNeg_neg) {
        // Reflection: refSc-scaled XY, downward Z with z2.
        return vec3<f32>(
            0.5 * refSc * rose * cth,
            0.5 * refSc * rose * sth,
            -0.5 * ((z2 * waggle + zwig) + dist),
        );
    }
    return vec3<f32>(
        0.5 * rose * cth,
        0.5 * rose * sth,
        0.5 * ((z1 * waggle + zwig) + dist),
    );
}
"#,
};
