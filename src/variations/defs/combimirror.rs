//! combimirror (Thomas Michels, with Brad Stefanov) — combined
//! vertical / horizontal / Z / point mirror with per-branch color shifts.
//!
//! A "replace"-style variation: JWildfire assigns `pVarTP = pAmount ·
//! affine` (not `+=`), then applies up to four independent random
//! mirror branches, each negating an axis about a movable centre and
//! optionally rotating the colour register. Because our dispatcher sums
//! `result += w · f(p)`, the body reads its own weight and returns the
//! JWF result pre-divided by `w` (idisc pattern) so the outer multiply
//! cancels — exact for the single-variation transforms combimirror is
//! used on.
//!
//! RNG order is fixed (point, vertical, horizontal, Z = 4 draws) and the
//! point branch fires on `random > pmirror/2` while the other three fire
//! on `random < mirror/2` — transcribed verbatim so imported flames
//! match JWildfire's stream.
//!
//! Source:
//!   - `output/variation-jwf-source/CombimirrorFunc.java`
//!
//! # Authors
//! - Thomas Michels
//! - Brad Stefanov

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Combined mirror — scales the affine input by the variation weight, then
/// applies four independent random mirror branches (point, vertical,
/// horizontal, Z), each negating its axis about a movable centre and
/// shifting the colour register by a per-branch amount. `*mirror`
/// parameters act as probabilities in `[0, 2]` (the branch fires with
/// probability `mirror/2`).
/// 
/// # Authors
/// - Thomas Michels
/// - Brad Stefanov
pub static COMBIMIRROR: VariationDef = VariationDef {
    name: "combimirror",
    aliases: &[],
    display_name: "Combimirror",
    category: VariationCategory::Full3D,
    // Phase-agnostic: honors JWildfire `fx_priority` (e.g. JWF-rando7 puts
    // combimirror in the pre phase via `combimirror_fx_priority="-1"`).
    // Default bucket is normal. `Feature::Replace` makes the moved-pre/post
    // emission assign (`temp = w·body`) rather than accumulate; combined
    // with the idisc body (returns the JWF value pre-divided by w) this is
    // exact in normal (replace-when-sole) and pre (`w·(jwf/w) = jwf`).
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::NeedsTransform, Feature::WritesColor, Feature::AlwaysZ, Feature::Replace],
    parameters: &[
        param!("vmirror", "V Mirror", unlimited_float, 1.0, 0.0, 2.0, "Vertical-mirror probability (fires when a random < vmirror/2). Negates X about `vmove`."),
        param!("vmove", "V Move", unlimited_float, 0.05, -10.0, 10.0, "X offset added after the vertical mirror: `x → -x + vmove`."),
        param!("hmirror", "H Mirror", unlimited_float, 0.5, 0.0, 2.0, "Horizontal-mirror probability (fires when a random < hmirror/2). Negates Y about `hmove`."),
        param!("hmove", "H Move", unlimited_float, 0.35, -10.0, 10.0, "Y offset added after the horizontal mirror: `y → -y + hmove`."),
        param!("zmirror", "Z Mirror", unlimited_float, 0.0, 0.0, 2.0, "Z-mirror probability (fires when a random < zmirror/2). Negates Z about `zmove`. 3D only."),
        param!("zmove", "Z Move", unlimited_float, 0.0, -10.0, 10.0, "Z offset added after the Z mirror: `z → -z + zmove`."),
        param!("pmirror", "P Mirror", unlimited_float, 0.0, 0.0, 2.0, "Point-mirror probability — note this branch fires when a random > pmirror/2 (so 0 = always, 2 = never). Negates X and Y about (pmovex, pmovey)."),
        param!("pmovex", "P Move X", unlimited_float, 0.05, -10.0, 10.0, "X offset added after the point mirror: `x → -x + pmovex`."),
        param!("pmovey", "P Move Y", unlimited_float, 0.0, -10.0, 10.0, "Y offset added after the point mirror: `y → -y + pmovey`."),
        param!("vcolorshift", "V Color Shift", unlimited_float, 0.0, -1.0, 1.0, "Colour-register shift applied when the vertical mirror fires. Visible colour requires the transform's Direct Color slider > 0."),
        param!("hcolorshift", "H Color Shift", unlimited_float, 0.0, -1.0, 1.0, "Colour-register shift applied when the horizontal mirror fires."),
        param!("zcolorshift", "Z Color Shift", unlimited_float, 0.0, -1.0, 1.0, "Colour-register shift applied when the Z mirror fires."),
        param!("pcolorshift", "P Color Shift", unlimited_float, 0.0, -1.0, 1.0, "Colour-register shift applied when the point mirror fires."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_combimirror(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    let vmirror = get_param(xform_id, variation_id, 0u);
    let vmove = get_param(xform_id, variation_id, 1u);
    let hmirror = get_param(xform_id, variation_id, 2u);
    let hmove = get_param(xform_id, variation_id, 3u);
    let pmirror = get_param(xform_id, variation_id, 6u);
    let pmovex = get_param(xform_id, variation_id, 7u);
    let pmovey = get_param(xform_id, variation_id, 8u);
    let vcolorshift = get_param(xform_id, variation_id, 9u);
    let hcolorshift = get_param(xform_id, variation_id, 10u);
    let pcolorshift = get_param(xform_id, variation_id, 12u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    // JWF assigns pVarTP = pAmount * affine (replace).
    var px = w * p.x;
    var py = w * p.y;

    // Point mirror — fires on random > pmirror/2.
    if (rng_nextf(rng) > pmirror * 0.5) {
        px = -px + pmovex;
        py = -py + pmovey;
        let c = *vc + pcolorshift;
        *vc = c - trunc(c);
    }
    // Vertical mirror.
    if (rng_nextf(rng) < vmirror * 0.5) {
        px = -px + vmove;
        let c = *vc + vcolorshift;
        *vc = c - trunc(c);
    }
    // Horizontal mirror.
    if (rng_nextf(rng) < hmirror * 0.5) {
        py = -py + hmove;
        let c = *vc + hcolorshift;
        *vc = c - trunc(c);
    }
    // Z mirror still consumes a random draw for stream parity, even in 2D.
    let _z_draw = rng_nextf(rng);

    // Pre-divide by w so the dispatcher's outer `w *` restores the
    // replace-style JWF values.
    return vec2<f32>(px * inv_w, py * inv_w);
}
"#,
    wgsl_3d: r#"
fn variation_combimirror(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    let vmirror = get_param(xform_id, variation_id, 0u);
    let vmove = get_param(xform_id, variation_id, 1u);
    let hmirror = get_param(xform_id, variation_id, 2u);
    let hmove = get_param(xform_id, variation_id, 3u);
    let zmirror = get_param(xform_id, variation_id, 4u);
    let zmove = get_param(xform_id, variation_id, 5u);
    let pmirror = get_param(xform_id, variation_id, 6u);
    let pmovex = get_param(xform_id, variation_id, 7u);
    let pmovey = get_param(xform_id, variation_id, 8u);
    let vcolorshift = get_param(xform_id, variation_id, 9u);
    let hcolorshift = get_param(xform_id, variation_id, 10u);
    let zcolorshift = get_param(xform_id, variation_id, 11u);
    let pcolorshift = get_param(xform_id, variation_id, 12u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    // JWF: z = pAmount*affine.xy, z2 = pAmount*affine.z (z2.re), assigned.
    var px = w * p.x;
    var py = w * p.y;
    var pz = w * p.z;

    // Point mirror — fires on random > pmirror/2.
    if (rng_nextf(rng) > pmirror * 0.5) {
        px = -px + pmovex;
        py = -py + pmovey;
        let c = *vc + pcolorshift;
        *vc = c - trunc(c);
    }
    // Vertical mirror.
    if (rng_nextf(rng) < vmirror * 0.5) {
        px = -px + vmove;
        let c = *vc + vcolorshift;
        *vc = c - trunc(c);
    }
    // Horizontal mirror.
    if (rng_nextf(rng) < hmirror * 0.5) {
        py = -py + hmove;
        let c = *vc + hcolorshift;
        *vc = c - trunc(c);
    }
    // Z mirror.
    if (rng_nextf(rng) < zmirror * 0.5) {
        pz = -pz + zmove;
        let c = *vc + zcolorshift;
        *vc = c - trunc(c);
    }

    return vec3<f32>(px * inv_w, py * inv_w, pz * inv_w);
}
"#,
};
