//! `glynnSShape` (Glynn × Gielis Super-Shape, Java-recovered)
//!
//! Glynn-style transform combined with a Gielis Super-Shape curve.
//! Same structure as `glynnlissa` (batch 72) but uses a Gielis
//! super-shape instead of a Lissajous.
//!
//!   - 11 user params: radius, radius1, thickness, phi1, m, n1, n2,
//!                     n3, scale, pow, contrast
//!   - 3 init slots: _x1 = radius · cos(π·phi1/180);
//!                   _y1 = radius · sin(π·phi1/180);
//!                   _absPow = |pow|
//!
//! supershape(m, n1, n2, n3, phi):
//!   t1 = |cos(m·phi/4)|^n2
//!   t2 = |sin(m·phi/4)|^n3
//!   r  = (t1 + t2)^(1/n1)
//!   if r != 0: invert r and emit (cos phi, sin phi) · (1/r); else (0, 0)
//!
//! cpp PluginVarCalc empty unported_stub; recovered from Java.
//! RNG (2+ calls per iteration). Output factors cleanly through
//! outer multiplier.
//!
//! Source: `output/jwildfire-vars/output/glynnsshape.cpp`.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Glynn warp combined with a Gielis super-shape — same structure as
/// `glynnlissa` but uses a Gielis super-shape `r⁻¹ · (cos φ, sin φ)` (with
/// `m, n1, n2, n3` shape parameters) as the inner-curve sample instead of a
/// Lissajous. The Gielis super-shape can produce a wide variety of
/// symmetric organic shapes (stars, polygons, flowers, etc.) depending on
/// the parameters.
pub static GLYNNSSHAPE: VariationDef = VariationDef {
    name: "glynnSShape",
    display_name: "Glynn S-Shape",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("radius", "Radius", unlimited_float, 1.0, -10.0, 10.0, "Outer cutoff radius. Points inside use the super-shape curve; outside use the Glynn power-warp."),
        param!("radius1", "Radius 1", unlimited_float, 0.5, -10.0, 10.0, "Inner-curve sampling radius. Negative values trigger a small-circle fallback inside the cutoff."),
        param!("thickness", "Thickness", unlimited_float, 1.0, -10.0, 10.0, "Inner-circle thickness (used when `radius1 < 0`)."),
        param!("phi1", "Phi 1", unlimited_float, 0.0, -360.0, 360.0, "Inner-curve center angle, in degrees."),
        param!("m", "M", unlimited_float, 5.0, -50.0, 50.0, "Super-shape symmetry parameter (number of lobes)."),
        param!("n1", "N1", unlimited_float, 1.7, -10.0, 10.0, "Super-shape exponent 1 — outer envelope shape (controls overall sharpness)."),
        param!("n2", "N2", unlimited_float, 1.7, -10.0, 10.0, "Super-shape exponent 2 — controls the cos-side shape."),
        param!("n3", "N3", unlimited_float, 1.7, -10.0, 10.0, "Super-shape exponent 3 — controls the sin-side shape."),
        param!("scale", "Scale", unlimited_float, 0.71, -10.0, 10.0, "Inner-curve scale factor."),
        param!("pow", "Pow", unlimited_float, 1.5, -10.0, 10.0, "Glynn power exponent (absolute value used internally)."),
        param!("contrast", "Contrast", unlimited_float, 0.5, -10.0, 10.0, "Glynn warp probability threshold — higher = more aggressive warping outside the cutoff."),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 3,
    wgsl_init: Some(r#"
fn init_glynnSShape(user: array<f32, 11>) -> array<f32, 3> {
    let pi = 3.14159265358979;
    let phi_rad = user[3] * pi / 180.0;
    var out: array<f32, 3>;
    out[0] = user[0] * cos(phi_rad);
    out[1] = user[0] * sin(phi_rad);
    out[2] = abs(user[9]);
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn glynnSShape_super(m: f32, n1: f32, n2: f32, n3: f32, phi: f32) -> vec2<f32> {
    let half_phi = m * phi * 0.25;
    let t1 = pow(max(abs(cos(half_phi)), 1e-30), n2);
    let t2 = pow(max(abs(sin(half_phi)), 1e-30), n3);
    let safe_n1 = select(n1, 1e-30, abs(n1) < 1e-30);
    let r = pow(max(t1 + t2, 1e-30), 1.0 / safe_n1);
    if (abs(r) < 1e-30) {
        return vec2<f32>(0.0, 0.0);
    }
    let inv_r = 1.0 / r;
    return vec2<f32>(inv_r * cos(phi), inv_r * sin(phi));
}

fn variation_glynnSShape(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let radius = get_param(xform_id, variation_id, 0u);
    let radius1 = get_param(xform_id, variation_id, 1u);
    let thickness = get_param(xform_id, variation_id, 2u);
    let m = get_param(xform_id, variation_id, 4u);
    let n1 = get_param(xform_id, variation_id, 5u);
    let n2 = get_param(xform_id, variation_id, 6u);
    let n3 = get_param(xform_id, variation_id, 7u);
    let scale = get_param(xform_id, variation_id, 8u);
    let contrast = get_param(xform_id, variation_id, 10u);
    let x1 = get_param(xform_id, variation_id, 11u);
    let y1 = get_param(xform_id, variation_id, 12u);
    let abs_pow = get_param(xform_id, variation_id, 13u);
    let two_pi = 6.28318530717959;

    let phi = rng_nextf(rng) * two_pi;
    let sp = glynnSShape_super(m, n1, n2, n3, phi);
    let r = sqrt(p.x * p.x + p.y * p.y);

    if (r < abs(radius)) {
        if (radius1 >= 0.0) {
            return vec2<f32>(sp.x * scale * radius1 + x1, sp.y * scale * radius1 + y1);
        }
        let cr = radius1 * (thickness + (1.0 - thickness) * rng_nextf(rng));
        let cphi = two_pi * rng_nextf(rng);
        return vec2<f32>(cr * cos(cphi) + x1, cr * sin(cphi) + y1);
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
        return vec2<f32>(sp.x * scale * radius1 + x1, sp.y * scale * radius1 + y1);
    }
    return vec2<f32>(xi, yi);
}
"#,
    wgsl_3d: Some(r#"
fn glynnSShape_super(m: f32, n1: f32, n2: f32, n3: f32, phi: f32) -> vec2<f32> {
    let half_phi = m * phi * 0.25;
    let t1 = pow(max(abs(cos(half_phi)), 1e-30), n2);
    let t2 = pow(max(abs(sin(half_phi)), 1e-30), n3);
    let safe_n1 = select(n1, 1e-30, abs(n1) < 1e-30);
    let r = pow(max(t1 + t2, 1e-30), 1.0 / safe_n1);
    if (abs(r) < 1e-30) {
        return vec2<f32>(0.0, 0.0);
    }
    let inv_r = 1.0 / r;
    return vec2<f32>(inv_r * cos(phi), inv_r * sin(phi));
}

fn variation_glynnSShape(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let radius = get_param(xform_id, variation_id, 0u);
    let radius1 = get_param(xform_id, variation_id, 1u);
    let thickness = get_param(xform_id, variation_id, 2u);
    let m = get_param(xform_id, variation_id, 4u);
    let n1 = get_param(xform_id, variation_id, 5u);
    let n2 = get_param(xform_id, variation_id, 6u);
    let n3 = get_param(xform_id, variation_id, 7u);
    let scale = get_param(xform_id, variation_id, 8u);
    let contrast = get_param(xform_id, variation_id, 10u);
    let x1 = get_param(xform_id, variation_id, 11u);
    let y1 = get_param(xform_id, variation_id, 12u);
    let abs_pow = get_param(xform_id, variation_id, 13u);
    let two_pi = 6.28318530717959;

    let phi = rng_nextf(rng) * two_pi;
    let sp = glynnSShape_super(m, n1, n2, n3, phi);
    let r = sqrt(p.x * p.x + p.y * p.y);

    if (r < abs(radius)) {
        if (radius1 >= 0.0) {
            return vec3<f32>(sp.x * scale * radius1 + x1, sp.y * scale * radius1 + y1, p.z);
        }
        let cr = radius1 * (thickness + (1.0 - thickness) * rng_nextf(rng));
        let cphi = two_pi * rng_nextf(rng);
        return vec3<f32>(cr * cos(cphi) + x1, cr * sin(cphi) + y1, p.z);
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
        return vec3<f32>(sp.x * scale * radius1 + x1, sp.y * scale * radius1 + y1, p.z);
    }
    return vec3<f32>(xi, yi, p.z);
}
"#),
};
