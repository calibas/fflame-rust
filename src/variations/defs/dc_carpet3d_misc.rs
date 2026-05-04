//! dc_carpet3D (Xyrus02 / Stefanov)
//!
//! 3D carpet — random ±1 corner offsets per axis, mapped through the
//! transform's affine matrix. cpp couples color to z via `dz =
//! pVarTP.color * scale_z + offset_z` then `pVarTP.z = dz` (or +=).
//! Since we drop color writes, we drop the color-coupled z output too.
//!
//! 14 user params (origin, color_a/b/c/d/e/f, stretch_x/y, scale_x/y,
//! scale_z, offset_z, reset_z) — color params kept to match cpp
//! interface but unused in body.
//! 1 init slot (_H = 0.1 * origin) — unused since color is dropped.
//!
//! Body factors cleanly through outer multiplier (cpp uses pAmount on
//! the spatial output lines). Reads xform's affine via needs_transform
//! and applies it to the (x, y) carpet position.
//!
//! Compromises:
//!   - Color writes dropped (writes_color compromise)
//!   - Z output: `output.z = p.z + w * offset_z` (drop the
//!     color-coupled `dz = color * scale_z + offset_z`; emit
//!     just offset_z for a constant Z bump)
//!   - reset_z dropped (without color we can't compute the dz it would
//!     reset to)
//!
//! Source: `output/jwildfire-vars/output/dc_carpet3d.cpp`
//! (Java-recovered; cpp PluginVarCalc empty unported_stub).

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

pub static DC_CARPET3D: VariationDef = VariationDef {
    name: "dc_carpet3D",
    display_name: "DC Carpet 3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("origin", "Origin", unlimited_float, 0.5, -10.0, 10.0),
        param!("color_a", "Color A", unlimited_float, 0.5, -10.0, 10.0),
        param!("color_b", "Color B", unlimited_float, 1.0, -10.0, 10.0),
        param!("color_c", "Color C", unlimited_float, 1.0, -10.0, 10.0),
        param!("color_d", "Color D", unlimited_float, 1.0, -10.0, 10.0),
        param!("color_e", "Color E", unlimited_float, 0.5, -10.0, 10.0),
        param!("color_f", "Color F", unlimited_float, 1.0, -10.0, 10.0),
        param!("stretch_x", "Stretch X", unlimited_float, 1.0, -10.0, 10.0),
        param!("stretch_y", "Stretch Y", unlimited_float, 1.0, -10.0, 10.0),
        param!("scale_x", "Scale X", unlimited_float, 1.0, -10.0, 10.0),
        param!("scale_y", "Scale Y", unlimited_float, 1.0, -10.0, 10.0),
        param!("scale_z", "Scale Z", unlimited_float, 1.0, -10.0, 10.0),
        param!("offset_z", "Offset Z", unlimited_float, 0.0, -10.0, 10.0),
        param!("reset_z", "Reset Z", unlimited_float, 0.0, 0.0, 1.0),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_dc_carpet3D(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let stretch_x = get_param(xform_id, variation_id, 7u);
    let stretch_y = get_param(xform_id, variation_id, 8u);
    let scale_x = get_param(xform_id, variation_id, 9u);
    let scale_y = get_param(xform_id, variation_id, 10u);
    let xf = transforms[xform_id];

    let x0 = select(1.0, -1.0, rng_nextf(rng) < 0.5);
    let y0 = select(1.0, -1.0, rng_nextf(rng) > 0.5);
    let x = p.x + x0 * stretch_x;
    let y = p.y + y0 * stretch_y;

    let nx = (xf.a * x + xf.b * y + xf.e) * scale_x;
    let ny = (xf.c * x + xf.d * y + xf.f) * scale_y;
    return vec2<f32>(nx, ny);
}
"#,
    wgsl_3d: Some(r#"
fn variation_dc_carpet3D(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let stretch_x = get_param(xform_id, variation_id, 7u);
    let stretch_y = get_param(xform_id, variation_id, 8u);
    let scale_x = get_param(xform_id, variation_id, 9u);
    let scale_y = get_param(xform_id, variation_id, 10u);
    let offset_z = get_param(xform_id, variation_id, 12u);
    let xf = transforms[xform_id];
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let x0 = select(1.0, -1.0, rng_nextf(rng) < 0.5);
    let y0 = select(1.0, -1.0, rng_nextf(rng) > 0.5);
    let x = p.x + x0 * stretch_x;
    let y = p.y + y0 * stretch_y;

    let nx = (xf.a * x + xf.b * y + xf.e) * scale_x;
    let ny = (xf.c * x + xf.d * y + xf.f) * scale_y;
    // Z output compromise: drop color-coupled term, keep just offset_z
    let nz = p.z + offset_z * inv_w;
    return vec3<f32>(nx, ny, nz);
}
"#),
};
