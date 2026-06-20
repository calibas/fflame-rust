//! `circlecrop` (Xyrus02)
//!
//! Normal-phase circle-crop. Same algorithm as `pre_circlecrop` and
//! `post_circlecrop` (already ported), but in normal phase the body
//! uses `FPx +=` (accumulator) so we use `needs_transform` divide-out
//! to handle the `+ x0` / `+ y0` constant offsets that don't factor
//! through the outer multiplier.
//!
//!   - 5 user params: radius, x, y, scatter_area, zero (int 0/1)
//!   - 1 init slot: cA = clamp(scatter_area, -1, 1)
//!
//! The cpp's "doHide" path (when zero=1 and point is outside cr)
//! sets `FPx = FPy = 0` to flag the point hidden. Our system has no
//! doHide flag — we just return (0, 0) at that branch (which plots
//! at origin instead of hiding). This matches visually for typical
//! use (most points fall inside cr or zero=0).
//!
//! Source: `output/jwildfire-vars/output/circlecrop.cpp`.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Normal-phase circle-crop — tests whether the input lies inside a circle
/// of radius `radius` centered at `(x, y)`. Inside, the input passes
/// through scaled by weight. Outside, behavior depends on `zero`: if 1, the
/// point is hidden (collapsed to origin); if 0, it scatters onto the circle
/// boundary with `scatter_area` randomization.
///
/// # Authors
/// - Xyrus02
pub static CIRCLECROP: VariationDef = VariationDef {
    name: "circlecrop",
    aliases: &[],
    display_name: "Circle Crop",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::NeedsTransform, Feature::CanHide],
    parameters: &[
        param!("radius", "Radius", unlimited_float, 1.0, -10.0, 10.0, "Crop circle radius."),
        param!("x", "X", unlimited_float, 0.0, -10.0, 10.0, "X center of the crop circle."),
        param!("y", "Y", unlimited_float, 0.0, -10.0, 10.0, "Y center of the crop circle."),
        param!("scatter_area", "Scatter Area", unlimited_float, 0.0, -1.0, 1.0, "Random scatter band along the circle boundary. 0 = snap to boundary; ±1 = scatter across full half-radius."),
        param!("zero", "Zero", bool, true, "When on, points outside the circle collapse to the origin. When off, they scatter onto the boundary."),
    ],
    init_param_count: 1,
    wgsl_init: Some(r#"
fn init_circlecrop(user: array<f32, 5>) -> array<f32, 1> {
    var out: array<f32, 1>;
    out[0] = clamp(user[3], -1.0, 1.0);
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_circlecrop(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec2<f32> {
    let cr = get_param(xform_id, variation_id, 0u);
    let x0 = get_param(xform_id, variation_id, 1u);
    let y0 = get_param(xform_id, variation_id, 2u);
    let zero = i32(get_param(xform_id, variation_id, 4u));
    let ca = get_param(xform_id, variation_id, 5u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let dx = p.x - x0;
    let dy = p.y - y0;
    let rad = sqrt(dx * dx + dy * dy);
    let ang = atan2(dy, dx);
    let rdc = cr + rng_nextf(rng) * 0.5 * ca;
    let esc = rad > cr;
    let cr0 = zero == 1;

    if (cr0 && esc) {
        // cpp sets pVarTP.doHide — actually hide the point now.
        *hide = true;
        return vec2<f32>(0.0, 0.0);
    }
    if (!cr0 && esc) {
        // cpp: FPx += w · rdc · cos(ang) + x0  →  body / w returns this / w.
        return vec2<f32>((w * rdc * cos(ang) + x0) * inv_w, (w * rdc * sin(ang) + y0) * inv_w);
    }
    return vec2<f32>((w * dx + x0) * inv_w, (w * dy + y0) * inv_w);
}
"#,
    wgsl_3d: r#"
fn variation_circlecrop(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec3<f32> {
    let cr = get_param(xform_id, variation_id, 0u);
    let x0 = get_param(xform_id, variation_id, 1u);
    let y0 = get_param(xform_id, variation_id, 2u);
    let zero = i32(get_param(xform_id, variation_id, 4u));
    let ca = get_param(xform_id, variation_id, 5u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let dx = p.x - x0;
    let dy = p.y - y0;
    let rad = sqrt(dx * dx + dy * dy);
    let ang = atan2(dy, dx);
    let rdc = cr + rng_nextf(rng) * 0.5 * ca;
    let esc = rad > cr;
    let cr0 = zero == 1;

    if (cr0 && esc) {
        *hide = true;
        return vec3<f32>(0.0, 0.0, p.z);
    }
    if (!cr0 && esc) {
        return vec3<f32>((w * rdc * cos(ang) + x0) * inv_w, (w * rdc * sin(ang) + y0) * inv_w, p.z);
    }
    return vec3<f32>((w * dx + x0) * inv_w, (w * dy + y0) * inv_w, p.z);
}
"#,
};
