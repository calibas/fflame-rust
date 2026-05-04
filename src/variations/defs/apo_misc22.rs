//! Apophysis miscellany batch 22: dc_carpet, post_point_symmetry_wf, cpow3_wf
//!
//!   - dc_carpet (Apophysis): randomized fractal carpet using
//!     `signum(c) · fmod(|c|, 1)` (toward-zero fractional part) plus a
//!     random `±1` cell offset per axis, then mapped through the
//!     transform's affine. 2 user params (origin, iterations;
//!     iterations is unused in body — kept to match cpp). Body factors
//!     cleanly through outer multiplier; reads `xf.a/b/c/d/e/f` for the
//!     affine transform. RNG (2 calls/iter for ±1 sign choices). cpp's
//!     direct-color writes (TC) skipped per writes_color compromise.
//!
//!   - post_point_symmetry_wf (Maschke): post-phase N-fold rotational
//!     symmetry around (centre_x, centre_y). 3 user params (centre_x,
//!     centre_y, order); cpp's colorshift omitted per writes_color
//!     compromise. Computes the per-iteration rotation angle on-the-fly
//!     instead of caching `_sina[16]`/`_cosa[16]` (cpp's fixed table
//!     would overflow our 16-slot budget). RNG (1 call/iter to pick
//!     the rotation index in [0, order)).
//!
//!   - cpow3_wf (CozyG / WF): randomized complex-power variation, the
//!     CPow3 family with discrete spread, secondary spread, and offset.
//!     7 user params (r, a, divisor, spread, discrete_spread, spread2,
//!     offset2) + 7 init slots (_c, _d, _half_c, _half_d, _ang,
//!     _inv_spread, _full_spread). RNG (3 calls/iter). Body factors
//!     cleanly through outer multiplier. Z passes through.
//!
//! Sources:
//!   - `output/jwildfire-vars/output/dc_carpet.cpp`
//!   - `output/jwildfire-vars/output/post_point_symmetry_wf.cpp`
//!   - `output/jwildfire-vars/output/cpow3_wf.cpp`

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// ---------------------------------------------------------------------------
// dc_carpet
// ---------------------------------------------------------------------------

