//! Apophysis miscellany 8 — five more clean ports
//!
//!   - `csc_squared`        (Whittaker Courtney 2018) — `pow(csc² +
//!                                                         π·m, n) + a`
//!                                                       per-axis scale.
//!                                                       7 user params
//!                                                       recovered from
//!                                                       Java (cpp
//!                                                       APO_VARIABLES
//!                                                       empty)
//!   - `hyperbolicellipse`  (?)                        — `(sinh(x)·cos(a·y),
//!                                                         cosh(x)·sin(a·y))`.
//!                                                       1 user param `a`.
//!                                                       cpp uses
//!                                                       `FPx = …` like
//!                                                       anamorphcyl
//!   - `layered_spiral`     (Will Evans)               — radial spiral
//!                                                       `(a·cos t,
//!                                                         a·sin t)`
//!                                                       with
//!                                                       `t = x²+y²+ε`.
//!                                                       1 user (radius)
//!   - `atan2_spirals`      (Whittaker Courtney 2018)  — 14 user params
//!                                                       recovered from
//!                                                       Java (cpp
//!                                                       APO_VARIABLES
//!                                                       empty); clean
//!   - `gridout2`           (Faber/Faber 2007 +
//!                          Stefanov vars)             — 4 user params;
//!                                                       8-cell quadrant
//!                                                       routing on
//!                                                       round(x)·c vs
//!                                                       round(y)·d
//!
//! Sources: each variation's `.cpp` file in
//! `output/jwildfire-vars/output/`.
//!
//! Faithfulness:
//!   - `csc_squared` cpp APO_VARIABLES is empty — porter omitted all
//!     7 user params while leaving them as struct fields with constants.
//!     Recovered from Java setParameter.
//!   - `atan2_spirals` cpp APO_VARIABLES is empty — porter omitted all
//!     14 user params (12 reals + 2 with no defaults — `_yx_add`,
//!     `_yy_add`, `_sin_add` are zero-default fields). Recovered from
//!     Java setParameter.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// csc_squared (Whittaker Courtney 2018)
//   csc = csc_div / cos(x/cos_div) / tan(x/tan_div)
//   fx = pow(csc² + π·pi_mult, csc_pow) + csc_add
//   out = (x · fx, y · fx · scaley)
// 7 user params (Java-recovered): csc_div, cos_div, tan_div, csc_pow,
//   pi_mult, csc_add, scaley.
// Clean factor through outer.
// =============================================================================
pub static CSC_SQUARED: VariationDef = VariationDef {
    name: "csc_squared",
    display_name: "Csc Squared",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("csc_div", "Csc Div", unlimited_float, 1.0, -10.0, 10.0),
        param!("cos_div", "Cos Div", unlimited_float, 1.0, -10.0, 10.0),
        param!("tan_div", "Tan Div", unlimited_float, 1.0, -10.0, 10.0),
        param!("csc_pow", "Csc Pow", unlimited_float, 0.5, -10.0, 10.0),
        param!("pi_mult", "Pi Mult", unlimited_float, 0.5, -10.0, 10.0),
        param!("csc_add", "Csc Add", unlimited_float, 0.25, -10.0, 10.0),
        param!("scaley", "Scale Y", unlimited_float, 1.0, -10.0, 10.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_csc_squared(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let csc_div = get_param(xform_id, variation_id, 0u);
    let cos_div = get_param(xform_id, variation_id, 1u);
    let tan_div = get_param(xform_id, variation_id, 2u);
    let csc_pow = get_param(xform_id, variation_id, 3u);
    let pi_mult = get_param(xform_id, variation_id, 4u);
    let csc_add = get_param(xform_id, variation_id, 5u);
    let scaley = get_param(xform_id, variation_id, 6u);
    let pi = 3.14159265358979;

    let safe_cos_div = select(cos_div, 1e-30, abs(cos_div) < 1e-30);
    let safe_tan_div = select(tan_div, 1e-30, abs(tan_div) < 1e-30);
    let cx = cos(p.x / safe_cos_div);
    let tx = tan(p.x / safe_tan_div);
    let safe_cx = select(cx, 1e-30, abs(cx) < 1e-30);
    let safe_tx = select(tx, 1e-30, abs(tx) < 1e-30);
    let csc = csc_div / safe_cx / safe_tx;
    let base = max(csc * csc + pi * pi_mult, 1e-30);
    let fx_factor = pow(base, csc_pow) + csc_add;
    return vec2<f32>(p.x * fx_factor, p.y * fx_factor * scaley);
}
"#,
    wgsl_3d: Some(r#"
fn variation_csc_squared(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let csc_div = get_param(xform_id, variation_id, 0u);
    let cos_div = get_param(xform_id, variation_id, 1u);
    let tan_div = get_param(xform_id, variation_id, 2u);
    let csc_pow = get_param(xform_id, variation_id, 3u);
    let pi_mult = get_param(xform_id, variation_id, 4u);
    let csc_add = get_param(xform_id, variation_id, 5u);
    let scaley = get_param(xform_id, variation_id, 6u);
    let pi = 3.14159265358979;

    let safe_cos_div = select(cos_div, 1e-30, abs(cos_div) < 1e-30);
    let safe_tan_div = select(tan_div, 1e-30, abs(tan_div) < 1e-30);
    let cx = cos(p.x / safe_cos_div);
    let tx = tan(p.x / safe_tan_div);
    let safe_cx = select(cx, 1e-30, abs(cx) < 1e-30);
    let safe_tx = select(tx, 1e-30, abs(tx) < 1e-30);
    let csc = csc_div / safe_cx / safe_tx;
    let base = max(csc * csc + pi * pi_mult, 1e-30);
    let fx_factor = pow(base, csc_pow) + csc_add;
    return vec3<f32>(p.x * fx_factor, p.y * fx_factor * scaley, p.z);
}
"#),
};

// =============================================================================
// hyperbolicellipse
//   xt = sinh(x) · cos(a · y)
//   yt = cosh(x) · sin(a · y)
//   out = (xt, yt)
// 1 user param (a). cpp uses `FPx = …` (assignment) — same handling
// as anamorphcyl/ennepers; works as long as it's the only normal
// variation in its transform.
// =============================================================================
pub static HYPERBOLICELLIPSE: VariationDef = VariationDef {
    name: "hyperbolicellipse",
    display_name: "Hyperbolic Ellipse",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("a", "A", unlimited_float, 1.0, -10.0, 10.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_hyperbolicellipse(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let xt = sinh(p.x) * cos(a * p.y);
    let yt = cosh(p.x) * sin(a * p.y);
    return vec2<f32>(xt, yt);
}
"#,
    wgsl_3d: Some(r#"
fn variation_hyperbolicellipse(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let xt = sinh(p.x) * cos(a * p.y);
    let yt = cosh(p.x) * sin(a * p.y);
    return vec3<f32>(xt, yt, p.z);
}
"#),
};

// =============================================================================
// layered_spiral (Will Evans)
//   a = x · radius
//   t = x² + y² + ε
//   out = (a · cos(t), a · sin(t))
// 1 user param (radius); clean.
// =============================================================================
pub static LAYERED_SPIRAL: VariationDef = VariationDef {
    name: "layered_spiral",
    display_name: "Layered Spiral",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("radius", "Radius", unlimited_float, 1.0, -10.0, 10.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_layered_spiral(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let radius = get_param(xform_id, variation_id, 0u);
    let a = p.x * radius;
    let t = p.x * p.x + p.y * p.y + 1e-9;
    return vec2<f32>(a * cos(t), a * sin(t));
}
"#,
    wgsl_3d: Some(r#"
fn variation_layered_spiral(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let radius = get_param(xform_id, variation_id, 0u);
    let a = p.x * radius;
    let t = p.x * p.x + p.y * p.y + 1e-9;
    return vec3<f32>(a * cos(t), a * sin(t), p.z);
}
"#),
};

// =============================================================================
// atan2_spirals (Whittaker Courtney 2018)
//   xy2 = (x² + y²)^x2y2_pow;  r = (x² + y²)^r_power
//   fx = x_mult · atan2(r·r_mult + r_add, xy2·xy2_mult + xy2_add) + x_add
//   fy = sin(atan2(y/yy_div + yy_add, x/yx_div + yx_add) + sin_add) · y_mult
//   if x ≥ 0: out_x = -fx + π
//   else:     out_x =  fx - π
//   out_y = fy
// 14 user params (Java-recovered): r_mult, r_add, xy2_mult, xy2_add,
//   x_mult, x_add, yx_div, yx_add, yy_div, yy_add, sin_add, y_mult,
//   r_power, x2y2_pow
// Clean factor through outer.
// =============================================================================
pub static ATAN2_SPIRALS: VariationDef = VariationDef {
    name: "atan2_spirals",
    display_name: "Atan2 Spirals",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("r_mult", "R Mult", unlimited_float, 1.5, -10.0, 10.0),
        param!("r_add", "R Add", unlimited_float, 1.0, -10.0, 10.0),
        param!("xy2_mult", "XY² Mult", unlimited_float, 1.125, -10.0, 10.0),
        param!("xy2_add", "XY² Add", unlimited_float, 0.132, -10.0, 10.0),
        param!("x_mult", "X Mult", unlimited_float, 2.0, -10.0, 10.0),
        param!("x_add", "X Add", unlimited_float, 0.25, -10.0, 10.0),
        param!("yx_div", "YX Div", unlimited_float, 1.0, -10.0, 10.0),
        param!("yx_add", "YX Add", unlimited_float, 0.0, -10.0, 10.0),
        param!("yy_div", "YY Div", unlimited_float, 1.25, -10.0, 10.0),
        param!("yy_add", "YY Add", unlimited_float, 0.0, -10.0, 10.0),
        param!("sin_add", "Sin Add", unlimited_float, 0.0, -10.0, 10.0),
        param!("y_mult", "Y Mult", unlimited_float, 1.0, -10.0, 10.0),
        param!("r_power", "R Power", unlimited_float, 0.5, -10.0, 10.0),
        param!("x2y2_pow", "X²Y² Pow", unlimited_float, 1.0, -10.0, 10.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_atan2_spirals(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let r_mult = get_param(xform_id, variation_id, 0u);
    let r_add = get_param(xform_id, variation_id, 1u);
    let xy2_mult = get_param(xform_id, variation_id, 2u);
    let xy2_add = get_param(xform_id, variation_id, 3u);
    let x_mult = get_param(xform_id, variation_id, 4u);
    let x_add = get_param(xform_id, variation_id, 5u);
    let yx_div = get_param(xform_id, variation_id, 6u);
    let yx_add = get_param(xform_id, variation_id, 7u);
    let yy_div = get_param(xform_id, variation_id, 8u);
    let yy_add = get_param(xform_id, variation_id, 9u);
    let sin_add = get_param(xform_id, variation_id, 10u);
    let y_mult = get_param(xform_id, variation_id, 11u);
    let r_power = get_param(xform_id, variation_id, 12u);
    let x2y2_pow = get_param(xform_id, variation_id, 13u);
    let pi = 3.14159265358979;

    let r2 = max(p.x * p.x + p.y * p.y, 1e-30);
    let xy2 = pow(r2, x2y2_pow);
    let r = pow(r2, r_power);
    let fx = x_mult * atan2(r * r_mult + r_add, xy2 * xy2_mult + xy2_add) + x_add;
    let safe_yy = select(yy_div, 1e-30, abs(yy_div) < 1e-30);
    let safe_yx = select(yx_div, 1e-30, abs(yx_div) < 1e-30);
    let fy = sin(atan2(p.y / safe_yy + yy_add, p.x / safe_yx + yx_add) + sin_add) * y_mult;
    let out_x = select(fx - pi, -fx + pi, p.x >= 0.0);
    return vec2<f32>(out_x, fy);
}
"#,
    wgsl_3d: Some(r#"
fn variation_atan2_spirals(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let r_mult = get_param(xform_id, variation_id, 0u);
    let r_add = get_param(xform_id, variation_id, 1u);
    let xy2_mult = get_param(xform_id, variation_id, 2u);
    let xy2_add = get_param(xform_id, variation_id, 3u);
    let x_mult = get_param(xform_id, variation_id, 4u);
    let x_add = get_param(xform_id, variation_id, 5u);
    let yx_div = get_param(xform_id, variation_id, 6u);
    let yx_add = get_param(xform_id, variation_id, 7u);
    let yy_div = get_param(xform_id, variation_id, 8u);
    let yy_add = get_param(xform_id, variation_id, 9u);
    let sin_add = get_param(xform_id, variation_id, 10u);
    let y_mult = get_param(xform_id, variation_id, 11u);
    let r_power = get_param(xform_id, variation_id, 12u);
    let x2y2_pow = get_param(xform_id, variation_id, 13u);
    let pi = 3.14159265358979;

    let r2 = max(p.x * p.x + p.y * p.y, 1e-30);
    let xy2 = pow(r2, x2y2_pow);
    let r = pow(r2, r_power);
    let fx = x_mult * atan2(r * r_mult + r_add, xy2 * xy2_mult + xy2_add) + x_add;
    let safe_yy = select(yy_div, 1e-30, abs(yy_div) < 1e-30);
    let safe_yx = select(yx_div, 1e-30, abs(yx_div) < 1e-30);
    let fy = sin(atan2(p.y / safe_yy + yy_add, p.x / safe_yx + yx_add) + sin_add) * y_mult;
    let out_x = select(fx - pi, -fx + pi, p.x >= 0.0);
    return vec3<f32>(out_x, fy, p.z);
}
"#),
};

// =============================================================================
// gridout2 (Faber/Faber 2007, vars added by Stefanov)
//   x = round(p.x) · c;  y = round(p.y) · d
//   8-cell quadrant routing on (x, y) signs picks ±a/x or ±b/y offset
// 4 user params (a, b, c, d). Clean factor through outer.
// (rint = round-to-nearest-even in cpp; WGSL `round` is round-half-away
// from zero. The semantic difference matters only at exactly *.5
// inputs, which for floating-point grids in flame iteration is
// effectively never; treat as equivalent.)
// =============================================================================
pub static GRIDOUT2: VariationDef = VariationDef {
    name: "gridout2",
    display_name: "Grid Out 2",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("a", "A", unlimited_float, 1.0, -10.0, 10.0),
        param!("b", "B", unlimited_float, 1.0, -10.0, 10.0),
        param!("c", "C", unlimited_float, 1.0, -10.0, 10.0),
        param!("d", "D", unlimited_float, 1.0, -10.0, 10.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_gridout2(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let c = get_param(xform_id, variation_id, 2u);
    let d = get_param(xform_id, variation_id, 3u);

    let x = round(p.x) * c;
    let y = round(p.y) * d;

    var fx: f32;
    var fy: f32;
    if (y <= 0.0) {
        if (x > 0.0) {
            if (-y >= x) {
                fx = p.x + a; fy = p.y;
            } else {
                fx = p.x;     fy = p.y + b;
            }
        } else {
            if (y <= x) {
                fx = p.x + a; fy = p.y;
            } else {
                fx = p.x;     fy = p.y - b;
            }
        }
    } else {
        if (x > 0.0) {
            if (y >= x) {
                fx = p.x - a; fy = p.y;
            } else {
                fx = p.x;     fy = p.y + b;
            }
        } else {
            if (y > -x) {
                fx = p.x - a; fy = p.y;
            } else {
                fx = p.x;     fy = p.y - b;
            }
        }
    }
    return vec2<f32>(fx, fy);
}
"#,
    wgsl_3d: Some(r#"
fn variation_gridout2(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let c = get_param(xform_id, variation_id, 2u);
    let d = get_param(xform_id, variation_id, 3u);

    let x = round(p.x) * c;
    let y = round(p.y) * d;

    var fx: f32;
    var fy: f32;
    if (y <= 0.0) {
        if (x > 0.0) {
            if (-y >= x) {
                fx = p.x + a; fy = p.y;
            } else {
                fx = p.x;     fy = p.y + b;
            }
        } else {
            if (y <= x) {
                fx = p.x + a; fy = p.y;
            } else {
                fx = p.x;     fy = p.y - b;
            }
        }
    } else {
        if (x > 0.0) {
            if (y >= x) {
                fx = p.x - a; fy = p.y;
            } else {
                fx = p.x;     fy = p.y + b;
            }
        } else {
            if (y > -x) {
                fx = p.x - a; fy = p.y;
            } else {
                fx = p.x;     fy = p.y - b;
            }
        }
    }
    return vec3<f32>(fx, fy, p.z);
}
"#),
};
