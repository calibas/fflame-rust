//! Sosa attractors batch 2: threepoint_js, lorenz_js, woggle_js
//!
//! More JS-suffix variations from Jesus Sosa (2017), based on Paul
//! Bourke's collection. Each is a Java-recovered cpp port (cpp
//! `PluginVarCalc` was an `unported_stub` or partial).
//!
//!   - threepoint_js: 3-branch IFS triangle (Roger Bagula). 0 user
//!     params. RNG (2 calls/iter for 3-way branch). Body factors
//!     cleanly through outer multiplier.
//!
//!   - lorenz_js: Lorenz attractor Euler-step IFS. 7 user params
//!     (a, b, c, h, centerx, centery, scale) recovered from Java
//!     setParameter (cpp APO_VARIABLES had only 3). 1 init slot
//!     `_bdcs = 1/scale`. Full3D. cpp's TC color write skipped per
//!     writes_color compromise. Body factors cleanly.
//!
//!   - woggle_js (Sosa, based on Paul Bourke's Woggle): N-tile fold
//!     attractor. 1 user param `m` (Java-recovered, default 2,
//!     clamped [2, 12]). cpp's persistent `_a[25]/_b[25]` lookup
//!     tables replaced with runtime `cos(2π·c/m)/sin(2π·c/m)` to
//!     fit our 16-slot init budget. RNG (1 call/iter). Body factors
//!     cleanly.
//!
//! Sources:
//!   - `output/jwildfire-vars/output/threepoint_js.cpp`
//!   - `output/jwildfire-vars/output/lorenz_js.cpp`
//!   - `output/jwildfire-vars/output/woggle_js.cpp`

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// ---------------------------------------------------------------------------
// threepoint_js
// ---------------------------------------------------------------------------

