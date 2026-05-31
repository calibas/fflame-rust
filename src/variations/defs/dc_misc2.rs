//! Direct-color + pre-rect: dc_cube, pre_rect_wf
//!
//!   - dc_cube: random face of a unit cube — picks one of
//!     6 cube faces (i in {0, 1, 2}, j in {0, 1}) and emits a random
//!     point on that face. 9 user params: x/y/z scales + 6 face colors
//!     (c1..c6). 6 init slots clamp the face colors to [0, 1] per cpp's
//!     PluginVarPrepare. Writes color based on the picked face.
//!     RNG (4 calls/iter for face + 2 unit-square coords). Full3D.
//!     Body factors cleanly through outer multiplier.
//!
//!   - pre_rect_wf: pre-phase variation that replaces the
//!     affine input with a uniform random sample from a rectangle.
//!     4 user params (x0, x1, y0, y1) + 2 init slots (_dx = x1-x0,
//!     _dy = y1-y0). RNG (2 calls/iter). needs_transform to access
//!     weight in pre phase.
//!
//! Sources:
//!   - `output/jwildfire-vars/output/dc_cube.cpp`
//!   - `output/jwildfire-vars/output/pre_rect_wf.cpp`

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// ---------------------------------------------------------------------------
// dc_cube
// ---------------------------------------------------------------------------

