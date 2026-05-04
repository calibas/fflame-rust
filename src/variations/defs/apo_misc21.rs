//! Apophysis miscellany batch 21: heart_wf, post_ztranslate_wf, post_mirror_wf
//!
//!   - heart_wf (Maschke): polar heart-curve mapping
//!     `nx = 0.001 · (-t² + 40t + 1200) · sin(πt/180) · r`
//!     `ny = -0.001 · (-t² + 40t + 400) · cos(πt/180) · r`
//!     where `t = |a|/π · 60 · scale_r_{left,right} - shift_t`, capped at
//!     60. 5 user params (scale_x, scale_t, shift_t, scale_r_left,
//!     scale_r_right; scale_t is unused in the body — kept to match
//!     cpp). No init. Body factors cleanly through outer multiplier.
//!
//!   - post_ztranslate_wf (Maschke): post-phase Z translate.
//!     `p.z += w` where `w` is the variation weight. 0 user params.
//!     Trivial — uses needs_transform to read weight in post phase.
//!
//!   - post_mirror_wf (Maschke): post-phase axis mirroring with
//!     independent 50% chance per axis. 8 spatial user params (xaxis,
//!     yaxis, zaxis, xshift, yshift, zshift, xscale, yscale). cpp also
//!     includes color-shift params; skipped per writes_color-model
//!     compromise. RNG (3 calls per iteration). Each enabled axis
//!     independently flips the corresponding output coordinate.
//!
//! Sources:
//!   - `output/jwildfire-vars/output/heart_wf.cpp`
//!   - `output/jwildfire-vars/output/post_ztranslate_wf.cpp`
//!   - `output/jwildfire-vars/output/post_mirror_wf.cpp`

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// ---------------------------------------------------------------------------
// heart_wf
// ---------------------------------------------------------------------------

