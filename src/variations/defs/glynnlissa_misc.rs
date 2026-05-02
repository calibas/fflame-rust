//! `glynnlissa`
//!
//! Glynn-style transform combined with a Lissajous curve. Inside
//! the cutoff radius `|radius|` either samples a Lissajous point
//! (when `radius1 ≥ 0`) or a small circle (when `radius1 < 0`).
//! Outside the radius, applies the Glynn power-warp with
//! probabilistic toggle, then secondary Lissajous fill if the
//! warped point falls inside the inner Lissajous disc.
//!
//!   - 11 user params: radius, radius1, thickness, phi1, a, b,
//!                     width, phase, scale, pow, contrast
//!   - 3 init slots: _x1 = radius · cos(π·phi1/180);
//!                   _y1 = radius · sin(π·phi1/180);
//!                   _absPow = |pow|
//!
//! Body: cpp PluginVarCalc empty unported_stub; recovered from Java.
//! RNG (3+ calls per iteration). Output factors cleanly through
//! outer multiplier.
//!
//! Source: `output/jwildfire-vars/output/glynnlissa.cpp`.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

pub static GLYNNLISSA: VariationDef = VariationDef {
    name: "glynnlissa",
    display_name: "Glynn Lissa",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("radius", "Radius", unlimited_float, 1.0, -10.0, 10.0),
        param!("radius1", "Radius 1", unlimited_float, 0.5, -10.0, 10.0),
        param!("thickness", "Thickness", unlimited_float, 1.0, -10.0, 10.0),
        param!("phi1", "Phi 1", unlimited_float, 0.0, -360.0, 360.0),
        param!("a", "A", unlimited_float, 3.0, -50.0, 50.0),
        param!("b", "B", unlimited_float, 2.0, -50.0, 50.0),
        param!("width", "Width", unlimited_float, 0.0, -10.0, 10.0),
        param!("phase", "Phase", unlimited_float, 0.0, -10.0, 10.0),
        param!("scale", "Scale", unlimited_float, 0.71, -10.0, 10.0),
        param!("pow", "Pow", unlimited_float, 1.5, -10.0, 10.0),
        param!("contrast", "Contrast", unlimited_float, 0.5, -10.0, 10.0),
    ],
    needs_transform: false,
    writes_color: false,
    // 3 derived values at slots 11..14: _x1, _y1, _absPow
    init_param_count: 3,
    wgsl_init: Some(r#"
fn init_glynnlissa(user: array<f32, 11>) -> array<f32, 3> {
    let pi = 3.14159265358979;
    let phi_rad = user[3] * pi / 180.0;
    var out: array<f32, 3>;
    out[0] = user[0] * cos(phi_rad);
    out[1] = user[0] * sin(phi_rad);
    out[2] = abs(user[9]);
    return out;
}
"#),
    wgsl_2d: r#"
fn variation_glynnlissa(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let radius = get_param(xform_id, variation_id, 0u);
    let radius1 = get_param(xform_id, variation_id, 1u);
    let thickness = get_param(xform_id, variation_id, 2u);
    let a_p = get_param(xform_id, variation_id, 4u);
    let b_p = get_param(xform_id, variation_id, 5u);
    let width = get_param(xform_id, variation_id, 6u);
    let phase = get_param(xform_id, variation_id, 7u);
    let scale = get_param(xform_id, variation_id, 8u);
    let contrast = get_param(xform_id, variation_id, 10u);
    let x1 = get_param(xform_id, variation_id, 11u);
    let y1 = get_param(xform_id, variation_id, 12u);
    let abs_pow = get_param(xform_id, variation_id, 13u);
    let two_pi = 6.28318530717959;

    let t = rng_nextf(rng) * two_pi;
    let lx = sin(a_p * t + phase);
    let ly = sin(b_p * t);
    let r = sqrt(p.x * p.x + p.y * p.y);
    let y_jit = rng_nextf(rng) - 0.5;

    if (r < abs(radius)) {
        if (radius1 >= 0.0) {
            return vec2<f32>((lx * scale + width * y_jit) * radius1 + x1,
                             (ly * scale + width * y_jit) * radius1 + y1);
        }
        // radius1 < 0: small circle
        let cr = radius1 * (thickness + (1.0 - thickness) * rng_nextf(rng));
        let phi = two_pi * rng_nextf(rng);
        return vec2<f32>(cr * cos(phi) + x1, cr * sin(phi) + y1);
    }

    let alpha = abs(radius) / max(r, 1e-30);
    var xi: f32;
    var yi: f32;
    if (rng_nextf(rng) > contrast * pow(max(alpha, 1e-30), abs_pow)) {
        xi = p.x;
        yi = p.y;
    } else {
        xi = alpha * alpha * p.x;
        yi = alpha * alpha * p.y;
    }
    let dx = xi - x1;
    let dy = yi - y1;
    let z = dx * dx + dy * dy;
    if (z < radius1 * radius1) {
        return vec2<f32>((lx * scale + width * y_jit) * radius1 + x1,
                         (ly * scale + width * y_jit) * radius1 + y1);
    }
    return vec2<f32>(xi, yi);
}
"#,
    wgsl_3d: Some(r#"
fn variation_glynnlissa(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let radius = get_param(xform_id, variation_id, 0u);
    let radius1 = get_param(xform_id, variation_id, 1u);
    let thickness = get_param(xform_id, variation_id, 2u);
    let a_p = get_param(xform_id, variation_id, 4u);
    let b_p = get_param(xform_id, variation_id, 5u);
    let width = get_param(xform_id, variation_id, 6u);
    let phase = get_param(xform_id, variation_id, 7u);
    let scale = get_param(xform_id, variation_id, 8u);
    let contrast = get_param(xform_id, variation_id, 10u);
    let x1 = get_param(xform_id, variation_id, 11u);
    let y1 = get_param(xform_id, variation_id, 12u);
    let abs_pow = get_param(xform_id, variation_id, 13u);
    let two_pi = 6.28318530717959;

    let t = rng_nextf(rng) * two_pi;
    let lx = sin(a_p * t + phase);
    let ly = sin(b_p * t);
    let r = sqrt(p.x * p.x + p.y * p.y);
    let y_jit = rng_nextf(rng) - 0.5;

    if (r < abs(radius)) {
        if (radius1 >= 0.0) {
            return vec3<f32>((lx * scale + width * y_jit) * radius1 + x1,
                             (ly * scale + width * y_jit) * radius1 + y1,
                             p.z);
        }
        let cr = radius1 * (thickness + (1.0 - thickness) * rng_nextf(rng));
        let phi = two_pi * rng_nextf(rng);
        return vec3<f32>(cr * cos(phi) + x1, cr * sin(phi) + y1, p.z);
    }

    let alpha = abs(radius) / max(r, 1e-30);
    var xi: f32;
    var yi: f32;
    if (rng_nextf(rng) > contrast * pow(max(alpha, 1e-30), abs_pow)) {
        xi = p.x;
        yi = p.y;
    } else {
        xi = alpha * alpha * p.x;
        yi = alpha * alpha * p.y;
    }
    let dx = xi - x1;
    let dy = yi - y1;
    let z = dx * dx + dy * dy;
    if (z < radius1 * radius1) {
        return vec3<f32>((lx * scale + width * y_jit) * radius1 + x1,
                         (ly * scale + width * y_jit) * radius1 + y1,
                         p.z);
    }
    return vec3<f32>(xi, yi, p.z);
}
"#),
};