/// Random cube-face sampler with direct-color writes — picks one of 6 unit
/// cube faces (3 axes × 2 sides) and emits a random point on that face,
/// scaled per-axis by `(x, y, z)`. Each face has its own color (`c1..c6`,
/// clamped to `[0, 1]`) written to the color register, so a flame using
/// `dc_cube` shows the cube as 6 distinct colored faces.
///
/// Discards the input position entirely (samples a new point per
/// iteration). Visible color requires the transform's Direct Color
/// slider > 0.
pub static DC_CUBE: VariationDef = VariationDef {
    name: "dc_cube",
    aliases: &[],
    display_name: "DC Cube",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        // x/y/z kept at slots 0..2 so existing saved flames preserve their
        // values; the 6 color params are added at the end as new slots.
        param!("x", "X Scale", unlimited_float, 1.0, -10.0, 10.0, "X-axis output scale."),
        param!("y", "Y Scale", unlimited_float, 1.0, -10.0, 10.0, "Y-axis output scale."),
        param!("z", "Z Scale", unlimited_float, 1.0, -10.0, 10.0, "Z-axis output scale (3D only)."),
        param!("c1", "Color +X", unlimited_float, 0.0, -10.0, 10.0, "Color (clamped to [0, 1]) written when the +X face is picked."),
        param!("c2", "Color -X", unlimited_float, 0.0, -10.0, 10.0, "Color (clamped to [0, 1]) written when the -X face is picked."),
        param!("c3", "Color +Y", unlimited_float, 0.0, -10.0, 10.0, "Color (clamped to [0, 1]) written when the +Y face is picked."),
        param!("c4", "Color -Y", unlimited_float, 0.0, -10.0, 10.0, "Color (clamped to [0, 1]) written when the -Y face is picked."),
        param!("c5", "Color +Z", unlimited_float, 0.0, -10.0, 10.0, "Color (clamped to [0, 1]) written when the +Z face is picked."),
        param!("c6", "Color -Z", unlimited_float, 0.0, -10.0, 10.0, "Color (clamped to [0, 1]) written when the -Z face is picked."),
    ],
    needs_transform: false,
    writes_color: true,
    // 6 init slots (9..14): clamp(c1..c6, 0, 1) — mirrors cpp's PluginVarPrepare.
    init_param_count: 6,
    wgsl_init: Some(r#"
fn init_dc_cube(user: array<f32, 9>) -> array<f32, 6> {
    var out: array<f32, 6>;
    out[0] = clamp(user[3], 0.0, 1.0);
    out[1] = clamp(user[4], 0.0, 1.0);
    out[2] = clamp(user[5], 0.0, 1.0);
    out[3] = clamp(user[6], 0.0, 1.0);
    out[4] = clamp(user[7], 0.0, 1.0);
    out[5] = clamp(user[8], 0.0, 1.0);
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_dc_cube(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    let xs = get_param(xform_id, variation_id, 0u);
    let ys = get_param(xform_id, variation_id, 1u);
    let pp = 2.0 * rng_nextf(rng) - 1.0;
    let q = 2.0 * rng_nextf(rng) - 1.0;
    let i = i32(rng_nextf(rng) * 3.0);
    let j = i32(rng_nextf(rng) * 2.0);
    let s = select(1.0, -1.0, j == 1);
    var x: f32;
    var y: f32;
    // Pick face color: 2 colors per axis (slots 9..14).
    if (i <= 0) {
        x = s;
        y = pp;
        *vc = select(get_param(xform_id, variation_id, 9u),
                     get_param(xform_id, variation_id, 10u),
                     j == 1);
    } else if (i == 1) {
        x = pp;
        y = s;
        *vc = select(get_param(xform_id, variation_id, 11u),
                     get_param(xform_id, variation_id, 12u),
                     j == 1);
    } else {
        x = pp;
        y = q;
        *vc = select(get_param(xform_id, variation_id, 13u),
                     get_param(xform_id, variation_id, 14u),
                     j == 1);
    }
    return vec2<f32>(x * xs, y * ys);
}
"#,
    wgsl_3d: r#"
fn variation_dc_cube(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    let xs = get_param(xform_id, variation_id, 0u);
    let ys = get_param(xform_id, variation_id, 1u);
    let zs = get_param(xform_id, variation_id, 2u);
    let pp = 2.0 * rng_nextf(rng) - 1.0;
    let q = 2.0 * rng_nextf(rng) - 1.0;
    let i = i32(rng_nextf(rng) * 3.0);
    let j = i32(rng_nextf(rng) * 2.0);
    let s = select(1.0, -1.0, j == 1);
    var x: f32;
    var y: f32;
    var z: f32;
    if (i <= 0) {
        x = s;
        y = pp;
        z = q;
        *vc = select(get_param(xform_id, variation_id, 9u),
                     get_param(xform_id, variation_id, 10u),
                     j == 1);
    } else if (i == 1) {
        x = pp;
        y = s;
        z = q;
        *vc = select(get_param(xform_id, variation_id, 11u),
                     get_param(xform_id, variation_id, 12u),
                     j == 1);
    } else {
        x = pp;
        y = q;
        z = s;
        *vc = select(get_param(xform_id, variation_id, 13u),
                     get_param(xform_id, variation_id, 14u),
                     j == 1);
    }
    return vec3<f32>(x * xs, y * ys, z * zs);
}
"#,
};

// ---------------------------------------------------------------------------
// pre_rect_wf
// ---------------------------------------------------------------------------

/// Pre-phase uniform rectangle sample — replaces the input with a random
/// point uniformly sampled from the rectangle `[x0, x1] × [y0, y1]`, scaled
/// by the variation weight. Discards the input position entirely.
pub static PRE_RECT_WF: VariationDef = VariationDef {
    name: "pre_rect_wf",
    aliases: &[],
    display_name: "Pre Rect WF",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Pre,
    needs_rng: true,
    parameters: &[
        param!("x0", "X0", unlimited_float, -0.5, -10.0, 10.0, "Left edge of the sampling rectangle."),
        param!("x1", "X1", unlimited_float, 0.5, -10.0, 10.0, "Right edge of the sampling rectangle."),
        param!("y0", "Y0", unlimited_float, -0.5, -10.0, 10.0, "Bottom edge of the sampling rectangle."),
        param!("y1", "Y1", unlimited_float, 0.5, -10.0, 10.0, "Top edge of the sampling rectangle."),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 2,
    wgsl_init: Some(r#"
fn init_pre_rect_wf(user: array<f32, 4>) -> array<f32, 2> {
    var out: array<f32, 2>;
    out[0] = user[1] - user[0];
    out[1] = user[3] - user[2];
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_pre_rect_wf(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let x0 = get_param(xform_id, variation_id, 0u);
    let y0 = get_param(xform_id, variation_id, 2u);
    let dx = get_param(xform_id, variation_id, 4u);
    let dy = get_param(xform_id, variation_id, 5u);
    let w = transforms[xform_id].variations[variation_id];
    return vec2<f32>(w * (x0 + dx * rng_nextf(rng)), w * (y0 + dy * rng_nextf(rng)));
}
"#,
    wgsl_3d: r#"
fn variation_pre_rect_wf(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let x0 = get_param(xform_id, variation_id, 0u);
    let y0 = get_param(xform_id, variation_id, 2u);
    let dx = get_param(xform_id, variation_id, 4u);
    let dy = get_param(xform_id, variation_id, 5u);
    let w = transforms[xform_id].variations[variation_id];
    return vec3<f32>(w * (x0 + dx * rng_nextf(rng)), w * (y0 + dy * rng_nextf(rng)), p.z);
}
"#,
};
