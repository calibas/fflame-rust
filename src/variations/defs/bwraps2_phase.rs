//! `pre_bwraps2` and `post_bwraps2` (Xyrus02)
//!
//! Pre/post-phase variants of the bubble-wrap algorithm with yet
//! another `_g2` formula. Where:
//!
//!   - existing `bwraps`: g2 = gain² / radius + ε
//!   - existing `bwraps7`: g2 = gain² + ε
//!   - new `pre_bwraps2`/`post_bwraps2`: g2 = gain² / cellsize + ε
//!
//! All ship so flames built against any lineage render the same.
//!
//! Both use `needs_transform: true` to read the per-variation weight
//! and apply it directly inside the body (pre/post phases have no
//! outer multiplier), and write the result with the cpp's
//! `FTx = w · Vx` / `FPx = w · Vx` (assignment) semantics.
//!
//!   - 5 user params: cellsize, space, gain, inner_twist, outer_twist
//!   - 3 init slots: g2, r2, rfactor
//!
//! Sources:
//!   - `output/jwildfire-vars/output/pre_bwraps2.cpp`
//!   - `output/jwildfire-vars/output/post_bwraps2.cpp`

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// Shared init helper would need separate names per shader (since 2D
// and 3D bodies are concatenated independently). We define
// `init_pre_bwraps2` and `init_post_bwraps2` separately even though
// they compute the same thing — keeps each variation self-contained.

