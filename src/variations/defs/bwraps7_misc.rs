//! `bwraps7` — newer version of bwraps (slobo777, version 7)
//!
//! Same Bubble Wrap algorithm as our existing `bwraps`, but with
//! an updated `_g2` formula. Where the old `bwraps` uses
//! `g2 = gain² / radius + ε`, this v7 variant uses
//! `g2 = gain² + ε` (no division by radius). The difference shows
//! up in how the bubble fills the cell at small `gain` values.
//! Both variations ship so flames built against either lineage
//! render the same.
//!
//!   - 5 user params: cellsize, space, gain, inner_twist, outer_twist
//!   - 3 init slots: g2 = gain² + ε;
//!                   r2 = radius² where radius = ½·cellsize/(1+space²);
//!                   rfactor = radius / max_bubble
//!
//! Body factor cleanly through outer multiplier.
//!
//! Source: `output/jwildfire-vars/output/bwraps7.cpp`.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

pub static BWRAPS7: VariationDef = VariationDef {
    name: "bwraps7",
    display_name: "BWraps 7",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("cellsize", "Cell Size", unlimited_float, 1.0, -10.0, 10.0),
        param!("space", "Space", unlimited_float, 0.0, -1.0, 1.0),
        param!("gain", "Gain", unlimited_float, 2.0, -5.0, 5.0),
        param!("inner_twist", "Inner Twist", unlimited_float, 0.0, -10.0, 10.0),
        param!("outer_twist", "Outer Twist", unlimited_float, 0.0, -10.0, 10.0),
    ],
    needs_transform: false,
    writes_color: false,
    // 3 derived values at slots 5..8: g2, r2, rfactor
    init_param_count: 3,
    wgsl_init: Some(r#"
fn init_bwraps7(user: array<f32, 5>) -> array<f32, 3> {
    let cellsize = user[0];
    let space = user[1];
    let gain = user[2];
    var out: array<f32, 3>;
    if (abs(cellsize) < 1e-30) {
        out[0] = 0.0; out[1] = 0.0; out[2] = 0.0;
        return out;
    }
    let radius = 0.5 * (cellsize / (1.0 + space * space));
    let g2 = gain * gain + 1e-6;
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
    wgsl_2d: r#"
fn variation_bwraps7(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let cellsize = get_param(xform_id, variation_id, 0u);
    let inner_twist = get_param(xform_id, variation_id, 3u);
    let outer_twist = get_param(xform_id, variation_id, 4u);
    let g2 = get_param(xform_id, variation_id, 5u);
    let r2 = get_param(xform_id, variation_id, 6u);
    let rfactor = get_param(xform_id, variation_id, 7u);

    if (abs(cellsize) < 1e-30) {
        return p;
    }
    let cx = (floor(p.x / cellsize) + 0.5) * cellsize;
    let cy = (floor(p.y / cellsize) + 0.5) * cellsize;
    var lx = p.x - cx;
    var ly = p.y - cy;
    if ((lx * lx + ly * ly) > r2) {
        return p;
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
    return vec2<f32>(cx + ct * lx + st * ly, cy - st * lx + ct * ly);
}
"#,
    wgsl_3d: Some(r#"
fn variation_bwraps7(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let cellsize = get_param(xform_id, variation_id, 0u);
    let inner_twist = get_param(xform_id, variation_id, 3u);
    let outer_twist = get_param(xform_id, variation_id, 4u);
    let g2 = get_param(xform_id, variation_id, 5u);
    let r2 = get_param(xform_id, variation_id, 6u);
    let rfactor = get_param(xform_id, variation_id, 7u);

    if (abs(cellsize) < 1e-30) {
        return p;
    }
    let cx = (floor(p.x / cellsize) + 0.5) * cellsize;
    let cy = (floor(p.y / cellsize) + 0.5) * cellsize;
    var lx = p.x - cx;
    var ly = p.y - cy;
    if ((lx * lx + ly * ly) > r2) {
        return p;
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
    return vec3<f32>(cx + ct * lx + st * ly, cy - st * lx + ct * ly, p.z);
}
"#),
};
