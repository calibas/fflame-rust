//! `matrix3D` — a full 3D affine from a raw matrix (original).
//!
//! `x' = M·x + t` with all nine matrix entries and three offsets as
//! plain parameters. Exists because the transform's own 3D affine
//! machinery (`yz_coefs`/`zx_coefs`) mirrors JWildfire's sequential
//! per-plane composition with decoupled offsets — deliberately NOT a
//! general matrix, and awkward to decompose an arbitrary rotation into.
//! (The existing `affine3D` is JWildfire's translate/rotate/scale/shear
//! parameterization — good for hand-editing, wrong container for an
//! exact measured matrix.) Scripts that build 3D IFS pieces (the L-system work) need exact
//! arbitrary similarities, and this is the honest container: the same
//! role `mobius` plays for the group decompositions.
//!
//! AlwaysZ: z is written unconditionally — a 3D affine that lost its z
//! under `preserve_z = false` would silently flatten the structure it
//! exists to build (the decomposition lesson).

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Full 3D affine `x' = M·x + t`, all twelve coefficients as
/// parameters. The container for script-built 3D IFS pieces.
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5
pub static MATRIX3D: VariationDef = VariationDef {
    name: "matrix3D",
    aliases: &[],
    display_name: "Matrix 3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    features: &[Feature::AlwaysZ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    parameters: &[
        param!("xx", "XX", unlimited_float, 1.0, -4.0, 4.0, "Matrix entry: input x to output x."),
        param!("xy", "XY", unlimited_float, 0.0, -4.0, 4.0, "Matrix entry: input y to output x."),
        param!("xz", "XZ", unlimited_float, 0.0, -4.0, 4.0, "Matrix entry: input z to output x."),
        param!("yx", "YX", unlimited_float, 0.0, -4.0, 4.0, "Matrix entry: input x to output y."),
        param!("yy", "YY", unlimited_float, 1.0, -4.0, 4.0, "Matrix entry: input y to output y."),
        param!("yz", "YZ", unlimited_float, 0.0, -4.0, 4.0, "Matrix entry: input z to output y."),
        param!("zx", "ZX", unlimited_float, 0.0, -4.0, 4.0, "Matrix entry: input x to output z."),
        param!("zy", "ZY", unlimited_float, 0.0, -4.0, 4.0, "Matrix entry: input y to output z."),
        param!("zz", "ZZ", unlimited_float, 1.0, -4.0, 4.0, "Matrix entry: input z to output z."),
        param!("tx", "TX", unlimited_float, 0.0, -4.0, 4.0, "Translation x."),
        param!("ty", "TY", unlimited_float, 0.0, -4.0, 4.0, "Translation y."),
        param!("tz", "TZ", unlimited_float, 0.0, -4.0, 4.0, "Translation z."),
    ],
    wgsl_2d: r#"
fn variation_matrix3D(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    // 2D reading: the same affine applied at z = 0, xy of the result.
    let xx = get_param(xform_id, variation_id, 0u);
    let xy = get_param(xform_id, variation_id, 1u);
    let yx = get_param(xform_id, variation_id, 3u);
    let yy = get_param(xform_id, variation_id, 4u);
    let tx = get_param(xform_id, variation_id, 9u);
    let ty = get_param(xform_id, variation_id, 10u);
    return vec2<f32>(xx * p.x + xy * p.y + tx, yx * p.x + yy * p.y + ty);
}
"#,
    wgsl_3d: r#"
fn variation_matrix3D(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let xx = get_param(xform_id, variation_id, 0u);
    let xy = get_param(xform_id, variation_id, 1u);
    let xz = get_param(xform_id, variation_id, 2u);
    let yx = get_param(xform_id, variation_id, 3u);
    let yy = get_param(xform_id, variation_id, 4u);
    let yz = get_param(xform_id, variation_id, 5u);
    let zx = get_param(xform_id, variation_id, 6u);
    let zy = get_param(xform_id, variation_id, 7u);
    let zz = get_param(xform_id, variation_id, 8u);
    let tx = get_param(xform_id, variation_id, 9u);
    let ty = get_param(xform_id, variation_id, 10u);
    let tz = get_param(xform_id, variation_id, 11u);
    return vec3<f32>(
        xx * p.x + xy * p.y + xz * p.z + tx,
        yx * p.x + yy * p.y + yz * p.z + ty,
        zx * p.x + zy * p.y + zz * p.z + tz,
    );
}
"#,
};