// =============================================================================
// pre_bwraps2 (Xyrus02)
//   Pre-phase bubble-wrap. Operates on the input `p` and returns the
//   transformed point (with w applied directly inside).
// =============================================================================
pub static PRE_BWRAPS2: VariationDef = VariationDef {
    name: "pre_bwraps2",
    display_name: "Pre BWraps 2",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Pre,
    needs_rng: false,
    parameters: &[
        param!("cellsize", "Cell Size", unlimited_float, 1.0, -10.0, 10.0),
        param!("space", "Space", unlimited_float, 0.0, -1.0, 1.0),
        param!("gain", "Gain", unlimited_float, 2.0, -5.0, 5.0),
        param!("inner_twist", "Inner Twist", unlimited_float, 0.0, -10.0, 10.0),
        param!("outer_twist", "Outer Twist", unlimited_float, 0.0, -10.0, 10.0),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 3,
    wgsl_init: Some(r#"
fn init_pre_bwraps2(user: array<f32, 5>) -> array<f32, 3> {
    let cellsize = user[0];
    let space = user[1];
    let gain = user[2];
    var out: array<f32, 3>;
    if (abs(cellsize) < 1e-30) {
        out[0] = 0.0; out[1] = 0.0; out[2] = 0.0;
        return out;
    }
    let radius = 0.5 * (cellsize / (1.0 + space * space));
    let g2 = gain * gain / cellsize + 1e-6;
    var max_bubble = g2 * radius;
    if (max_bubble > 2.0) {
        max_bubble = 1.0;
    } else {
        max_bubble = max_bubble * (1.0 / ((max_bubble * max_bubble) * 0.25 + 1.0));
    }
    out[0] = g2;
    out[1] = radius * radius;
    out[2] = radius / max(max_bubble, 1e-30);
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_pre_bwraps2(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let cellsize = get_param(xform_id, variation_id, 0u);
    let inner_twist = get_param(xform_id, variation_id, 3u);
    let outer_twist = get_param(xform_id, variation_id, 4u);
    let g2 = get_param(xform_id, variation_id, 5u);
    let r2 = get_param(xform_id, variation_id, 6u);
    let rfactor = get_param(xform_id, variation_id, 7u);
    let w = transforms[xform_id].variations[variation_id];

    if (abs(cellsize) < 1e-30) {
        return vec2<f32>(w * p.x, w * p.y);
    }
    let cx = (floor(p.x / cellsize) + 0.5) * cellsize;
    let cy = (floor(p.y / cellsize) + 0.5) * cellsize;
    var lx = p.x - cx;
    var ly = p.y - cy;
    if ((lx * lx + ly * ly) > r2) {
        return vec2<f32>(w * p.x, w * p.y);
    }
    lx = lx * g2;
    ly = ly * g2;
    let rb = rfactor / ((lx * lx + ly * ly) * 0.25 + 1.0);
    lx = lx * rb;
    ly = ly * rb;
    let rr = (lx * lx + ly * ly) / r2;
    let theta = inner_twist * (1.0 - rr) + outer_twist * rr;
    let st = sin(theta);
    let ct = cos(theta);
    return vec2<f32>(w * (cx + ct * lx + st * ly), w * (cy - st * lx + ct * ly));
}
"#,
    wgsl_3d: Some(r#"
fn variation_pre_bwraps2(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let cellsize = get_param(xform_id, variation_id, 0u);
    let inner_twist = get_param(xform_id, variation_id, 3u);
    let outer_twist = get_param(xform_id, variation_id, 4u);
    let g2 = get_param(xform_id, variation_id, 5u);
    let r2 = get_param(xform_id, variation_id, 6u);
    let rfactor = get_param(xform_id, variation_id, 7u);
    let w = transforms[xform_id].variations[variation_id];

    if (abs(cellsize) < 1e-30) {
        return vec3<f32>(w * p.x, w * p.y, p.z);
    }
    let cx = (floor(p.x / cellsize) + 0.5) * cellsize;
    let cy = (floor(p.y / cellsize) + 0.5) * cellsize;
    var lx = p.x - cx;
    var ly = p.y - cy;
    if ((lx * lx + ly * ly) > r2) {
        return vec3<f32>(w * p.x, w * p.y, p.z);
    }
    lx = lx * g2;
    ly = ly * g2;
    let rb = rfactor / ((lx * lx + ly * ly) * 0.25 + 1.0);
    lx = lx * rb;
    ly = ly * rb;
    let rr = (lx * lx + ly * ly) / r2;
    let theta = inner_twist * (1.0 - rr) + outer_twist * rr;
    let st = sin(theta);
    let ct = cos(theta);
    return vec3<f32>(w * (cx + ct * lx + st * ly), w * (cy - st * lx + ct * ly), p.z);
}
"#),
};

// =============================================================================
// post_bwraps2 (Xyrus02)
//   Post-phase bubble-wrap. Same body as pre_bwraps2; runs at post
//   phase on the accumulated point.
// =============================================================================
pub static POST_BWRAPS2: VariationDef = VariationDef {
    name: "post_bwraps2",
    display_name: "Post BWraps 2",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Post,
    needs_rng: false,
    parameters: &[
        param!("cellsize", "Cell Size", unlimited_float, 1.0, -10.0, 10.0),
        param!("space", "Space", unlimited_float, 0.0, -1.0, 1.0),
        param!("gain", "Gain", unlimited_float, 2.0, -5.0, 5.0),
        param!("inner_twist", "Inner Twist", unlimited_float, 0.0, -10.0, 10.0),
        param!("outer_twist", "Outer Twist", unlimited_float, 0.0, -10.0, 10.0),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 3,
    wgsl_init: Some(r#"
fn init_post_bwraps2(user: array<f32, 5>) -> array<f32, 3> {
    let cellsize = user[0];
    let space = user[1];
    let gain = user[2];
    var out: array<f32, 3>;
    if (abs(cellsize) < 1e-30) {
        out[0] = 0.0; out[1] = 0.0; out[2] = 0.0;
        return out;
    }
    let radius = 0.5 * (cellsize / (1.0 + space * space));
    let g2 = gain * gain / cellsize + 1e-6;
    var max_bubble = g2 * radius;
    if (max_bubble > 2.0) {
        max_bubble = 1.0;
    } else {
        max_bubble = max_bubble * (1.0 / ((max_bubble * max_bubble) * 0.25 + 1.0));
    }
    out[0] = g2;
    out[1] = radius * radius;
    out[2] = radius / max(max_bubble, 1e-30);
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_post_bwraps2(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let cellsize = get_param(xform_id, variation_id, 0u);
    let inner_twist = get_param(xform_id, variation_id, 3u);
    let outer_twist = get_param(xform_id, variation_id, 4u);
    let g2 = get_param(xform_id, variation_id, 5u);
    let r2 = get_param(xform_id, variation_id, 6u);
    let rfactor = get_param(xform_id, variation_id, 7u);
    let w = transforms[xform_id].variations[variation_id];

    if (abs(cellsize) < 1e-30) {
        return vec2<f32>(w * p.x, w * p.y);
    }
    let cx = (floor(p.x / cellsize) + 0.5) * cellsize;
    let cy = (floor(p.y / cellsize) + 0.5) * cellsize;
    var lx = p.x - cx;
    var ly = p.y - cy;
    if ((lx * lx + ly * ly) > r2) {
        return vec2<f32>(w * p.x, w * p.y);
    }
    lx = lx * g2;
    ly = ly * g2;
    let rb = rfactor / ((lx * lx + ly * ly) * 0.25 + 1.0);
    lx = lx * rb;
    ly = ly * rb;
    let rr = (lx * lx + ly * ly) / r2;
    let theta = inner_twist * (1.0 - rr) + outer_twist * rr;
    let st = sin(theta);
    let ct = cos(theta);
    return vec2<f32>(w * (cx + ct * lx + st * ly), w * (cy - st * lx + ct * ly));
}
"#,
    wgsl_3d: Some(r#"
fn variation_post_bwraps2(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let cellsize = get_param(xform_id, variation_id, 0u);
    let inner_twist = get_param(xform_id, variation_id, 3u);
    let outer_twist = get_param(xform_id, variation_id, 4u);
    let g2 = get_param(xform_id, variation_id, 5u);
    let r2 = get_param(xform_id, variation_id, 6u);
    let rfactor = get_param(xform_id, variation_id, 7u);
    let w = transforms[xform_id].variations[variation_id];

    if (abs(cellsize) < 1e-30) {
        return vec3<f32>(w * p.x, w * p.y, p.z);
    }
    let cx = (floor(p.x / cellsize) + 0.5) * cellsize;
    let cy = (floor(p.y / cellsize) + 0.5) * cellsize;
    var lx = p.x - cx;
    var ly = p.y - cy;
    if ((lx * lx + ly * ly) > r2) {
        return vec3<f32>(w * p.x, w * p.y, p.z);
    }
    lx = lx * g2;
    ly = ly * g2;
    let rb = rfactor / ((lx * lx + ly * ly) * 0.25 + 1.0);
    lx = lx * rb;
    ly = ly * rb;
    let rr = (lx * lx + ly * ly) / r2;
    let theta = inner_twist * (1.0 - rr) + outer_twist * rr;
    let st = sin(theta);
    let ct = cos(theta);
    return vec3<f32>(w * (cx + ct * lx + st * ly), w * (cy - st * lx + ct * ly), p.z);
}
"#),
};
