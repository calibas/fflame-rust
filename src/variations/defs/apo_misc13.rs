//! Apophysis miscellany 13 — three more
//!
//!   - `q_ode`   (dark-beam, Java-recovered) — 12 user params; cpp
//!                                                APO_VARIABLES has
//!                                                them but PluginVarCalc
//!                                                empty unported_stub.
//!                                                Body has w-scaled
//!                                                terms only on
//!                                                q_ode02·x and
//!                                                q_ode11·y; the rest
//!                                                are unscaled.
//!                                                needs_transform
//!                                                divide-out
//!   - `ripple`  (Xyrus02)                     — 8 user (frequency,
//!                                                velocity, amplitude,
//!                                                centerx/y, phase,
//!                                                scale, fixed_dist_calc
//!                                                int) + 6 init slots
//!                                                (_f, _p, _s, _vxp,
//!                                                _pxa, _pixa). Clean
//!                                                factor through outer.
//!                                                cpp uses `FPx = …`
//!                                                like anamorphcyl
//!   - `scry2`   (dark-beam)                   — 3 user (sides int, star,
//!                                                circle) + 6 init slots
//!                                                (sina, cosa, sins,
//!                                                coss, sinc, cosc).
//!                                                Body has scry-style
//!                                                `d = r1·(r2 + 1/w)` so
//!                                                output `(x/d, y/d)`
//!                                                contains 1/w internally.
//!                                                needs_transform
//!                                                divide-out.
//!                                                Has a runtime sides
//!                                                loop. cpp's local
//!                                                `double VAR(circle)`
//!                                                line is a porter
//!                                                bug (macro expands
//!                                                to struct member);
//!                                                we use a local
//!                                                `circle_v = sqrt(x²+y²)`
//!                                                like loonie2's pattern
//!
//! Sources: each variation's `.cpp` file in
//! `output/jwildfire-vars/output/`.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// q_ode (dark-beam, Java-recovered)
//   fx_cpp = q01 + w·q02·x + q03·x² + q04·x·y + q05·y + q06·y²
//   fy_cpp = q07 + q08·x + q09·x² + q10·x·y + w·q11·y + q12·y²
//   needs_transform divide-out: body returns fx_cpp/w, fy_cpp/w.
// =============================================================================
pub static Q_ODE: VariationDef = VariationDef {
    name: "q_ode",
    display_name: "Q ODE",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("q_ode01", "Q ODE 01", unlimited_float, 0.0, -10.0, 10.0),
        param!("q_ode02", "Q ODE 02", unlimited_float, 1.0, -10.0, 10.0),
        param!("q_ode03", "Q ODE 03", unlimited_float, 0.0, -10.0, 10.0),
        param!("q_ode04", "Q ODE 04", unlimited_float, 0.0, -10.0, 10.0),
        param!("q_ode05", "Q ODE 05", unlimited_float, 0.0, -10.0, 10.0),
        param!("q_ode06", "Q ODE 06", unlimited_float, 0.0, -10.0, 10.0),
        param!("q_ode07", "Q ODE 07", unlimited_float, 0.0, -10.0, 10.0),
        param!("q_ode08", "Q ODE 08", unlimited_float, 1.0, -10.0, 10.0),
        param!("q_ode09", "Q ODE 09", unlimited_float, 0.0, -10.0, 10.0),
        param!("q_ode10", "Q ODE 10", unlimited_float, 0.0, -10.0, 10.0),
        param!("q_ode11", "Q ODE 11", unlimited_float, 0.0, -10.0, 10.0),
        param!("q_ode12", "Q ODE 12", unlimited_float, 0.0, -10.0, 10.0),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_q_ode(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let q01 = get_param(xform_id, variation_id, 0u);
    let q02 = get_param(xform_id, variation_id, 1u);
    let q03 = get_param(xform_id, variation_id, 2u);
    let q04 = get_param(xform_id, variation_id, 3u);
    let q05 = get_param(xform_id, variation_id, 4u);
    let q06 = get_param(xform_id, variation_id, 5u);
    let q07 = get_param(xform_id, variation_id, 6u);
    let q08 = get_param(xform_id, variation_id, 7u);
    let q09 = get_param(xform_id, variation_id, 8u);
    let q10 = get_param(xform_id, variation_id, 9u);
    let q11 = get_param(xform_id, variation_id, 10u);
    let q12 = get_param(xform_id, variation_id, 11u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let fx_cpp = q01 + w * q02 * p.x + q03 * p.x * p.x + q04 * p.x * p.y + q05 * p.y + q06 * p.y * p.y;
    let fy_cpp = q07 + q08 * p.x + q09 * p.x * p.x + q10 * p.x * p.y + w * q11 * p.y + q12 * p.y * p.y;
    return vec2<f32>(fx_cpp * inv_w, fy_cpp * inv_w);
}
"#,
    wgsl_3d: Some(r#"
fn variation_q_ode(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let q01 = get_param(xform_id, variation_id, 0u);
    let q02 = get_param(xform_id, variation_id, 1u);
    let q03 = get_param(xform_id, variation_id, 2u);
    let q04 = get_param(xform_id, variation_id, 3u);
    let q05 = get_param(xform_id, variation_id, 4u);
    let q06 = get_param(xform_id, variation_id, 5u);
    let q07 = get_param(xform_id, variation_id, 6u);
    let q08 = get_param(xform_id, variation_id, 7u);
    let q09 = get_param(xform_id, variation_id, 8u);
    let q10 = get_param(xform_id, variation_id, 9u);
    let q11 = get_param(xform_id, variation_id, 10u);
    let q12 = get_param(xform_id, variation_id, 11u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let fx_cpp = q01 + w * q02 * p.x + q03 * p.x * p.x + q04 * p.x * p.y + q05 * p.y + q06 * p.y * p.y;
    let fy_cpp = q07 + q08 * p.x + q09 * p.x * p.x + q10 * p.x * p.y + w * q11 * p.y + q12 * p.y * p.y;
    return vec3<f32>(fx_cpp * inv_w, fy_cpp * inv_w, p.z);
}
"#),
};

// =============================================================================
// ripple (Xyrus02)
//   8 user: frequency, velocity, amplitude, centerx, centery, phase,
//           scale, fixed_dist_calc (int 0/1)
//   6 init: _f = frequency·5;  _p = phase·2π − π;  _s = scale (or ε);
//           _vxp = velocity·_p;  _pxa = _p·(amplitude·0.01);
//           _pixa = (π − _p)·(amplitude·0.01)
//   Body computes wave-distorted position then linearly interpolates
//   between two phases. Clean factor through outer.
// =============================================================================
pub static RIPPLE: VariationDef = VariationDef {
    name: "ripple",
    display_name: "Ripple",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("frequency", "Frequency", unlimited_float, 2.0, -10.0, 10.0),
        param!("velocity", "Velocity", unlimited_float, 1.0, -10.0, 10.0),
        param!("amplitude", "Amplitude", unlimited_float, 0.5, -10.0, 10.0),
        param!("centerx", "Center X", unlimited_float, 0.0, -10.0, 10.0),
        param!("centery", "Center Y", unlimited_float, 0.0, -10.0, 10.0),
        param!("phase", "Phase", unlimited_float, 0.0, -10.0, 10.0),
        param!("scale", "Scale", unlimited_float, 1.0, -10.0, 10.0),
        param!("fixed_dist_calc", "Fixed Dist Calc", int, 0.0, 0.0, 1.0),
    ],
    needs_transform: false,
    writes_color: false,
    // 6 derived values at slots 8..14:
    //   8: _f, 9: _p, 10: _s, 11: _vxp, 12: _pxa, 13: _pixa
    init_param_count: 6,
    wgsl_init: Some(r#"
fn init_ripple(user: array<f32, 8>) -> array<f32, 6> {
    let pi = 3.14159265358979;
    let two_pi = 6.28318530717959;
    let f = user[0] * 5.0;
    let a_local = user[2] * 0.01;
    let p_local = user[5] * two_pi - pi;
    let s = select(user[6], 1e-30, abs(user[6]) < 1e-30);
    let vxp = user[1] * p_local;
    let pxa = p_local * a_local;
    let pixa = (pi - p_local) * a_local;
    var out: array<f32, 6>;
    out[0] = f;
    out[1] = p_local;
    out[2] = s;
    out[3] = vxp;
    out[4] = pxa;
    out[5] = pixa;
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_ripple(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let centerx = get_param(xform_id, variation_id, 3u);
    let centery = get_param(xform_id, variation_id, 4u);
    let fixed_dc = get_param(xform_id, variation_id, 7u);
    let _f = get_param(xform_id, variation_id, 8u);
    let _p = get_param(xform_id, variation_id, 9u);
    let _s = get_param(xform_id, variation_id, 10u);
    let _vxp = get_param(xform_id, variation_id, 11u);
    let _pxa = get_param(xform_id, variation_id, 12u);
    let _pixa = get_param(xform_id, variation_id, 13u);
    let _is = 1.0 / _s;

    let x = p.x * _s - centerx;
    let y = p.y * _s + centery;
    var d: f32;
    if (fixed_dc != 0.0) {
        d = sqrt(x * x + y * y);
    } else {
        d = sqrt(x * x * y * y);
    }
    d = max(d, 1e-30);
    let nx = x / d;
    let ny = y / d;
    let wave = cos(_f * d - _vxp);
    let d1 = wave * _pxa + d;
    let d2 = wave * _pixa + d;
    let u1 = centerx + nx * d1;
    let v1 = -centery + ny * d1;
    let u2 = centerx + nx * d2;
    let v2 = -centery + ny * d2;
    let lx = u1 + (u2 - u1) * _p;
    let ly = v1 + (v2 - v1) * _p;
    return vec2<f32>(lx * _is, ly * _is);
}
"#,
    wgsl_3d: Some(r#"
fn variation_ripple(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let centerx = get_param(xform_id, variation_id, 3u);
    let centery = get_param(xform_id, variation_id, 4u);
    let fixed_dc = get_param(xform_id, variation_id, 7u);
    let _f = get_param(xform_id, variation_id, 8u);
    let _p = get_param(xform_id, variation_id, 9u);
    let _s = get_param(xform_id, variation_id, 10u);
    let _vxp = get_param(xform_id, variation_id, 11u);
    let _pxa = get_param(xform_id, variation_id, 12u);
    let _pixa = get_param(xform_id, variation_id, 13u);
    let _is = 1.0 / _s;

    let x = p.x * _s - centerx;
    let y = p.y * _s + centery;
    var d: f32;
    if (fixed_dc != 0.0) {
        d = sqrt(x * x + y * y);
    } else {
        d = sqrt(x * x * y * y);
    }
    d = max(d, 1e-30);
    let nx = x / d;
    let ny = y / d;
    let wave = cos(_f * d - _vxp);
    let d1 = wave * _pxa + d;
    let d2 = wave * _pixa + d;
    let u1 = centerx + nx * d1;
    let v1 = -centery + ny * d1;
    let u2 = centerx + nx * d2;
    let v2 = -centery + ny * d2;
    let lx = u1 + (u2 - u1) * _p;
    let ly = v1 + (v2 - v1) * _p;
    return vec3<f32>(lx * _is, ly * _is, p.z);
}
"#),
};

// =============================================================================
// scry2 (dark-beam)
//   3 user: sides int, star, circle
//   6 init: sina/cosa from 2π/sides, sins/coss from -π/2·star,
//           sinc/cosc from π/2·circle
//   Body: rotates input N-1 times accumulating r2 = max(r2, x·coss +
//     |y|·sins), mixes with circle_v = sqrt(x²+y²) by cosc/sinc, then
//     squares (or signed-squares for sides=2). Scry effect:
//     r1 = r2;  d = r1·(r2 + 1/w);  r = 1/d;  out = (x·r, y·r)
//   Body has 1/w internally — needs_transform divide-out.
// =============================================================================
pub static SCRY2: VariationDef = VariationDef {
    name: "scry2",
    display_name: "Scry 2",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("sides", "Sides", int, 4.0, 1.0, 50.0),
        param!("star", "Star", unlimited_float, 0.0, -10.0, 10.0),
        param!("circle", "Circle", unlimited_float, 0.0, -10.0, 10.0),
    ],
    needs_transform: true,
    writes_color: false,
    // 6 derived values at slots 3..9:
    //   3: sina, 4: cosa, 5: sins, 6: coss, 7: sinc, 8: cosc
    init_param_count: 6,
    wgsl_init: Some(r#"
fn init_scry2(user: array<f32, 3>) -> array<f32, 6> {
    let sides = max(user[0], 1.0);
    let star = user[1];
    let circle_p = user[2];
    let two_pi = 6.28318530717959;
    let half_pi = 1.5707963267948966;
    let a = two_pi / sides;
    let as_ = -half_pi * star;
    let ac = half_pi * circle_p;
    var out: array<f32, 6>;
    out[0] = sin(a);
    out[1] = cos(a);
    out[2] = sin(as_);
    out[3] = cos(as_);
    out[4] = sin(ac);
    out[5] = cos(ac);
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_scry2(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let sides_in = max(get_param(xform_id, variation_id, 0u), 1.0);
    let sina = get_param(xform_id, variation_id, 3u);
    let cosa = get_param(xform_id, variation_id, 4u);
    let sins = get_param(xform_id, variation_id, 5u);
    let coss = get_param(xform_id, variation_id, 6u);
    let sinc = get_param(xform_id, variation_id, 7u);
    let cosc = get_param(xform_id, variation_id, 8u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    var xrt = p.x;
    var yrt = p.y;
    var r2 = xrt * coss + abs(yrt) * sins;
    let circle_v = sqrt(p.x * p.x + p.y * p.y);
    let sides_i = i32(sides_in);
    var i: i32 = 0;
    for (var k: i32 = 0; k < sides_i - 1; k = k + 1) {
        let swp = xrt * cosa - yrt * sina;
        yrt = xrt * sina + yrt * cosa;
        xrt = swp;
        r2 = max(r2, xrt * coss + abs(yrt) * sins);
        i = k + 1;
    }
    r2 = r2 * cosc + circle_v * sinc;
    let r1 = r2;
    if (i > 1) {
        r2 = r2 * r2;
    } else {
        r2 = abs(r2) * r2;
    }
    let d = r1 * (r2 + inv_w);
    let safe_d = select(d, 1e-30, abs(d) < 1e-30);
    let r = 1.0 / safe_d;
    return vec2<f32>(p.x * r * inv_w, p.y * r * inv_w);
}
"#,
    wgsl_3d: Some(r#"
fn variation_scry2(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let sides_in = max(get_param(xform_id, variation_id, 0u), 1.0);
    let sina = get_param(xform_id, variation_id, 3u);
    let cosa = get_param(xform_id, variation_id, 4u);
    let sins = get_param(xform_id, variation_id, 5u);
    let coss = get_param(xform_id, variation_id, 6u);
    let sinc = get_param(xform_id, variation_id, 7u);
    let cosc = get_param(xform_id, variation_id, 8u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    var xrt = p.x;
    var yrt = p.y;
    var r2 = xrt * coss + abs(yrt) * sins;
    let circle_v = sqrt(p.x * p.x + p.y * p.y);
    let sides_i = i32(sides_in);
    var i: i32 = 0;
    for (var k: i32 = 0; k < sides_i - 1; k = k + 1) {
        let swp = xrt * cosa - yrt * sina;
        yrt = xrt * sina + yrt * cosa;
        xrt = swp;
        r2 = max(r2, xrt * coss + abs(yrt) * sins);
        i = k + 1;
    }
    r2 = r2 * cosc + circle_v * sinc;
    let r1 = r2;
    if (i > 1) {
        r2 = r2 * r2;
    } else {
        r2 = abs(r2) * r2;
    }
    let d = r1 * (r2 + inv_w);
    let safe_d = select(d, 1e-30, abs(d) < 1e-30);
    let r = 1.0 / safe_d;
    return vec3<f32>(p.x * r * inv_w, p.y * r * inv_w, p.z);
}
"#),
};
