//! Sosa attractors batch 3: lace_js, wallpaper_js
//!
//!   - lace_js (Sosa, based on Paul Bourke's lace.c): 4-branch
//!     Sierpinski-triangle-like IFS with three pivot points at the
//!     vertices of an equilateral triangle. 0 user params. RNG
//!     (1 call/iter for 4-way branch). Body factors cleanly through
//!     outer multiplier.
//!
//!   - wallpaper_js (Sosa, based on Paul Bourke's wallpaper): random
//!     blend between Mira-style sqrt warp and identity. 3 user params
//!     (a, b, c) recovered from Java setParameter (cpp APO_VARIABLES
//!     was empty). RNG (1 call/iter). Body lacks VVAR scaling on both
//!     branches — uses needs_transform divide-out so outer × body
//!     restores the unweighted cpp output. cpp's setBrightness(300)
//!     side effect ignored (renderer setting, not part of the
//!     variation transform).
//!
//! Sources:
//!   - `output/jwildfire-vars/output/lace_js.cpp`
//!   - `output/jwildfire-vars/output/wallpaper_js.cpp`

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// ---------------------------------------------------------------------------
// lace_js
// ---------------------------------------------------------------------------

pub static LACE_JS: VariationDef = VariationDef {
    name: "lace_js",
    display_name: "Lace (JS)",
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
fn variation_lace_js(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    let r = 2.0;
    let inv_r = 0.5;
    let sqrt3_2 = 0.8660254037844386;
    let r0 = sqrt(p.x * p.x + p.y * p.y);
    let weight = rng_nextf(rng);
    var x: f32;
    var y: f32;
    var w: f32;
    if (weight > 0.75) {
        w = atan2(p.y, p.x - 1.0);
        y = -r0 * cos(w) * inv_r + 1.0;
        x = -r0 * sin(w) * inv_r;
    } else if (weight > 0.50) {
        w = atan2(p.y - sqrt3_2, p.x + 0.5);
        y = -r0 * cos(w) * inv_r - 0.5;
        x = -r0 * sin(w) * inv_r + sqrt3_2;
    } else if (weight > 0.25) {
        w = atan2(p.y + sqrt3_2, p.x + 0.5);
        y = -r0 * cos(w) * inv_r - 0.5;
        x = -r0 * sin(w) * inv_r - sqrt3_2;
    } else {
        w = atan2(p.y, p.x);
        y = -r0 * cos(w) * inv_r;
        x = -r0 * sin(w) * inv_r;
    }
    return vec2<f32>(x, y);
}
"#,
    wgsl_3d: Some(r#"
fn variation_lace_js(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    let r = 2.0;
    let inv_r = 0.5;
    let sqrt3_2 = 0.8660254037844386;
    let r0 = sqrt(p.x * p.x + p.y * p.y);
    let weight = rng_nextf(rng);
    var x: f32;
    var y: f32;
    var w: f32;
    if (weight > 0.75) {
        w = atan2(p.y, p.x - 1.0);
        y = -r0 * cos(w) * inv_r + 1.0;
        x = -r0 * sin(w) * inv_r;
    } else if (weight > 0.50) {
        w = atan2(p.y - sqrt3_2, p.x + 0.5);
        y = -r0 * cos(w) * inv_r - 0.5;
        x = -r0 * sin(w) * inv_r + sqrt3_2;
    } else if (weight > 0.25) {
        w = atan2(p.y + sqrt3_2, p.x + 0.5);
        y = -r0 * cos(w) * inv_r - 0.5;
        x = -r0 * sin(w) * inv_r - sqrt3_2;
    } else {
        w = atan2(p.y, p.x);
        y = -r0 * cos(w) * inv_r;
        x = -r0 * sin(w) * inv_r;
    }
    return vec3<f32>(x, y, p.z);
}
"#),
};

// ---------------------------------------------------------------------------
// wallpaper_js
// ---------------------------------------------------------------------------

pub static WALLPAPER_JS: VariationDef = VariationDef {
    name: "wallpaper_js",
    display_name: "Wallpaper (JS)",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("a", "A", unlimited_float, 1.156, -10.0, 10.0),
        param!("b", "B", unlimited_float, -0.28, -10.0, 10.0),
        param!("c", "C", unlimited_float, 21.288, -100.0, 100.0),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn wallpaper_signum(v: f32) -> f32 {
    if (v < 0.0) { return -1.0; }
    if (v > 0.0) { return 1.0; }
    return 0.0;
}

fn variation_wallpaper_js(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let c = get_param(xform_id, variation_id, 2u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    if (rng_nextf(rng) < 0.5) {
        let trunc_x = trunc(p.x);
        let s = wallpaper_signum(trunc_x);
        let nx = p.y - s * sqrt(abs(b * p.x - c));
        let ny = a - p.x;
        return vec2<f32>(nx * inv_w, ny * inv_w);
    }
    return vec2<f32>(p.x * inv_w, p.y * inv_w);
}
"#,
    wgsl_3d: Some(r#"
fn wallpaper_signum(v: f32) -> f32 {
    if (v < 0.0) { return -1.0; }
    if (v > 0.0) { return 1.0; }
    return 0.0;
}

fn variation_wallpaper_js(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let c = get_param(xform_id, variation_id, 2u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    if (rng_nextf(rng) < 0.5) {
        let trunc_x = trunc(p.x);
        let s = wallpaper_signum(trunc_x);
        let nx = p.y - s * sqrt(abs(b * p.x - c));
        let ny = a - p.x;
        return vec3<f32>(nx * inv_w, ny * inv_w, p.z);
    }
    return vec3<f32>(p.x * inv_w, p.y * inv_w, p.z);
}
"#),
};
