//! cut_sqcir (Jesus Sosa, 2019) — JWildfire "cut_*" procedural-stencil
//! family.
//!
//! A "cut" variation is a shadertoy-style mask: it samples a point (the
//! affine input in `mode=0`, or a fresh random point in `mode=1`), tests
//! it against a procedural shape at `uv = point·zoom`, and HIDES points
//! that fall outside (or inside, when `invert`) the shape via the
//! `Feature::CanHide` mechanism (JWildfire's `pVarTP.doHide`). Kept points
//! plot at their sampled position. Output is a replace (`pVarTP.x =
//! pAmount·x`).
//!
//! This one's shape is a superellipse (Lamé curve): the unit region
//! `|u−v|^power + |u+v|^power ≤ 1`. With `power=2` it's a circle (rotated
//! 45°); as `power→0` it pinches toward a plus/asterisk; `power` is clamped
//! to `[0, 2]` so it never exceeds the square. Transcribed from
//! `output/variation-jwf-source/CutSqCirFunc.java` (CPU + GPU bodies agree
//! except the CPU's redundant early `return` in the invert branch — the GPU
//! code, which we follow, sets `(0,0)` in both branches; the point is hidden
//! either way).
//!
//! # Authors
//! - Jesus Sosa (cut_* family)

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Square-to-circle morph tiling stencil mask.
///
/// # Authors
/// - Jesus Sosa
pub static CUT_SQCIR: VariationDef = VariationDef {
    name: "cut_sqcir",
    aliases: &[],
    display_name: "Cut SqCir",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::Replace, Feature::CanHide],
    parameters: &[
        param!("mode", "Mode", enum, 1, &["Affine Input", "Random Point"], "Affine Input samples the incoming point; Random Point samples a fresh uniform point each call (the usual stencil mode)."),
        param!("zoom", "Zoom", float, 2.0, 0.1, 50.0, "Pattern scale: `uv = point·zoom`."),
        param!("invert", "Invert", bool, false, "Invert the cut — keep what would be hidden and vice versa."),
        param!("power", "Power", float, 2.0, 0.0, 2.0, "Superellipse exponent: 2 = circle, →0 pinches toward a plus/asterisk."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_cut_sqcir(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec2<f32> {
    let mode = i32(get_param(xform_id, variation_id, 0u));
    let zoom = get_param(xform_id, variation_id, 1u);
    let invert = i32(get_param(xform_id, variation_id, 2u));
    let power = get_param(xform_id, variation_id, 3u);

    var x: f32;
    var y: f32;
    if (mode == 0) {
        x = p.x;
        y = p.y;
    } else {
        x = rng_nextf(rng) - 0.5;
        y = rng_nextf(rng) - 0.5;
    }
    let uvx = x * zoom;
    let uvy = y * zoom;
    let lhs = pow(abs(uvx - uvy), power) + pow(abs(uvy + uvx), power);
    let color = select(0, 1, lhs <= 1.0);

    var hidden = false;
    if (invert == 0) {
        if (color == 0) { x = 0.0; y = 0.0; hidden = true; }
    } else {
        if (color == 1) { x = 0.0; y = 0.0; hidden = true; }
    }
    *hide = hidden;
    // pVarTP.x = pAmount * x (Replace; dispatcher applies the weight).
    return vec2<f32>(x, y);
}
"#,
    wgsl_3d: r#"
fn variation_cut_sqcir(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec3<f32> {
    let mode = i32(get_param(xform_id, variation_id, 0u));
    let zoom = get_param(xform_id, variation_id, 1u);
    let invert = i32(get_param(xform_id, variation_id, 2u));
    let power = get_param(xform_id, variation_id, 3u);

    var x: f32;
    var y: f32;
    if (mode == 0) {
        x = p.x;
        y = p.y;
    } else {
        x = rng_nextf(rng) - 0.5;
        y = rng_nextf(rng) - 0.5;
    }
    let uvx = x * zoom;
    let uvy = y * zoom;
    let lhs = pow(abs(uvx - uvy), power) + pow(abs(uvy + uvx), power);
    let color = select(0, 1, lhs <= 1.0);

    var hidden = false;
    if (invert == 0) {
        if (color == 0) { x = 0.0; y = 0.0; hidden = true; }
    } else {
        if (color == 1) { x = 0.0; y = 0.0; hidden = true; }
    }
    *hide = hidden;
    // z passes through (JWF: gated `pVarTP.z += pAmount*z`).
    return vec3<f32>(x, y, p.z);
}
"#,
};
