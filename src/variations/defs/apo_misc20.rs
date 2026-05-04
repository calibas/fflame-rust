//! Apophysis miscellany batch 20: cannabiscurve_wf, spherical3D_wf, swirl3D_wf
//!
//!   - cannabiscurve_wf: cannabis-curve polar plot (mathworld.wolfram.com
//!     /CannabisCurve.html). 1 user param `filled` (int). RNG when
//!     `filled == 1` (multiplies r by random01). Body factors cleanly
//!     through outer multiplier; Z passes through.
//!
//!   - spherical3D_wf: 3D spherical inversion with adjustable exponent.
//!     2 user params (invert int, exponent) + 1 init slot
//!     (_regularForm = |exponent - 2| < ε, stored as float). Body
//!     factors cleanly. Full3D.
//!
//!   - swirl3D_wf (Maschke): 3D swirl with z-modulation. 1 user param
//!     `n`. No init. Body factors cleanly; cpp also writes color (TC),
//!     skipped here per `writes_color`-model conflict (compromise
//!     established in batch 60 for spirograph3D).
//!
//! Sources:
//!   - `output/jwildfire-vars/output/cannabiscurve_wf.cpp`
//!   - `output/jwildfire-vars/output/spherical3d_wf.cpp`
//!   - `output/jwildfire-vars/output/swirl3d_wf.cpp`

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// ---------------------------------------------------------------------------
// cannabiscurve_wf
// ---------------------------------------------------------------------------

pub static CANNABISCURVE_WF: VariationDef = VariationDef {
    name: "cannabiscurve_wf",
    display_name: "Cannabis Curve WF",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("filled", "Filled", int, 1.0, 0.0, 1.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_cannabiscurve_wf(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let filled = i32(get_param(xform_id, variation_id, 0u));
    var a = atan2(p.x, p.y);
    var r = (1.0 + 0.9 * cos(8.0 * a))
          * (1.0 + 0.1 * cos(24.0 * a))
          * (0.9 + 0.1 * cos(200.0 * a))
          * (1.0 + sin(a));
    a = a + 1.5707963267948966;
    if (filled == 1) {
        r = r * rng_nextf(rng);
    }
    return vec2<f32>(sin(a) * r, cos(a) * r);
}
"#,
    wgsl_3d: Some(r#"
fn variation_cannabiscurve_wf(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let filled = i32(get_param(xform_id, variation_id, 0u));
    var a = atan2(p.x, p.y);
    var r = (1.0 + 0.9 * cos(8.0 * a))
          * (1.0 + 0.1 * cos(24.0 * a))
          * (0.9 + 0.1 * cos(200.0 * a))
          * (1.0 + sin(a));
    a = a + 1.5707963267948966;
    if (filled == 1) {
        r = r * rng_nextf(rng);
    }
    return vec3<f32>(sin(a) * r, cos(a) * r, p.z);
}
"#),
};

// ---------------------------------------------------------------------------
// spherical3D_wf
// ---------------------------------------------------------------------------

pub static SPHERICAL3D_WF: VariationDef = VariationDef {
    name: "spherical3D_wf",
    display_name: "Spherical 3D WF",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("invert", "Invert", int, 0.0, 0.0, 1.0),
        param!("exponent", "Exponent", unlimited_float, 2.0, -10.0, 10.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 1,
    wgsl_init: Some(r#"
fn init_spherical3D_wf(user: array<f32, 2>) -> array<f32, 1> {
    var out: array<f32, 1>;
    if (abs(user[1] - 2.0) < 1e-6) {
        out[0] = 1.0;
    } else {
        out[0] = 0.0;
    }
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_spherical3D_wf(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let invert = i32(get_param(xform_id, variation_id, 0u));
    let exponent = get_param(xform_id, variation_id, 1u);
    let regular = get_param(xform_id, variation_id, 2u) > 0.5;
    let small = 1e-30;
    let denom = p.x * p.x + p.y * p.y + small;
    var r: f32;
    if (regular) {
        r = 1.0 / denom;
    } else {
        r = 1.0 / pow(max(denom, small), exponent * 0.5);
    }
    if (invert != 0) {
        r = -r;
    }
    return vec2<f32>(p.x * r, p.y * r);
}
"#,
    wgsl_3d: Some(r#"
fn variation_spherical3D_wf(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let invert = i32(get_param(xform_id, variation_id, 0u));
    let exponent = get_param(xform_id, variation_id, 1u);
    let regular = get_param(xform_id, variation_id, 2u) > 0.5;
    let small = 1e-30;
    let denom = p.x * p.x + p.y * p.y + p.z * p.z + small;
    var r: f32;
    if (regular) {
        r = 1.0 / denom;
    } else {
        r = 1.0 / pow(max(denom, small), exponent * 0.5);
    }
    if (invert != 0) {
        r = -r;
    }
    return vec3<f32>(p.x * r, p.y * r, p.z * r);
}
"#),
};

// ---------------------------------------------------------------------------
// swirl3D_wf
// ---------------------------------------------------------------------------

pub static SWIRL3D_WF: VariationDef = VariationDef {
    name: "swirl3D_wf",
    display_name: "Swirl 3D WF",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("n", "N", unlimited_float, 0.0, -10.0, 10.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_swirl3D_wf(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let small = 1e-30;
    let rad = sqrt(p.x * p.x + p.y * p.y) + small;
    let ang = atan2(p.x, p.y);
    return vec2<f32>(rad * cos(ang), rad * sin(ang));
}
"#,
    wgsl_3d: Some(r#"
fn variation_swirl3D_wf(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let n = get_param(xform_id, variation_id, 0u);
    let small = 1e-30;
    let rad = sqrt(p.x * p.x + p.y * p.y) + small;
    let ang = atan2(p.x, p.y);
    return vec3<f32>(
        rad * cos(ang),
        rad * sin(ang),
        sin(6.0 * cos(rad) - n * ang),
    );
}
"#),
};
