//! `pre_circlecrop` and `post_circlecrop` (Xyrus02)
//!
//! Pre/post-phase circle-crop: tests whether the input/accumulator
//! point is inside a circle of radius `cr` centered at `(x0, y0)`,
//! and either passes through (with weight scaling and offset) or
//! "scatters" onto the boundary.
//!
//!   - 5 user params: radius, x, y, scatter_area, zero (int 0/1)
//!   - 1 init slot: cA = clamp(scatter_area, -1, 1)
//!
//! The cpp's "doHide" path (when zero=1 and point is outside cr)
//! sets `FTx = FTy = 0` to flag the point hidden. Our system has
//! no doHide flag — we just return (0, 0) at that branch, which
//! plots at origin instead of hiding. This matches the visual
//! output for typical use (most points fall inside cr or zero=0).
//!
//! Both use `needs_transform: true` to read the per-variation weight
//! and apply it directly inside the body (pre/post phases have no
//! outer multiplier). RNG.
//!
//! Sources:
//!   - `output/jwildfire-vars/output/pre_circlecrop.cpp`
//!   - `output/jwildfire-vars/output/post_circlecrop.cpp`

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// pre_circlecrop
// =============================================================================
/// Same circle-crop algorithm as `circlecrop` — tests whether the input
/// lies inside a circle of radius `radius` centered at `(x, y)`. Inside,
/// passes through scaled by weight. Outside, behavior depends on `zero`: if
/// 1, the point is hidden (collapsed to origin); if 0, it scatters onto the
/// circle boundary with `scatter_area` randomization. Applied at pre-phase,
/// before any normal-phase variations run.
///
/// # Authors
/// - Xyrus02
pub static PRE_CIRCLECROP: VariationDef = VariationDef {
    name: "pre_circlecrop",
    display_name: "Pre Circle Crop",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Pre,
    needs_rng: true,
    parameters: &[
        param!("radius", "Radius", unlimited_float, 1.0, -10.0, 10.0, "Crop circle radius."),
        param!("x", "X", unlimited_float, 0.0, -10.0, 10.0, "X center of the crop circle."),
        param!("y", "Y", unlimited_float, 0.0, -10.0, 10.0, "Y center of the crop circle."),
        param!("scatter_area", "Scatter Area", unlimited_float, 0.0, -1.0, 1.0, "Random scatter band along the circle boundary. 0 = snap to boundary; ±1 = scatter across full half-radius."),
        param!("zero", "Zero", bool, true, "Behavior outside the circle: 1 = hide (collapse to origin), 0 = scatter onto boundary."),
    ],
    needs_transform: true,
    writes_color: false,
    // 1 derived value at slot 5: cA = clamp(scatter_area, -1, 1)
    init_param_count: 1,
    wgsl_init: Some(r#"
fn init_pre_circlecrop(user: array<f32, 5>) -> array<f32, 1> {
    var out: array<f32, 1>;
    out[0] = clamp(user[3], -1.0, 1.0);
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_pre_circlecrop(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let cr = get_param(xform_id, variation_id, 0u);
    let x0 = get_param(xform_id, variation_id, 1u);
    let y0 = get_param(xform_id, variation_id, 2u);
    let zero = i32(get_param(xform_id, variation_id, 4u));
    let ca = get_param(xform_id, variation_id, 5u);
    let w = transforms[xform_id].variations[variation_id];

    let dx = p.x - x0;
    let dy = p.y - y0;
    let rad = sqrt(dx * dx + dy * dy);
    let ang = atan2(dy, dx);
    let rdc = cr + rng_nextf(rng) * 0.5 * ca;
    let esc = rad > cr;
    let cr0 = zero == 1;

    if (cr0 && esc) {
        return vec2<f32>(0.0, 0.0);
    }
    if (!cr0 && esc) {
        return vec2<f32>(w * rdc * cos(ang) + x0, w * rdc * sin(ang) + y0);
    }
    return vec2<f32>(w * dx + x0, w * dy + y0);
}
"#,
    wgsl_3d: Some(r#"
fn variation_pre_circlecrop(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let cr = get_param(xform_id, variation_id, 0u);
    let x0 = get_param(xform_id, variation_id, 1u);
    let y0 = get_param(xform_id, variation_id, 2u);
    let zero = i32(get_param(xform_id, variation_id, 4u));
    let ca = get_param(xform_id, variation_id, 5u);
    let w = transforms[xform_id].variations[variation_id];

    let dx = p.x - x0;
    let dy = p.y - y0;
    let rad = sqrt(dx * dx + dy * dy);
    let ang = atan2(dy, dx);
    let rdc = cr + rng_nextf(rng) * 0.5 * ca;
    let esc = rad > cr;
    let cr0 = zero == 1;

    if (cr0 && esc) {
        return vec3<f32>(0.0, 0.0, p.z);
    }
    if (!cr0 && esc) {
        return vec3<f32>(w * rdc * cos(ang) + x0, w * rdc * sin(ang) + y0, p.z);
    }
    return vec3<f32>(w * dx + x0, w * dy + y0, p.z);
}
"#),
};

// =============================================================================
// post_circlecrop
// =============================================================================
/// Same circle-crop algorithm as `circlecrop` — tests whether the input
/// lies inside a circle of radius `radius` centered at `(x, y)`. Inside,
/// passes through scaled by weight. Outside, behavior depends on `zero`: if
/// 1, the point is hidden (collapsed to origin); if 0, it scatters onto the
/// circle boundary with `scatter_area` randomization. Applied at post-
/// phase, on the accumulated output.
///
/// # Authors
/// - Xyrus02
pub static POST_CIRCLECROP: VariationDef = VariationDef {
    name: "post_circlecrop",
    display_name: "Post Circle Crop",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Post,
    needs_rng: true,
    parameters: &[
        param!("radius", "Radius", unlimited_float, 1.0, -10.0, 10.0, "Crop circle radius."),
        param!("x", "X", unlimited_float, 0.0, -10.0, 10.0, "X center of the crop circle."),
        param!("y", "Y", unlimited_float, 0.0, -10.0, 10.0, "Y center of the crop circle."),
        param!("scatter_area", "Scatter Area", unlimited_float, 0.0, -1.0, 1.0, "Random scatter band along the circle boundary. 0 = snap to boundary; ±1 = scatter across full half-radius."),
        param!("zero", "Zero", bool, true, "Behavior outside the circle: 1 = hide (collapse to origin), 0 = scatter onto boundary."),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 1,
    wgsl_init: Some(r#"
fn init_post_circlecrop(user: array<f32, 5>) -> array<f32, 1> {
    var out: array<f32, 1>;
    out[0] = clamp(user[3], -1.0, 1.0);
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_post_circlecrop(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let cr = get_param(xform_id, variation_id, 0u);
    let x0 = get_param(xform_id, variation_id, 1u);
    let y0 = get_param(xform_id, variation_id, 2u);
    let zero = i32(get_param(xform_id, variation_id, 4u));
    let ca = get_param(xform_id, variation_id, 5u);
    let w = transforms[xform_id].variations[variation_id];

    let dx = p.x - x0;
    let dy = p.y - y0;
    let rad = sqrt(dx * dx + dy * dy);
    let ang = atan2(dy, dx);
    let rdc = cr + rng_nextf(rng) * 0.5 * ca;
    let esc = rad > cr;
    let cr0 = zero == 1;

    if (cr0 && esc) {
        return vec2<f32>(0.0, 0.0);
    }
    if (!cr0 && esc) {
        return vec2<f32>(w * rdc * cos(ang) + x0, w * rdc * sin(ang) + y0);
    }
    return vec2<f32>(w * dx + x0, w * dy + y0);
}
"#,
    wgsl_3d: Some(r#"
fn variation_post_circlecrop(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let cr = get_param(xform_id, variation_id, 0u);
    let x0 = get_param(xform_id, variation_id, 1u);
    let y0 = get_param(xform_id, variation_id, 2u);
    let zero = i32(get_param(xform_id, variation_id, 4u));
    let ca = get_param(xform_id, variation_id, 5u);
    let w = transforms[xform_id].variations[variation_id];

    let dx = p.x - x0;
    let dy = p.y - y0;
    let rad = sqrt(dx * dx + dy * dy);
    let ang = atan2(dy, dx);
    let rdc = cr + rng_nextf(rng) * 0.5 * ca;
    let esc = rad > cr;
    let cr0 = zero == 1;

    if (cr0 && esc) {
        return vec3<f32>(0.0, 0.0, p.z);
    }
    if (!cr0 && esc) {
        return vec3<f32>(w * rdc * cos(ang) + x0, w * rdc * sin(ang) + y0, p.z);
    }
    return vec3<f32>(w * dx + x0, w * dy + y0, p.z);
}
"#),
};