pub static THREEPOINT_JS: VariationDef = VariationDef {
    name: "threepoint_js",
    display_name: "Three Point IFS (JS)",
    category: VariationCategory::Advanced2D,
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
fn variation_threepoint_js(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    let r1 = rng_nextf(rng);
    var x: f32;
    var y: f32;
    if (r1 < 0.333) {
        x = p.x * 0.5 - p.y * 0.5 + 0.5;
        y = -p.x * 0.5 - p.y * 0.5 + 0.5;
    } else {
        let r2 = rng_nextf(rng);
        if (r2 < 0.666) {
            x = p.y;
            y = p.x;
        } else {
            x = -p.y * 0.5 + 0.5;
            y = -p.x * 0.5 + 0.5;
        }
    }
    return vec2<f32>(x, y);
}
"#,
    wgsl_3d: Some(r#"
fn variation_threepoint_js(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    let r1 = rng_nextf(rng);
    var x: f32;
    var y: f32;
    if (r1 < 0.333) {
        x = p.x * 0.5 - p.y * 0.5 + 0.5;
        y = -p.x * 0.5 - p.y * 0.5 + 0.5;
    } else {
        let r2 = rng_nextf(rng);
        if (r2 < 0.666) {
            x = p.y;
            y = p.x;
        } else {
            x = -p.y * 0.5 + 0.5;
            y = -p.x * 0.5 + 0.5;
        }
    }
    return vec3<f32>(x, y, p.z);
}
"#),
};

// ---------------------------------------------------------------------------
// lorenz_js
// ---------------------------------------------------------------------------

pub static LORENZ_JS: VariationDef = VariationDef {
    name: "lorenz_js",
    display_name: "Lorenz (JS)",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("a", "A", unlimited_float, 10.0, -100.0, 100.0),
        param!("b", "B", unlimited_float, 28.0, -100.0, 100.0),
        param!("c", "C", unlimited_float, 1.66, -100.0, 100.0),
        param!("h", "H (step)", unlimited_float, 0.00001, -1.0, 1.0),
        param!("centerx", "Center X", unlimited_float, 0.0, -10.0, 10.0),
        param!("centery", "Center Y", unlimited_float, 0.0, -10.0, 10.0),
        param!("scale", "Scale", unlimited_float, 1000.0, -10000.0, 10000.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 1,
    wgsl_init: Some(r#"
fn init_lorenz_js(user: array<f32, 7>) -> array<f32, 1> {
    var out: array<f32, 1>;
    let scale = user[6];
    let safe_scale = select(scale, 1e-5, abs(scale) < 1e-30);
    out[0] = 1.0 / safe_scale;
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_lorenz_js(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let c = get_param(xform_id, variation_id, 2u);
    let h = get_param(xform_id, variation_id, 3u);
    let x = p.x + h * a * (p.y - p.x);
    let y = p.y + h * (p.x * b - p.y);
    return vec2<f32>(x, y);
}
"#,
    wgsl_3d: Some(r#"
fn variation_lorenz_js(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let c = get_param(xform_id, variation_id, 2u);
    let h = get_param(xform_id, variation_id, 3u);
    let x = p.x + h * a * (p.y - p.x);
    let y = p.y + h * (p.x * (b - p.z) - p.y);
    let z = p.z + h * (p.x * p.y - c * p.z);
    return vec3<f32>(x, y, z);
}
"#),
};

// ---------------------------------------------------------------------------
// woggle_js
// ---------------------------------------------------------------------------

pub static WOGGLE_JS: VariationDef = VariationDef {
    name: "woggle_js",
    display_name: "Woggle (JS)",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("m", "M", int, 2.0, 2.0, 12.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_woggle_js(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let m = clamp(i32(get_param(xform_id, variation_id, 0u)), 2, 12);
    let mf = f32(m);
    let two_pi = 6.28318530717959;
    let r = sqrt(1.25) * sqrt(mf);
    let safe_r = select(r, 1e-30, abs(r) < 1e-30);
    let c = i32(rng_nextf(rng) * mf);
    let c_clamped = clamp(c, 0, m - 1);
    let theta = two_pi * f32(c_clamped) / mf;
    let a_c = cos(theta);
    let b_c = sin(theta);
    let r2 = sqrt(p.x * p.x + p.y * p.y);
    let safe_r2 = select(r2, 1e-30, abs(r2) < 1e-30);
    let ra = 1.0 / (sqrt(3.0) * safe_r2);
    var x: f32;
    var y: f32;
    if ((c_clamped & 1) == 0) {
        x = -p.x / safe_r + ra * p.y / safe_r + a_c;
        y = -ra * p.x / safe_r - p.y / safe_r + b_c;
    } else {
        x = p.x / safe_r + ra * p.y / safe_r + a_c;
        y = -ra * p.x / safe_r + p.y / safe_r + b_c;
    }
    return vec2<f32>(x, y);
}
"#,
    wgsl_3d: Some(r#"
fn variation_woggle_js(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let m = clamp(i32(get_param(xform_id, variation_id, 0u)), 2, 12);
    let mf = f32(m);
    let two_pi = 6.28318530717959;
    let r = sqrt(1.25) * sqrt(mf);
    let safe_r = select(r, 1e-30, abs(r) < 1e-30);
    let c = i32(rng_nextf(rng) * mf);
    let c_clamped = clamp(c, 0, m - 1);
    let theta = two_pi * f32(c_clamped) / mf;
    let a_c = cos(theta);
    let b_c = sin(theta);
    let r2 = sqrt(p.x * p.x + p.y * p.y);
    let safe_r2 = select(r2, 1e-30, abs(r2) < 1e-30);
    let ra = 1.0 / (sqrt(3.0) * safe_r2);
    var x: f32;
    var y: f32;
    if ((c_clamped & 1) == 0) {
        x = -p.x / safe_r + ra * p.y / safe_r + a_c;
        y = -ra * p.x / safe_r - p.y / safe_r + b_c;
    } else {
        x = p.x / safe_r + ra * p.y / safe_r + a_c;
        y = -ra * p.x / safe_r + p.y / safe_r + b_c;
    }
    return vec3<f32>(x, y, p.z);
}
"#),
};