pub static DC_CARPET: VariationDef = VariationDef {
    name: "dc_carpet",
    display_name: "DC Carpet",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("origin", "Origin", unlimited_float, 1.0, -10.0, 10.0),
        param!("iterations", "Iterations", int, 5.0, 1.0, 100.0),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn dc_carpet_signum(v: f32) -> f32 {
    if (v < 0.0) { return -1.0; }
    return 1.0;
}

fn dc_carpet_rndsign(rng: ptr<function, RngState>) -> f32 {
    if (rng_nextf(rng) < 0.5) { return -1.0; }
    return 1.0;
}

fn variation_dc_carpet(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let xf = transforms[xform_id];
    let x0 = dc_carpet_rndsign(rng);
    let y0 = dc_carpet_rndsign(rng);
    let fx = abs(p.x) - trunc(abs(p.x));
    let fy = abs(p.y) - trunc(abs(p.y));
    let x = dc_carpet_signum(p.x) * fx + x0;
    let y = dc_carpet_signum(p.y) * fy + y0;
    return vec2<f32>(xf.a * x + xf.b * y + xf.e, xf.c * x + xf.d * y + xf.f);
}
"#,
    wgsl_3d: Some(r#"
fn dc_carpet_signum(v: f32) -> f32 {
    if (v < 0.0) { return -1.0; }
    return 1.0;
}

fn dc_carpet_rndsign(rng: ptr<function, RngState>) -> f32 {
    if (rng_nextf(rng) < 0.5) { return -1.0; }
    return 1.0;
}

fn variation_dc_carpet(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let xf = transforms[xform_id];
    let x0 = dc_carpet_rndsign(rng);
    let y0 = dc_carpet_rndsign(rng);
    let fx = abs(p.x) - trunc(abs(p.x));
    let fy = abs(p.y) - trunc(abs(p.y));
    let x = dc_carpet_signum(p.x) * fx + x0;
    let y = dc_carpet_signum(p.y) * fy + y0;
    return vec3<f32>(xf.a * x + xf.b * y + xf.e, xf.c * x + xf.d * y + xf.f, p.z);
}
"#),
};

// ---------------------------------------------------------------------------
// post_point_symmetry_wf
// ---------------------------------------------------------------------------

pub static POST_POINT_SYMMETRY_WF: VariationDef = VariationDef {
    name: "post_point_symmetry_wf",
    display_name: "Post Point Symmetry WF",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Post,
    needs_rng: true,
    parameters: &[
        param!("centre_x", "Centre X", unlimited_float, 0.25, -10.0, 10.0),
        param!("centre_y", "Centre Y", unlimited_float, 0.5, -10.0, 10.0),
        param!("order", "Order", int, 3.0, 1.0, 16.0),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_post_point_symmetry_wf(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let cx = get_param(xform_id, variation_id, 0u);
    let cy = get_param(xform_id, variation_id, 1u);
    let order = max(i32(get_param(xform_id, variation_id, 2u)), 1);
    let order_f = f32(order);
    let w = transforms[xform_id].variations[variation_id];
    let two_pi = 6.28318530717959;

    let dx = (p.x - cx) * w;
    let dy = (p.y - cy) * w;
    let idx = i32(rng_nextf(rng) * order_f);
    let idx_clamped = clamp(idx, 0, order - 1);
    let angle = f32(idx_clamped) * two_pi / order_f;
    let cosa = cos(angle);
    let sina = sin(angle);
    return vec2<f32>(cx + dx * cosa + dy * sina, cy + dy * cosa - dx * sina);
}
"#,
    wgsl_3d: Some(r#"
fn variation_post_point_symmetry_wf(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let cx = get_param(xform_id, variation_id, 0u);
    let cy = get_param(xform_id, variation_id, 1u);
    let order = max(i32(get_param(xform_id, variation_id, 2u)), 1);
    let order_f = f32(order);
    let w = transforms[xform_id].variations[variation_id];
    let two_pi = 6.28318530717959;

    let dx = (p.x - cx) * w;
    let dy = (p.y - cy) * w;
    let idx = i32(rng_nextf(rng) * order_f);
    let idx_clamped = clamp(idx, 0, order - 1);
    let angle = f32(idx_clamped) * two_pi / order_f;
    let cosa = cos(angle);
    let sina = sin(angle);
    return vec3<f32>(cx + dx * cosa + dy * sina, cy + dy * cosa - dx * sina, p.z);
}
"#),
};

// ---------------------------------------------------------------------------
// cpow3_wf
// ---------------------------------------------------------------------------

pub static CPOW3_WF: VariationDef = VariationDef {
    name: "cpow3_wf",
    display_name: "CPow3 WF",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("r", "R", unlimited_float, 1.0, -10.0, 10.0),
        param!("a", "A", unlimited_float, 0.1, -10.0, 10.0),
        param!("divisor", "Divisor", unlimited_float, 1.0, -10.0, 10.0),
        param!("spread", "Spread", unlimited_float, 1.0, -10.0, 10.0),
        param!("discrete_spread", "Discrete Spread", unlimited_float, 1.0, 0.0, 1.0),
        param!("spread2", "Spread 2", unlimited_float, 0.0, -10.0, 10.0),
        param!("offset2", "Offset 2", unlimited_float, 1.0, -10.0, 10.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 7,
    wgsl_init: Some(r#"
fn init_cpow3_wf(user: array<f32, 7>) -> array<f32, 7> {
    let pi = 3.14159265358979;
    let two_pi = 6.28318530717959;
    let r = user[0];
    let a = user[1];
    let divisor = user[2];
    let spread = user[3];
    let safe_div = select(divisor, 1e-30, abs(divisor) < 1e-30);
    let safe_spread = select(spread, 1e-30, abs(spread) < 1e-30);
    let c = r * cos(pi * 0.5 * a) / safe_div;
    let d = r * sin(pi * 0.5 * a) / safe_div;
    var out: array<f32, 7>;
    out[0] = c;
    out[1] = d;
    out[2] = c * 0.5;
    out[3] = d * 0.5;
    out[4] = two_pi / safe_div;
    out[5] = 0.5 / safe_spread;
    out[6] = two_pi * spread;
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_cpow3_wf(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let spread = get_param(xform_id, variation_id, 3u);
    let discrete_spread = get_param(xform_id, variation_id, 4u);
    let spread2 = get_param(xform_id, variation_id, 5u);
    let offset2 = get_param(xform_id, variation_id, 6u);
    let c = get_param(xform_id, variation_id, 7u);
    let d = get_param(xform_id, variation_id, 8u);
    let half_c = get_param(xform_id, variation_id, 9u);
    let half_d = get_param(xform_id, variation_id, 10u);
    let ang = get_param(xform_id, variation_id, 11u);
    let inv_spread = get_param(xform_id, variation_id, 12u);
    let full_spread = get_param(xform_id, variation_id, 13u);
    let two_pi = 6.28318530717959;

    var ai = atan2(p.x, p.y);
    var n = rng_nextf(rng) * spread;
    if (discrete_spread >= 1.0) {
        n = trunc(n);
    }
    if (ai < 0.0) {
        n = n + 1.0;
    }
    ai = ai + two_pi * n;
    if (cos(ai * inv_spread) < rng_nextf(rng) * 2.0 - 1.0) {
        ai = ai - full_spread;
    }
    let r2 = p.x * p.x + p.y * p.y;
    let lnr2 = log(max(r2, 1e-30));
    let ri = exp(half_c * lnr2 - d * ai);
    let ang2 = c * ai * half_d * lnr2 * ang * (rng_nextf(rng) * spread2 + offset2);
    return vec2<f32>(ri * cos(ang2), ri * sin(ang2));
}
"#,
    wgsl_3d: Some(r#"
fn variation_cpow3_wf(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let spread = get_param(xform_id, variation_id, 3u);
    let discrete_spread = get_param(xform_id, variation_id, 4u);
    let spread2 = get_param(xform_id, variation_id, 5u);
    let offset2 = get_param(xform_id, variation_id, 6u);
    let c = get_param(xform_id, variation_id, 7u);
    let d = get_param(xform_id, variation_id, 8u);
    let half_c = get_param(xform_id, variation_id, 9u);
    let half_d = get_param(xform_id, variation_id, 10u);
    let ang = get_param(xform_id, variation_id, 11u);
    let inv_spread = get_param(xform_id, variation_id, 12u);
    let full_spread = get_param(xform_id, variation_id, 13u);
    let two_pi = 6.28318530717959;

    var ai = atan2(p.x, p.y);
    var n = rng_nextf(rng) * spread;
    if (discrete_spread >= 1.0) {
        n = trunc(n);
    }
    if (ai < 0.0) {
        n = n + 1.0;
    }
    ai = ai + two_pi * n;
    if (cos(ai * inv_spread) < rng_nextf(rng) * 2.0 - 1.0) {
        ai = ai - full_spread;
    }
    let r2 = p.x * p.x + p.y * p.y;
    let lnr2 = log(max(r2, 1e-30));
    let ri = exp(half_c * lnr2 - d * ai);
    let ang2 = c * ai * half_d * lnr2 * ang * (rng_nextf(rng) * spread2 + offset2);
    return vec3<f32>(ri * cos(ang2), ri * sin(ang2), p.z);
}
"#),
};
