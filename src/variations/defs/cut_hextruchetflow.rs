//! cut_hextruchetflow (Jesus Sosa, 2019) — first of the JWildfire "cut_*"
//! procedural-stencil family.
//!
//! A "cut" variation is a shadertoy-style mask: it samples a point (the
//! affine input in `mode=0`, or a fresh random point in `mode=1`), looks
//! up a procedural pattern at `uv = point·zoom − shift`, and HIDES points
//! whose pattern value crosses a threshold (`color.x > tol`, inverted by
//! `invert`). Hidden points are dropped from the splat via the
//! `Feature::CanHide` mechanism (JWildfire's `pVarTP.doHide`); kept points
//! plot at their sampled position. Output is a replace
//! (`pVarTP.x = pAmount·x`).
//!
//! This one's pattern is a hex-truchet flow (Shadertoy ltVBDw). The hex
//! helpers (PixToHex / HexToPix / HexEdgeDist / Hashfv2) are transcribed
//! from `output/variation-jwf-source/CutHexTruchetFlowFunc.java`. `tol` is
//! hardcoded 0.9 (JWF's param is commented out). `step(1.5, 1.0) = 0`, so
//! the `uv + step(...)` offset is a no-op.
//!
//! # Authors
//! - Jesus Sosa (cut_* family)

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

