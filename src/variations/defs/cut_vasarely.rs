//! cut_vasarely (Jesus Sosa) — JWildfire "cut_*" procedural-stencil family.
//!
//! A "cut" variation is a shadertoy-style mask: it samples a point (the
//! affine input in `mode=0`, or a fresh random point in `mode=1`), tests
//! it against a procedural pattern, and HIDES points below a threshold via
//! `Feature::CanHide` (JWildfire's `pVarTP.doHide`). Kept points plot at
//! their sampled position; output is a replace (`pVarTP.x = pAmount·x`).
//!
//! The pattern is a Vasarely-style op-art bulge: a radial sine warped by
//! `b = max(0, 2 − l/size)` (a central swelling that falls off with radius
//! `l`), confined to the vertical strip `|uv.x| < 1`. Transcribed from
//! `output/variation-jwf-source/CutVasarelyFunc.java` (GPU body).
//!
//! Note: JWF's GPU code writes `color` via two ternary lines that reference
//! `color` mid-update; they reduce algebraically to `color = 0.5 + sin(...)`
//! inside the strip (else 0). We keep the literal two-step form so the f32
//! result matches the reference bit-for-bit.
//!
//! # Authors
//! - Jesus Sosa (cut_* family)

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Vasarely op-art bulge-grid stencil mask.
///
/// # Authors
/// - Jesus Sosa
pub static CUT_VASARELY: VariationDef = VariationDef {
    name: "cut_vasarely",
    aliases: &[],
    display_name: "Cut Vasarely",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::Replace, Feature::CanHide],
    parameters: &[
        param!("mode", "Mode", enum, 1, &["Affine Input", "Random Point"], "Affine Input samples the incoming point; Random Point samples a fresh uniform point each call (the usual stencil mode)."),
        param!("zoom", "Zoom", unlimited_float, 1.0, -50.0, 50.0, "Pattern scale: `uv = point·zoom`."),
        param!("invert", "Invert", bool, false, "Invert the cut — keep what would be hidden and vice versa."),
        param!("size", "Size", unlimited_float, 0.5, 0.001, 50.0, "Central-bulge radius: `b = max(0, 2 − l/size)`."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_cut_vasarely(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec2<f32> {
    let mode = i32(get_param(xform_id, variation_id, 0u));
    let zoom = get_param(xform_id, variation_id, 1u);
    let invert = i32(get_param(xform_id, variation_id, 2u));
    let size = get_param(xform_id, variation_id, 3u);

    var x: f32;
    var y: f32;
    if (mode == 0) {
        x = p.x;
        y = p.y;
    } else {
        x = 2.0 * rng_nextf(rng) - 1.0;
        y = 2.0 * rng_nextf(rng) - 1.0;
    }
    let uv = vec2<f32>(x, y) * zoom;
    let l = length(uv);
    let b = max(0.0, 2.0 - l / size);

    let cond = abs(uv.x) < 1.0;
    var color = 0.0;
    color = select(color, color - 0.5 - sin(l * sin(atan2(uv.y, uv.x) + 1.57 + 4.0 * b * b) * 150.0), cond);
    color = color - select(color, color - 0.5 - sin(l * sin(atan2(uv.y, uv.x) + 1.57 + 4.0 * b * b) * 150.0), cond);

    var hidden = false;
    if (invert == 0) {
        if (color < 0.1) { x = 0.0; y = 0.0; hidden = true; }
    } else {
        if (color >= 0.1) { x = 0.0; y = 0.0; hidden = true; }
    }
    *hide = hidden;
    // pVarTP.x = pAmount * x (Replace; dispatcher applies the weight).
    return vec2<f32>(x, y);
}
"#,
    wgsl_3d: r#"
fn variation_cut_vasarely(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec3<f32> {
    let mode = i32(get_param(xform_id, variation_id, 0u));
    let zoom = get_param(xform_id, variation_id, 1u);
    let invert = i32(get_param(xform_id, variation_id, 2u));
    let size = get_param(xform_id, variation_id, 3u);

    var x: f32;
    var y: f32;
    if (mode == 0) {
        x = p.x;
        y = p.y;
    } else {
        x = 2.0 * rng_nextf(rng) - 1.0;
        y = 2.0 * rng_nextf(rng) - 1.0;
    }
    let uv = vec2<f32>(x, y) * zoom;
    let l = length(uv);
    let b = max(0.0, 2.0 - l / size);

    let cond = abs(uv.x) < 1.0;
    var color = 0.0;
    color = select(color, color - 0.5 - sin(l * sin(atan2(uv.y, uv.x) + 1.57 + 4.0 * b * b) * 150.0), cond);
    color = color - select(color, color - 0.5 - sin(l * sin(atan2(uv.y, uv.x) + 1.57 + 4.0 * b * b) * 150.0), cond);

    var hidden = false;
    if (invert == 0) {
        if (color < 0.1) { x = 0.0; y = 0.0; hidden = true; }
    } else {
        if (color >= 0.1) { x = 0.0; y = 0.0; hidden = true; }
    }
    *hide = hidden;
    // z passes through (JWF: gated `pVarTP.z += pAmount*z`).
    return vec3<f32>(x, y, p.z);
}
"#,
};
