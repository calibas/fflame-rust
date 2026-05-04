//! WF-suffix simple curves: epispiral_wf, cloverleaf_wf, rose_wf, bubble_wf
//!
//! Four polar-curve and bubble-shape variations from JWildfire's WF
//! (Wildfire) family. All four are simple, clean ports — body factors
//! cleanly through the outer multiplier.
//!
//!   - epispiral_wf (Maschke): epispiral curve `r = 0.5 / cos(waves·a)`.
//!     1 user param `waves` (default 4). Returns (0, 0) when the cosine
//!     hits zero (cpp early-returns leaving FPx/FPy unchanged; closest
//!     equivalent in our outer-multiplier convention is to add nothing).
//!     Preserves cpp's `atan2(x, y)` swap.
//!
//!   - cloverleaf_wf (Maschke): cloverleaf curve `r = sin(2a) +
//!     0.25·sin(6a)`. 1 user param `filled` (int, default 1). RNG when
//!     filled==1. Preserves cpp's `atan2(x, y)` swap.
//!
//!   - rose_wf (Maschke): rose curve `r = amp · cos(waves·a)`. 3 user
//!     params (amp default 0.5, waves int default 4, filled int default 0).
//!     RNG when filled==1. Preserves cpp's `atan2(x, y)` swap.
//!
//!   - bubble_wf (Maschke): standard bubble inversion plus a random
//!     ±z bump (`±(2/r − 1)`). 0 user params. RNG (1 call/iter for
//!     z-sign). Full3D.
//!
//! Sources:
//!   - `output/jwildfire-vars/output/epispiral_wf.cpp`
//!   - `output/jwildfire-vars/output/cloverleaf_wf.cpp`
//!   - `output/jwildfire-vars/output/rose_wf.cpp`
//!   - `output/jwildfire-vars/output/bubble_wf.cpp`

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// ---------------------------------------------------------------------------
// epispiral_wf
// ---------------------------------------------------------------------------

pub static EPISPIRAL_WF: VariationDef = VariationDef {
    name: "epispiral_wf",
    display_name: "Epispiral WF",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("waves", "Waves", unlimited_float, 4.0, -50.0, 50.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_epispiral_wf(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let waves = get_param(xform_id, variation_id, 0u);
    let a = atan2(p.x, p.y);
    let d = cos(waves * a);
    if (abs(d) < 1e-30) {
        return vec2<f32>(0.0, 0.0);
    }
    let r = 0.5 / d;
    return vec2<f32>(sin(a) * r, cos(a) * r);
}
"#,
    wgsl_3d: Some(r#"
fn variation_epispiral_wf(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let waves = get_param(xform_id, variation_id, 0u);
    let a = atan2(p.x, p.y);
    let d = cos(waves * a);
    if (abs(d) < 1e-30) {
        return vec3<f32>(0.0, 0.0, p.z);
    }
    let r = 0.5 / d;
    return vec3<f32>(sin(a) * r, cos(a) * r, p.z);
}
"#),
};

// ---------------------------------------------------------------------------
// cloverleaf_wf
// ---------------------------------------------------------------------------

pub static CLOVERLEAF_WF: VariationDef = VariationDef {
    name: "cloverleaf_wf",
    display_name: "Cloverleaf WF",
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
fn variation_cloverleaf_wf(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let filled = i32(get_param(xform_id, variation_id, 0u));
    let a = atan2(p.x, p.y);
    var r = sin(2.0 * a) + 0.25 * sin(6.0 * a);
    if (filled == 1) {
        r = r * rng_nextf(rng);
    }
    return vec2<f32>(sin(a) * r, cos(a) * r);
}
"#,
    wgsl_3d: Some(r#"
fn variation_cloverleaf_wf(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let filled = i32(get_param(xform_id, variation_id, 0u));
    let a = atan2(p.x, p.y);
    var r = sin(2.0 * a) + 0.25 * sin(6.0 * a);
    if (filled == 1) {
        r = r * rng_nextf(rng);
    }
    return vec3<f32>(sin(a) * r, cos(a) * r, p.z);
}
"#),
};

// ---------------------------------------------------------------------------
// rose_wf
// ---------------------------------------------------------------------------

pub static ROSE_WF: VariationDef = VariationDef {
    name: "rose_wf",
    display_name: "Rose WF",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("amp", "Amplitude", unlimited_float, 0.5, -10.0, 10.0),
        param!("waves", "Waves", int, 4.0, -50.0, 50.0),
        param!("filled", "Filled", int, 0.0, 0.0, 1.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_rose_wf(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let amp = get_param(xform_id, variation_id, 0u);
    let waves = get_param(xform_id, variation_id, 1u);
    let filled = i32(get_param(xform_id, variation_id, 2u));
    let a = atan2(p.x, p.y);
    var r = amp * cos(waves * a);
    if (filled == 1) {
        r = r * rng_nextf(rng);
    }
    return vec2<f32>(sin(a) * r, cos(a) * r);
}
"#,
    wgsl_3d: Some(r#"
fn variation_rose_wf(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let amp = get_param(xform_id, variation_id, 0u);
    let waves = get_param(xform_id, variation_id, 1u);
    let filled = i32(get_param(xform_id, variation_id, 2u));
    let a = atan2(p.x, p.y);
    var r = amp * cos(waves * a);
    if (filled == 1) {
        r = r * rng_nextf(rng);
    }
    return vec3<f32>(sin(a) * r, cos(a) * r, p.z);
}
"#),
};

// ---------------------------------------------------------------------------
// bubble_wf
// ---------------------------------------------------------------------------

pub static BUBBLE_WF: VariationDef = VariationDef {
    name: "bubble_wf",
    display_name: "Bubble WF",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_bubble_wf(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    let r = (p.x * p.x + p.y * p.y) * 0.25 + 1.0;
    let safe_r = select(r, 1e-30, abs(r) < 1e-30);
    let t = 1.0 / safe_r;
    return vec2<f32>(t * p.x, t * p.y);
}
"#,
    wgsl_3d: Some(r#"
fn variation_bubble_wf(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    let r = (p.x * p.x + p.y * p.y) * 0.25 + 1.0;
    let safe_r = select(r, 1e-30, abs(r) < 1e-30);
    let t = 1.0 / safe_r;
    let z_bump = 2.0 / safe_r - 1.0;
    let z = select(z_bump, -z_bump, rng_nextf(rng) < 0.5);
    return vec3<f32>(t * p.x, t * p.y, z);
}
"#),
};
