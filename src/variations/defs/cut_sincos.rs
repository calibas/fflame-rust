//! cut_sincos (Jesus Sosa) — JWildfire "cut_*" procedural-stencil family.
//!
//! A "cut" variation is a shadertoy-style mask: it samples a point (the
//! affine input in `mode=0`, or a fresh random point in `mode=1`), tests
//! it against a procedural pattern, and HIDES points via `Feature::CanHide`
//! (JWildfire's `pVarTP.doHide`). Kept points plot at their sampled
//! position; output is a replace (`pVarTP.x = pAmount·x`).
//!
//! The pattern is a flowing sine-band field: the sampled point is bent
//! (`uv.y += cos(uv.x·10)·0.15`), advected by `time`, then folded through
//! `fract` to make repeating bands whose width is modulated by
//! `sin(uv.x·3)`. Unlike the other cut_* masks, the threshold is reversed:
//! with `invert=0` the LIT band (`color > 0`) is hidden and the dark gaps
//! are kept. Transcribed from
//! `output/variation-jwf-source/CutSinCosFunc.java` (GPU body).
//!
//! `seed` is a UI-only convenience in JWF (it reseeds and rederives `time`
//! from the wall clock); the GPU body never reads it, so we declare it for
//! `.flame` round-trip but leave it unused.
//!
//! # Authors
//! - Jesus Sosa (cut_* family)

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

pub static CUT_SINCOS: VariationDef = VariationDef {
    name: "cut_sincos",
    aliases: &[],
    display_name: "Cut SinCos",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::Replace, Feature::CanHide],
    parameters: &[
        param!("seed", "Seed", int, 1000.0, 0.0, 1000000.0, "UI-only in JWildfire (reseeds and rederives `time` from the wall clock). Ignored by the renderer."),
        param!("mode", "Mode", enum, 1, &["Affine Input", "Random Point"], "Affine Input samples the incoming point; Random Point samples a fresh uniform point each call (the usual stencil mode)."),
        param!("time", "Time", unlimited_float, 0.9, -100.0, 100.0, "Advection phase: the band field scrolls along X by `time·0.5`."),
        param!("zoom", "Zoom", unlimited_float, 0.5, -50.0, 50.0, "Pattern scale: `uv = point·zoom`."),
        param!("invert", "Invert", bool, false, "Invert the cut. Note: at `invert=0` the lit band is hidden and the dark gaps are kept."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_cut_sincos(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec2<f32> {
    let mode = i32(get_param(xform_id, variation_id, 1u));
    let time = get_param(xform_id, variation_id, 2u);
    let zoom = get_param(xform_id, variation_id, 3u);
    let invert = i32(get_param(xform_id, variation_id, 4u));

    var x: f32;
    var y: f32;
    if (mode == 0) {
        x = p.x;
        y = p.y;
    } else {
        x = rng_nextf(rng) - 0.5;
        y = rng_nextf(rng) - 0.5;
    }
    var uv = vec2<f32>(x * zoom, y * zoom);
    uv.x += 1.5;
    uv.y += cos(uv.x * 10.0) * 0.15;
    uv.y *= 1.5;
    let r = 7.3;
    let e = uv.x - time * 0.5;
    let t = e + abs(uv.y);
    let f = (1.0 + sin(uv.x * 3.0)) * 0.4 + 0.1;
    let d = f - abs(fract(t * r) - 0.5) * 2.0;
    let s = smoothstep(0.0, 0.05 / max(d + 0.5, 0.0), d + 0.1);
    let color = sqrt(s);

    var hidden = false;
    if (invert == 0) {
        if (color > 0.0) { x = 0.0; y = 0.0; hidden = true; }
    } else {
        if (color <= 0.0) { x = 0.0; y = 0.0; hidden = true; }
    }
    *hide = hidden;
    // pVarTP.x = pAmount * x (Replace; dispatcher applies the weight).
    return vec2<f32>(x, y);
}
"#,
    wgsl_3d: r#"
fn variation_cut_sincos(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec3<f32> {
    let mode = i32(get_param(xform_id, variation_id, 1u));
    let time = get_param(xform_id, variation_id, 2u);
    let zoom = get_param(xform_id, variation_id, 3u);
    let invert = i32(get_param(xform_id, variation_id, 4u));

    var x: f32;
    var y: f32;
    if (mode == 0) {
        x = p.x;
        y = p.y;
    } else {
        x = rng_nextf(rng) - 0.5;
        y = rng_nextf(rng) - 0.5;
    }
    var uv = vec2<f32>(x * zoom, y * zoom);
    uv.x += 1.5;
    uv.y += cos(uv.x * 10.0) * 0.15;
    uv.y *= 1.5;
    let r = 7.3;
    let e = uv.x - time * 0.5;
    let t = e + abs(uv.y);
    let f = (1.0 + sin(uv.x * 3.0)) * 0.4 + 0.1;
    let d = f - abs(fract(t * r) - 0.5) * 2.0;
    let s = smoothstep(0.0, 0.05 / max(d + 0.5, 0.0), d + 0.1);
    let color = sqrt(s);

    var hidden = false;
    if (invert == 0) {
        if (color > 0.0) { x = 0.0; y = 0.0; hidden = true; }
    } else {
        if (color <= 0.0) { x = 0.0; y = 0.0; hidden = true; }
    }
    *hide = hidden;
    // z passes through (JWF: gated `pVarTP.z += pAmount*z`).
    return vec3<f32>(x, y, p.z);
}
"#,
};
