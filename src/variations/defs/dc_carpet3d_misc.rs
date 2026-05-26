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

/// 3D Sierpinski-carpet IFS — picks a random corner of the unit square by
/// independent ±1 sign choices on X and Y (with per-axis offsets
/// `stretch_x`/`stretch_y`), then runs the result through the transform's
/// affine and scales by `scale_x`/`scale_y`. Originally a direct-color
/// variation with color-coupled Z output (`z = color · scale_z +
/// offset_z`); since we don't write color, the Z output collapses to a
/// constant `offset_z` bump and the color/scale_z/reset_z params are kept
/// only for interface parity with the C++/Java original.
///
/// # Authors
/// - Xyrus02
/// - Brad Stefanov
pub static DC_CARPET3D: VariationDef = VariationDef {
    name: "dc_carpet3D",
    display_name: "DC Carpet 3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("origin", "Origin", unlimited_float, 0.5, -10.0, 10.0, "Original `origin` parameter that controlled color in the C++/Java source. Unused in the Rust port since color writes are dropped — kept for interface parity."),
        param!("color_a", "Color A", unlimited_float, 0.5, -10.0, 10.0, "Color parameter (unused — color writes dropped). Kept for cpp/Java interface parity."),
        param!("color_b", "Color B", unlimited_float, 1.0, -10.0, 10.0, "Color parameter (unused — color writes dropped). Kept for cpp/Java interface parity."),
        param!("color_c", "Color C", unlimited_float, 1.0, -10.0, 10.0, "Color parameter (unused — color writes dropped). Kept for cpp/Java interface parity."),
        param!("color_d", "Color D", unlimited_float, 1.0, -10.0, 10.0, "Color parameter (unused — color writes dropped). Kept for cpp/Java interface parity."),
        param!("color_e", "Color E", unlimited_float, 0.5, -10.0, 10.0, "Color parameter (unused — color writes dropped). Kept for cpp/Java interface parity."),
        param!("color_f", "Color F", unlimited_float, 1.0, -10.0, 10.0, "Color parameter (unused — color writes dropped). Kept for cpp/Java interface parity."),
        param!("stretch_x", "Stretch X", unlimited_float, 1.0, -10.0, 10.0, "Magnitude of the ±1 X corner offset added to the input. Larger values pull the carpet's corners further apart along X."),
        param!("stretch_y", "Stretch Y", unlimited_float, 1.0, -10.0, 10.0, "Magnitude of the ±1 Y corner offset added to the input."),
        param!("scale_x", "Scale X", unlimited_float, 1.0, -10.0, 10.0, "Post-affine X scale applied to the carpet position."),
        param!("scale_y", "Scale Y", unlimited_float, 1.0, -10.0, 10.0, "Post-affine Y scale applied to the carpet position."),
        param!("scale_z", "Scale Z", unlimited_float, 1.0, -10.0, 10.0, "Color-to-Z coupling multiplier (unused — color writes dropped). Kept for cpp/Java interface parity."),
        param!("offset_z", "Offset Z", unlimited_float, 0.0, -10.0, 10.0, "Constant Z bump added to the output per iteration. Originally a fallback added to the color-derived Z; in this port it's the only contribution to Z."),
        param!("reset_z", "Reset Z", unlimited_float, 0.0, 0.0, 1.0, "Color-driven Z reset switch (unused — color writes dropped). Kept for cpp/Java interface parity."),
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
