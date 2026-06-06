//! Apophysis miscellany batch 21: heart_wf, post_ztranslate_wf, post_mirror_wf
//!
//!   - heart_wf: polar heart-curve mapping
//!     `nx = 0.001 · (-t² + 40t + 1200) · sin(πt/180) · r`
//!     `ny = -0.001 · (-t² + 40t + 400) · cos(πt/180) · r`
//!     where `t = |a|/π · 60 · scale_r_{left,right} - shift_t`, capped at
//!     60. 5 user params (scale_x, scale_t, shift_t, scale_r_left,
//!     scale_r_right; scale_t is unused in the body — kept to match
//!     cpp). No init. Body factors cleanly through outer multiplier.
//!
//!   - post_ztranslate_wf: post-phase Z translate.
//!     `p.z += w` where `w` is the variation weight. 0 user params.
//!     Trivial — uses needs_transform to read weight in post phase.
//!
//!   - post_mirror_wf: post-phase axis mirroring with
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
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// ---------------------------------------------------------------------------
// heart_wf
// ---------------------------------------------------------------------------

/// Polar heart-curve mapping — maps the input radius `r` and angle `a` onto
/// a heart-shaped curve via `nx = ±0.001·(-t² + 40t + 1200)·sin(πt/180)·r`
/// and `ny = -0.001·(-t² + 40t + 400)·cos(πt/180)·r` where `t = |a|/π · 60
/// · scale_r − shift_t` (capped at 60). The sign of `a` selects the left vs
/// right half of the heart, each with its own radial scale.
pub static HEART_WF: VariationDef = VariationDef {
    name: "heart_wf",
    aliases: &[],
    display_name: "Heart WF",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[
        param!("scale_x", "Scale X", unlimited_float, 1.0, -10.0, 10.0, "X output scale factor (multiplies the final X output)."),
        param!("scale_t", "Scale T", unlimited_float, 1.0, -10.0, 10.0, "Unused in body — preserved as a parameter for cpp parity and preset compatibility."),
        param!("shift_t", "Shift T", unlimited_float, 0.0, -10.0, 10.0, "T parameter offset subtracted from the `|a|/π · 60 · scale_r` expression."),
        param!("scale_r_left", "Scale R Left", unlimited_float, 1.0, -10.0, 10.0, "Radial scale for the left half of the heart (input angle `a < 0`)."),
        param!("scale_r_right", "Scale R Right", unlimited_float, 1.0, -10.0, 10.0, "Radial scale for the right half (input angle `a ≥ 0`)."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
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
    wgsl_3d: r#"
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
"#,
};

// ---------------------------------------------------------------------------
// post_ztranslate_wf
// ---------------------------------------------------------------------------

/// Post-phase Z translate — adds the variation weight to the Z coordinate.
/// The XY coordinates pass through unchanged. Useful for offsetting the Z
/// position of a transform's accumulated output.
pub static POST_ZTRANSLATE_WF: VariationDef = VariationDef {
    name: "post_ztranslate_wf",
    aliases: &[],
    display_name: "Post Z-Translate WF",
    category: VariationCategory::Depth3D,
    phase: VariationPhase::Post,
    features: &[Feature::NeedsTransform],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_post_ztranslate_wf(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    return p;
}
"#,
    wgsl_3d: r#"
fn variation_post_ztranslate_wf(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let w = transforms[xform_id].variations[variation_id];
    return vec3<f32>(p.x, p.y, p.z + w);
}
"#,
};

// ---------------------------------------------------------------------------
// post_mirror_wf
// ---------------------------------------------------------------------------

/// Post-phase per-axis mirroring — for each enabled axis (`xaxis`, `yaxis`,
/// `zaxis`), with 50% probability per iteration flips the corresponding
/// output coordinate via `coord = −scale · (coord + shift)`. Each axis's
/// RNG draw is independent, so multiple axes can flip in the same
/// iteration.
pub static POST_MIRROR_WF: VariationDef = VariationDef {
    name: "post_mirror_wf",
    aliases: &[],
    display_name: "Post Mirror WF",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Post,
    features: &[Feature::NeedsRng],
    parameters: &[
        param!("xaxis", "X Axis", bool, true, "Enable the X-axis mirror branch."),
        param!("yaxis", "Y Axis", bool, false, "Enable the Y-axis mirror branch."),
        param!("zaxis", "Z Axis", bool, false, "Enable the Z-axis mirror branch. 3D only."),
        param!("xshift", "X Shift", unlimited_float, 0.0, -10.0, 10.0, "X mirror offset — applied as `-scale·(x + xshift)` when the X branch fires."),
        param!("yshift", "Y Shift", unlimited_float, 0.0, -10.0, 10.0, "Y mirror offset."),
        param!("zshift", "Z Shift", unlimited_float, 0.0, -10.0, 10.0, "Z mirror offset."),
        param!("xscale", "X Scale", unlimited_float, 1.0, -10.0, 10.0, "X output scale (applied in both the X-branch and Y-branch paths)."),
        param!("yscale", "Y Scale", unlimited_float, 1.0, -10.0, 10.0, "Y output scale (applied in both branch paths)."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
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
    wgsl_3d: r#"
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
"#,
};
