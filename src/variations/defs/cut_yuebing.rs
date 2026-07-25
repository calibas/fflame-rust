//! cut_yuebing (Jesus Sosa) — JWildfire "cut_*" procedural-stencil family.
//!
//! A "cut" variation is a shadertoy-style mask: it samples a point (the
//! affine input in `mode=0`, or a fresh random point in `mode=1`), tests
//! it against a procedural pattern, and HIDES points below a threshold via
//! `Feature::CanHide` (JWildfire's `pVarTP.doHide`). Kept points plot at
//! their sampled position; output is a replace (`pVarTP.x = pAmount·x`).
//!
//! The pattern here is a single high-frequency moiré: `sin(x·y·10000·
//! duration)` where `duration = sin(p1/2)·p2`. The product `x·y` makes the
//! fringes hyperbolic; `p1`/`p2` tune the fringe frequency. Transcribed
//! from `output/variation-jwf-source/CutYuebingFunc.java` (GPU body).
//!
//! # Authors
//! - Jesus Sosa (cut_* family)

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Yuebing (mooncake) radial-pattern stencil mask.
///
/// # Authors
/// - Jesus Sosa
pub static CUT_YUEBING: VariationDef = VariationDef {
    name: "cut_yuebing",
    aliases: &[],
    display_name: "Cut Yuebing",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::Replace, Feature::CanHide],
    parameters: &[
        param!("mode", "Mode", enum, 1, &["Affine Input", "Random Point"], "Affine Input samples the incoming point; Random Point samples a fresh uniform point each call (the usual stencil mode)."),
        param!("zoom", "Zoom", unlimited_float, 0.25, -50.0, 50.0, "Pattern scale: `position = point·zoom`."),
        param!("invert", "Invert", bool, false, "Invert the cut — keep what would be hidden and vice versa."),
        param!("p1", "P1", unlimited_float, 2.0, -50.0, 50.0, "Fringe-frequency phase: `duration = sin(p1/2)·p2`."),
        param!("p2", "P2", unlimited_float, 25.0, -100.0, 100.0, "Fringe-frequency amplitude: `duration = sin(p1/2)·p2`."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_cut_yuebing(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec2<f32> {
    let mode = i32(get_param(xform_id, variation_id, 0u));
    let zoom = get_param(xform_id, variation_id, 1u);
    let invert = i32(get_param(xform_id, variation_id, 2u));
    let p1 = get_param(xform_id, variation_id, 3u);
    let p2 = get_param(xform_id, variation_id, 4u);

    var x: f32;
    var y: f32;
    if (mode == 0) {
        x = p.x;
        y = p.y;
    } else {
        x = rng_nextf(rng) - 0.5;
        y = rng_nextf(rng) - 0.5;
    }
    let posx = x * zoom;
    let posy = y * zoom;
    let duration = sin(p1 / 2.0) * p2;
    let color = sin(posx * posy * 10000.0 * duration);

    var hidden = false;
    if (invert == 0) {
        if (color < 0.3) { x = 0.0; y = 0.0; hidden = true; }
    } else {
        if (color >= 0.3) { x = 0.0; y = 0.0; hidden = true; }
    }
    *hide = hidden;
    // pVarTP.x = pAmount * x (Replace; dispatcher applies the weight).
    return vec2<f32>(x, y);
}
"#,
    wgsl_3d: r#"
fn variation_cut_yuebing(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec3<f32> {
    let mode = i32(get_param(xform_id, variation_id, 0u));
    let zoom = get_param(xform_id, variation_id, 1u);
    let invert = i32(get_param(xform_id, variation_id, 2u));
    let p1 = get_param(xform_id, variation_id, 3u);
    let p2 = get_param(xform_id, variation_id, 4u);

    var x: f32;
    var y: f32;
    if (mode == 0) {
        x = p.x;
        y = p.y;
    } else {
        x = rng_nextf(rng) - 0.5;
        y = rng_nextf(rng) - 0.5;
    }
    let posx = x * zoom;
    let posy = y * zoom;
    let duration = sin(p1 / 2.0) * p2;
    let color = sin(posx * posy * 10000.0 * duration);

    var hidden = false;
    if (invert == 0) {
        if (color < 0.3) { x = 0.0; y = 0.0; hidden = true; }
    } else {
        if (color >= 0.3) { x = 0.0; y = 0.0; hidden = true; }
    }
    *hide = hidden;
    // z passes through (JWF: gated `pVarTP.z += pAmount*z`).
    return vec3<f32>(x, y, p.z);
}
"#,
};
