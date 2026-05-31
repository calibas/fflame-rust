//! `butterfly_fay` (CozyG)
//!
//! Parametric butterfly curve with mode-based spread for points
//! inside vs outside the curve. Based on the Butterfly curve, 
//! discovered ~1988 by Temple H. Fay
//! 
//! Body computes:
//!
//!   t = _number_of_cycles · atan2(y, x)
//!   r = ½ · (exp(cos t) − 2·cos 4t − sin⁵(t/12) + offset)
//!   x' = r·sin t,  y' = −r·cos t
//!
//! Then routes through one of 6 modes (0..5) based on whether the
//! input point is "outside" or "inside" the curve, controlled by
//! `outer_mode`/`inner_mode` (or unified by `unified_inner_outer`).
//!
//!   - 11 user params: cycles, offset, unified_inner_outer (int),
//!                     outer_mode (int), inner_mode (int),
//!                     outer_spread, inner_spread,
//!                     outer_spread_ratio, inner_spread_ratio,
//!                     spread_split, fill
//!   - 1 init slot: _number_of_cycles (= cycles, or π² when cycles=0)
//!
//! `sin⁵(t/12)` computed as repeated multiplication (WGSL `pow` is
//! undefined for negative bases).
//!
//! Body has `VVAR · stuff` everywhere — clean factor through outer.
//! RNG is used only when `fill != 0`.
//!
//! Source: `output/jwildfire-vars/output/butterfly_fay.cpp`.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Parametric butterfly curve — emits points on a butterfly curve `r =
/// ½·(exp(cos t) − 2·cos 4t − sin⁵(t/12) + offset)` driven by the input
/// angle `t = cycles · atan2(y, x)`. Based on the Butterfly curve
/// discovered ~1988 by Temple H. Fay. Routes through one of 6 output modes
/// (controlled by `outer_mode`/`inner_mode`) depending on whether the input
/// lies inside or outside the curve, with optional `fill` randomization.
///
/// # Authors
/// - CozyG
pub static BUTTERFLY_FAY: VariationDef = VariationDef {
    name: "butterfly_fay",
    aliases: &[],
    display_name: "Butterfly Fay",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("cycles", "Cycles", unlimited_float, 0.0, -100.0, 100.0, "Number of butterfly-curve cycles per full input rotation. 0 falls back to π² internally."),
        param!("offset", "Offset", unlimited_float, 0.0, -10.0, 10.0, "Additive offset on the curve radius formula."),
        param!("unified_inner_outer", "Unified", bool, true, "When on, always use the outer mode/spread/ratio. When off, pick based on whether the input is inside or outside the curve."),
        param!("outer_mode", "Outer Mode", enum, 1, &["On Curve", "Radial Stretch", "Mirror Blend", "Mirror Add", "Half + Offset", "Linear Input"], "Output mode for points outside the butterfly curve."),
        param!("inner_mode", "Inner Mode", enum, 1, &["On Curve", "Radial Stretch", "Mirror Blend", "Mirror Add", "Half + Offset", "Linear Input"], "Output mode for points inside the butterfly curve."),
        param!("outer_spread", "Outer Spread", unlimited_float, 0.0, -10.0, 10.0, "Outer-mode spread amount (interpretation depends on `outer_mode`)."),
        param!("inner_spread", "Inner Spread", unlimited_float, 0.0, -10.0, 10.0, "Inner-mode spread amount (interpretation depends on `inner_mode`)."),
        param!("outer_spread_ratio", "Outer Ratio", unlimited_float, 1.0, -10.0, 10.0, "X-vs-Y ratio for outer-mode spread."),
        param!("inner_spread_ratio", "Inner Ratio", unlimited_float, 1.0, -10.0, 10.0, "X-vs-Y ratio for inner-mode spread."),
        param!("spread_split", "Spread Split", unlimited_float, 1.0, -10.0, 10.0, "Multiplier on the input radius used to decide inner vs outer (compared against the curve radius)."),
        param!("fill", "Fill", unlimited_float, 0.0, -10.0, 10.0, "Random fill amount added to the curve radius. 0 disables; non-zero triggers an RNG call."),
    ],
    needs_transform: false,
    writes_color: false,
    // 1 derived value at slot 11: _number_of_cycles
    init_param_count: 1,
    wgsl_init: Some(r#"
fn init_butterfly_fay(user: array<f32, 11>) -> array<f32, 1> {
    let pi_sq = 9.869604401089358;  // π²
    var out: array<f32, 1>;
    let cycles = user[0];
    out[0] = select(cycles, pi_sq, cycles == 0.0);
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_butterfly_fay(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let offset = get_param(xform_id, variation_id, 1u);
    let unified = i32(get_param(xform_id, variation_id, 2u));
    let outer_mode = i32(get_param(xform_id, variation_id, 3u));
    let inner_mode = i32(get_param(xform_id, variation_id, 4u));
    let outer_spread = get_param(xform_id, variation_id, 5u);
    let inner_spread = get_param(xform_id, variation_id, 6u);
    let outer_ratio = get_param(xform_id, variation_id, 7u);
    let inner_ratio = get_param(xform_id, variation_id, 8u);
    let spread_split = get_param(xform_id, variation_id, 9u);
    let fill = get_param(xform_id, variation_id, 10u);
    let n_cycles = get_param(xform_id, variation_id, 11u);

    let theta = atan2(p.y, p.x);
    let t = n_cycles * theta;
    let rin = spread_split * sqrt(p.x * p.x + p.y * p.y);
    let s = sin(t / 12.0);
    let s5 = s * s * s * s * s;
    var r = 0.5 * (exp(cos(t)) - 2.0 * cos(4.0 * t) - s5 + offset);
    if (fill != 0.0) {
        r = r + fill * (rng_nextf(rng) - 0.5);
    }
    let x_curve = r * sin(t);
    let y_curve = -r * cos(t);

    // Decide which branch
    let is_outer = (abs(rin) > abs(r)) || (unified == 1);
    let mode = select(inner_mode, outer_mode, is_outer);
    let spread = select(inner_spread, outer_spread, is_outer);
    let ratio = select(inner_ratio, outer_ratio, is_outer);
    let sign = select(-1.0, 1.0, is_outer);  // outer adds; inner subtracts (in modes 2/3)

    var fx: f32;
    var fy: f32;
    if (mode == 0) {
        fx = x_curve; fy = y_curve;
    } else if (mode == 1) {
        let rinx = rin * spread * ratio - spread * ratio + 1.0;
        let riny = rin * spread - spread + 1.0;
        fx = rinx * x_curve; fy = riny * y_curve;
    } else if (mode == 2) {
        var xin = abs(p.x);
        var yin = abs(p.y);
        if (x_curve < 0.0) { xin = -xin; }
        if (y_curve < 0.0) { yin = -yin; }
        // outer: + spread·ratio·(xin − x_curve);  inner: − spread·ratio·(x_curve − xin)
        // Algebraically identical with sign-flip absorbed: + sign·spread·ratio·(xin − x_curve)
        fx = x_curve + sign * spread * ratio * (xin - x_curve);
        fy = y_curve + sign * spread * (yin - y_curve);
    } else if (mode == 3) {
        var xin = abs(p.x);
        var yin = abs(p.y);
        if (x_curve < 0.0) { xin = -xin; }
        if (y_curve < 0.0) { yin = -yin; }
        // outer: + spread·ratio·xin;  inner: − spread·ratio·xin
        fx = x_curve + sign * spread * ratio * xin;
        fy = y_curve + sign * spread * yin;
    } else if (mode == 4) {
        let rinx = 0.5 * rin + spread * ratio;
        let riny = 0.5 * rin + spread;
        fx = rinx * x_curve; fy = riny * y_curve;
    } else if (mode == 5) {
        fx = x_curve + spread * ratio * p.x;
        fy = y_curve + spread * p.y;
    } else {
        fx = x_curve; fy = y_curve;
    }
    return vec2<f32>(fx, fy);
}
"#,
    wgsl_3d: r#"
fn variation_butterfly_fay(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let offset = get_param(xform_id, variation_id, 1u);
    let unified = i32(get_param(xform_id, variation_id, 2u));
    let outer_mode = i32(get_param(xform_id, variation_id, 3u));
    let inner_mode = i32(get_param(xform_id, variation_id, 4u));
    let outer_spread = get_param(xform_id, variation_id, 5u);
    let inner_spread = get_param(xform_id, variation_id, 6u);
    let outer_ratio = get_param(xform_id, variation_id, 7u);
    let inner_ratio = get_param(xform_id, variation_id, 8u);
    let spread_split = get_param(xform_id, variation_id, 9u);
    let fill = get_param(xform_id, variation_id, 10u);
    let n_cycles = get_param(xform_id, variation_id, 11u);

    let theta = atan2(p.y, p.x);
    let t = n_cycles * theta;
    let rin = spread_split * sqrt(p.x * p.x + p.y * p.y);
    let s = sin(t / 12.0);
    let s5 = s * s * s * s * s;
    var r = 0.5 * (exp(cos(t)) - 2.0 * cos(4.0 * t) - s5 + offset);
    if (fill != 0.0) {
        r = r + fill * (rng_nextf(rng) - 0.5);
    }
    let x_curve = r * sin(t);
    let y_curve = -r * cos(t);

    let is_outer = (abs(rin) > abs(r)) || (unified == 1);
    let mode = select(inner_mode, outer_mode, is_outer);
    let spread = select(inner_spread, outer_spread, is_outer);
    let ratio = select(inner_ratio, outer_ratio, is_outer);
    let sign = select(-1.0, 1.0, is_outer);

    var fx: f32;
    var fy: f32;
    if (mode == 0) {
        fx = x_curve; fy = y_curve;
    } else if (mode == 1) {
        let rinx = rin * spread * ratio - spread * ratio + 1.0;
        let riny = rin * spread - spread + 1.0;
        fx = rinx * x_curve; fy = riny * y_curve;
    } else if (mode == 2) {
        var xin = abs(p.x);
        var yin = abs(p.y);
        if (x_curve < 0.0) { xin = -xin; }
        if (y_curve < 0.0) { yin = -yin; }
        fx = x_curve + sign * spread * ratio * (xin - x_curve);
        fy = y_curve + sign * spread * (yin - y_curve);
    } else if (mode == 3) {
        var xin = abs(p.x);
        var yin = abs(p.y);
        if (x_curve < 0.0) { xin = -xin; }
        if (y_curve < 0.0) { yin = -yin; }
        fx = x_curve + sign * spread * ratio * xin;
        fy = y_curve + sign * spread * yin;
    } else if (mode == 4) {
        let rinx = 0.5 * rin + spread * ratio;
        let riny = 0.5 * rin + spread;
        fx = rinx * x_curve; fy = riny * y_curve;
    } else if (mode == 5) {
        fx = x_curve + spread * ratio * p.x;
        fy = y_curve + spread * p.y;
    } else {
        fx = x_curve; fy = y_curve;
    }
    return vec3<f32>(fx, fy, p.z);
}
"#,
};