pub static CUT_HEXTRUCHETFLOW: VariationDef = VariationDef {
    name: "cut_hextruchetflow",
    aliases: &[],
    display_name: "Cut HexTruchet Flow",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::Replace, Feature::CanHide],
    parameters: &[
        param!("randomize", "Randomize (seed)", unlimited_float, 0.0, 0.0, 1000000.0, "Integer seed; shifts the pattern via `seed·sin(seed·180/π)`."),
        param!("mode", "Mode", enum, 1, &["Affine Input", "Random Point"], "Affine Input samples the incoming point; Random Point samples a fresh uniform point each call (the usual stencil mode)."),
        param!("grid", "Grid", bool, false, "Overlay the hex-cell edges into the mask."),
        param!("zoom", "Zoom", float, 10.0, 0.1, 50.0, "Pattern scale: `uv = point·zoom − shift`."),
        param!("invert", "Invert", bool, false, "Invert the cut — keep what would be hidden and vice versa."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn cuthtf_PixToHex(p: vec2<f32>) -> vec2<f32> {
    let sqrt3 = 1.73205;
    var c: vec3<f32>;
    let t0 = vec2<f32>((1.0 / sqrt3) * p.x - (1.0 / 3.0) * p.y, (2.0 / 3.0) * p.y);
    c.x = t0.x; c.z = t0.y; c.y = -c.x - c.z;
    let r0 = floor(c + 0.5);
    let dr = abs(r0 - c);
    let t1 = vec3<f32>(dr.y, dr.z, dr.x);
    let t2 = vec3<f32>(dr.z, dr.x, dr.y);
    let r = r0 - step(t1, dr) * step(t2, dr) * dot(r0, vec3<f32>(1.0));
    return vec2<f32>(r.x, r.z);
}
fn cuthtf_HexToPix(h: vec2<f32>) -> vec2<f32> {
    let sqrt3 = 1.73205;
    return vec2<f32>(sqrt3 * (h.x + 0.5 * h.y), 1.5 * h.y);
}
fn cuthtf_HexEdgeDist(p0: vec2<f32>) -> f32 {
    let sqrt3 = 1.73205;
    let p = abs(p0);
    return (sqrt3 / 2.0) - p.x + 0.5 * min(p.x - sqrt3 * p.y, 0.0);
}
fn cuthtf_Hashfv2(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(37.0, 39.0))) * 43758.54);
}
fn cuthtf_ShowScene(p: vec2<f32>, grid: i32) -> vec3<f32> {
    let sqrt3 = 1.73205;
    var col = vec3<f32>(0.0);
    var w = vec3<f32>(0.0);
    let cId = cuthtf_PixToHex(p);
    let pc = cuthtf_HexToPix(cId);
    let dir = 2.0 * step(cuthtf_Hashfv2(cId), 0.5) - 1.0;
    let t0 = pc + vec2<f32>(0.0, -dir);
    w.x = t0.x; w.y = t0.y;
    w.z = dot(t0 - p, t0 - p);
    var q = pc + vec2<f32>(sqrt3 / 2.0, 0.5 * dir);
    var d = dot(q - p, q - p);
    if (d < w.z) { w = vec3<f32>(q, d); }
    q = pc + vec2<f32>(-sqrt3 / 2.0, 0.5 * dir);
    d = dot(q - p, q - p);
    if (d < w.z) { w = vec3<f32>(q, d); }
    w.z = abs(sqrt(w.z) - 0.5);
    d = cuthtf_HexEdgeDist(p - pc);
    if (grid == 1) {
        // JWF uses smoothstep(1,1,d) (edge0==edge1) which is the limiting
        // step(1,d); use that to avoid the WGSL undefined case.
        col = vec3<f32>(1.0) * mix(1.0, step(1.0, d), smoothstep(0.01, 0.02, d));
    }
    if (w.z < 0.25) {
        col = vec3<f32>(1.0);
    }
    return col;
}
fn variation_cut_hextruchetflow(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec2<f32> {
    const PI: f32 = 3.14159265359;
    let randomize = get_param(xform_id, variation_id, 0u);
    let mode = i32(get_param(xform_id, variation_id, 1u));
    let grid = i32(get_param(xform_id, variation_id, 2u));
    let zoom = get_param(xform_id, variation_id, 3u);
    let invert = i32(get_param(xform_id, variation_id, 4u));
    let tol = 0.9;

    var x: f32;
    var y: f32;
    if (mode == 0) {
        x = p.x;
        y = p.y;
    } else {
        x = rng_nextf(rng) - 0.5;
        y = rng_nextf(rng) - 0.5;
    }
    let shiftxy = randomize * sin(randomize * 180.0 / PI);
    let uv = vec2<f32>(x * zoom - shiftxy, y * zoom - shiftxy);
    let col = cuthtf_ShowScene(uv, grid).x;

    var hidden = false;
    if (invert == 0) {
        if (col > tol) { x = 0.0; y = 0.0; hidden = true; }
    } else {
        if (col <= tol) { x = 0.0; y = 0.0; hidden = true; }
    }
    *hide = hidden;
    // pVarTP.x = pAmount * x (Replace; dispatcher applies the weight).
    return vec2<f32>(x, y);
}
"#,
    wgsl_3d: r#"
fn cuthtf_PixToHex(p: vec2<f32>) -> vec2<f32> {
    let sqrt3 = 1.73205;
    var c: vec3<f32>;
    let t0 = vec2<f32>((1.0 / sqrt3) * p.x - (1.0 / 3.0) * p.y, (2.0 / 3.0) * p.y);
    c.x = t0.x; c.z = t0.y; c.y = -c.x - c.z;
    let r0 = floor(c + 0.5);
    let dr = abs(r0 - c);
    let t1 = vec3<f32>(dr.y, dr.z, dr.x);
    let t2 = vec3<f32>(dr.z, dr.x, dr.y);
    let r = r0 - step(t1, dr) * step(t2, dr) * dot(r0, vec3<f32>(1.0));
    return vec2<f32>(r.x, r.z);
}
fn cuthtf_HexToPix(h: vec2<f32>) -> vec2<f32> {
    let sqrt3 = 1.73205;
    return vec2<f32>(sqrt3 * (h.x + 0.5 * h.y), 1.5 * h.y);
}
fn cuthtf_HexEdgeDist(p0: vec2<f32>) -> f32 {
    let sqrt3 = 1.73205;
    let p = abs(p0);
    return (sqrt3 / 2.0) - p.x + 0.5 * min(p.x - sqrt3 * p.y, 0.0);
}
fn cuthtf_Hashfv2(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(37.0, 39.0))) * 43758.54);
}
fn cuthtf_ShowScene(p: vec2<f32>, grid: i32) -> vec3<f32> {
    let sqrt3 = 1.73205;
    var col = vec3<f32>(0.0);
    var w = vec3<f32>(0.0);
    let cId = cuthtf_PixToHex(p);
    let pc = cuthtf_HexToPix(cId);
    let dir = 2.0 * step(cuthtf_Hashfv2(cId), 0.5) - 1.0;
    let t0 = pc + vec2<f32>(0.0, -dir);
    w.x = t0.x; w.y = t0.y;
    w.z = dot(t0 - p, t0 - p);
    var q = pc + vec2<f32>(sqrt3 / 2.0, 0.5 * dir);
    var d = dot(q - p, q - p);
    if (d < w.z) { w = vec3<f32>(q, d); }
    q = pc + vec2<f32>(-sqrt3 / 2.0, 0.5 * dir);
    d = dot(q - p, q - p);
    if (d < w.z) { w = vec3<f32>(q, d); }
    w.z = abs(sqrt(w.z) - 0.5);
    d = cuthtf_HexEdgeDist(p - pc);
    if (grid == 1) {
        col = vec3<f32>(1.0) * mix(1.0, step(1.0, d), smoothstep(0.01, 0.02, d));
    }
    if (w.z < 0.25) {
        col = vec3<f32>(1.0);
    }
    return col;
}
fn variation_cut_hextruchetflow(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec3<f32> {
    const PI: f32 = 3.14159265359;
    let randomize = get_param(xform_id, variation_id, 0u);
    let mode = i32(get_param(xform_id, variation_id, 1u));
    let grid = i32(get_param(xform_id, variation_id, 2u));
    let zoom = get_param(xform_id, variation_id, 3u);
    let invert = i32(get_param(xform_id, variation_id, 4u));
    let tol = 0.9;

    var x: f32;
    var y: f32;
    if (mode == 0) {
        x = p.x;
        y = p.y;
    } else {
        x = rng_nextf(rng) - 0.5;
        y = rng_nextf(rng) - 0.5;
    }
    let shiftxy = randomize * sin(randomize * 180.0 / PI);
    let uv = vec2<f32>(x * zoom - shiftxy, y * zoom - shiftxy);
    let col = cuthtf_ShowScene(uv, grid).x;

    var hidden = false;
    if (invert == 0) {
        if (col > tol) { x = 0.0; y = 0.0; hidden = true; }
    } else {
        if (col <= tol) { x = 0.0; y = 0.0; hidden = true; }
    }
    *hide = hidden;
    // z passes through (JWF: gated `pVarTP.z += pAmount*z`).
    return vec3<f32>(x, y, p.z);
}
"#,
};