pub static HEART_WF: VariationDef = VariationDef {
    name: "heart_wf",
    display_name: "Heart WF",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("scale_x", "Scale X", unlimited_float, 1.0, -10.0, 10.0),
        param!("scale_t", "Scale T", unlimited_float, 1.0, -10.0, 10.0),
        param!("shift_t", "Shift T", unlimited_float, 0.0, -10.0, 10.0),
        param!("scale_r_left", "Scale R Left", unlimited_float, 1.0, -10.0, 10.0),
        param!("scale_r_right", "Scale R Right", unlimited_float, 1.0, -10.0, 10.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_heart_wf(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let scale_x = get_param(xform_id, variation_id, 0u);
    let shift_t = get_param(xform_id, variation_id, 2u);
    let scale_r_left = get_param(xform_id, variation_id, 3u);
    let scale_r_right = get_param(xform_id, variation_id, 4u);
    let pi = 3.14159265358979;
    let t_max = 60.0;
    let a = atan2(p.x, p.y);
    let r = sqrt(p.x * p.x + p.y * p.y);
    var t: f32;
    var nx: f32;
    if (a < 0.0) {
        t = -a / pi * t_max * scale_r_left - shift_t;
        if (t > t_max) { t = t_max; }
        nx = -0.001 * (-t * t + 40.0 * t + 1200.0) * sin(pi * t / 180.0) * r;
    } else {
        t = a / pi * t_max * scale_r_right - shift_t;
        if (t > t_max) { t = t_max; }
        nx = 0.001 * (-t * t + 40.0 * t + 1200.0) * sin(pi * t / 180.0) * r;
    }
    let ny = -0.001 * (-t * t + 40.0 * t + 400.0) * cos(pi * t / 180.0) * r;
    return vec2<f32>(nx * scale_x, ny);
}
"#,
    wgsl_3d: Some(r#"
fn variation_heart_wf(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let scale_x = get_param(xform_id, variation_id, 0u);
    let shift_t = get_param(xform_id, variation_id, 2u);
    let scale_r_left = get_param(xform_id, variation_id, 3u);
    let scale_r_right = get_param(xform_id, variation_id, 4u);
    let pi = 3.14159265358979;
    let t_max = 60.0;
    let a = atan2(p.x, p.y);
    let r = sqrt(p.x * p.x + p.y * p.y);
    var t: f32;
    var nx: f32;
    if (a < 0.0) {
        t = -a / pi * t_max * scale_r_left - shift_t;
        if (t > t_max) { t = t_max; }
        nx = -0.001 * (-t * t + 40.0 * t + 1200.0) * sin(pi * t / 180.0) * r;
    } else {
        t = a / pi * t_max * scale_r_right - shift_t;
        if (t > t_max) { t = t_max; }
        nx = 0.001 * (-t * t + 40.0 * t + 1200.0) * sin(pi * t / 180.0) * r;
    }
    let ny = -0.001 * (-t * t + 40.0 * t + 400.0) * cos(pi * t / 180.0) * r;
    return vec3<f32>(nx * scale_x, ny, p.z);
}
"#),
};

// ---------------------------------------------------------------------------
// post_ztranslate_wf
// ---------------------------------------------------------------------------

pub static POST_ZTRANSLATE_WF: VariationDef = VariationDef {
    name: "post_ztranslate_wf",
    display_name: "Post Z-Translate WF",
    category: VariationCategory::Depth3D,
    phase: VariationPhase::Post,
    needs_rng: false,
    parameters: &[],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_post_ztranslate_wf(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    return p;
}
"#,
    wgsl_3d: Some(r#"
fn variation_post_ztranslate_wf(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let w = transforms[xform_id].variations[variation_id];
    return vec3<f32>(p.x, p.y, p.z + w);
}
"#),
};

// ---------------------------------------------------------------------------
// post_mirror_wf
// ---------------------------------------------------------------------------

pub static POST_MIRROR_WF: VariationDef = VariationDef {
    name: "post_mirror_wf",
    display_name: "Post Mirror WF",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Post,
    needs_rng: true,
    parameters: &[
        param!("xaxis", "X Axis", int, 1.0, 0.0, 1.0),
        param!("yaxis", "Y Axis", int, 0.0, 0.0, 1.0),
        param!("zaxis", "Z Axis", int, 0.0, 0.0, 1.0),
        param!("xshift", "X Shift", unlimited_float, 0.0, -10.0, 10.0),
        param!("yshift", "Y Shift", unlimited_float, 0.0, -10.0, 10.0),
        param!("zshift", "Z Shift", unlimited_float, 0.0, -10.0, 10.0),
        param!("xscale", "X Scale", unlimited_float, 1.0, -10.0, 10.0),
        param!("yscale", "Y Scale", unlimited_float, 1.0, -10.0, 10.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_post_mirror_wf(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let xaxis = i32(get_param(xform_id, variation_id, 0u));
    let yaxis = i32(get_param(xform_id, variation_id, 1u));
    let xshift = get_param(xform_id, variation_id, 3u);
    let yshift = get_param(xform_id, variation_id, 4u);
    let xscale = get_param(xform_id, variation_id, 6u);
    let yscale = get_param(xform_id, variation_id, 7u);

    var x = p.x;
    var y = p.y;
    if (xaxis > 0 && rng_nextf(rng) < 0.5) {
        x = xscale * (-x - xshift);
        y = yscale * y;
    }
    if (yaxis > 0 && rng_nextf(rng) < 0.5) {
        x = xscale * x;
        y = yscale * (-y - yshift);
    }
    return vec2<f32>(x, y);
}
"#,
    wgsl_3d: Some(r#"
fn variation_post_mirror_wf(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let xaxis = i32(get_param(xform_id, variation_id, 0u));
    let yaxis = i32(get_param(xform_id, variation_id, 1u));
    let zaxis = i32(get_param(xform_id, variation_id, 2u));
    let xshift = get_param(xform_id, variation_id, 3u);
    let yshift = get_param(xform_id, variation_id, 4u);
    let zshift = get_param(xform_id, variation_id, 5u);
    let xscale = get_param(xform_id, variation_id, 6u);
    let yscale = get_param(xform_id, variation_id, 7u);

    var x = p.x;
    var y = p.y;
    var z = p.z;
    if (xaxis > 0 && rng_nextf(rng) < 0.5) {
        x = xscale * (-x - xshift);
        y = yscale * y;
    }
    if (yaxis > 0 && rng_nextf(rng) < 0.5) {
        x = xscale * x;
        y = yscale * (-y - yshift);
    }
    if (zaxis > 0 && rng_nextf(rng) < 0.5) {
        z = -z - zshift;
    }
    return vec3<f32>(x, y, z);
}
"#),
};
