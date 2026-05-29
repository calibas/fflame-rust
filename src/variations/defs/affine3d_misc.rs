//! affine3D (Framelet)
//!
//! General 3D affine transform with separate yaw/pitch/roll rotations,
//! per-axis scales, optional shear, and translation. 15 user params:
//!   - translate: tX, tY, tZ
//!   - scale:     sX, sY, sZ
//!   - rotate:    rX, rY, rZ (degrees)
//!   - shear:     sXY, sXZ, sYX, sYZ, sZX, sZY
//!
//! 7 init slots — `_sinX, _cosX, _sinY, _cosY, _sinZ, _cosZ,
//! _hasShear`. Matches upstream cpp's `_sinX/cosX/sinY/cosY/sinZ/cosZ`
//! plus its `_hasShear` boolean. (Previously inlined per-iteration
//! because of the now-defunct 16-slot per-variation buffer ceiling;
//! the packed-variation-params layout removes that constraint.)
//!
//! Body factors cleanly through outer multiplier (cpp uses VVAR
//! consistently on every output term, including the translate
//! constants).
//!
//! Source: `output/jwildfire-vars/output/affine3d.cpp`.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// General 3D affine transform — applies translation, per-axis scaling,
/// yaw/pitch/roll rotation, and optional shear in a single combined
/// transform. The 15 parameters cover translate (3), scale (3), rotate (3
/// in degrees), and shear (6 cross-axis terms). The body automatically
/// detects whether shear is significant (`|sxy| + |sxz| + ... > ε`) and
/// skips the sheared path when all 6 shear params are below threshold.
///
/// # Authors
/// - Framelet
pub static AFFINE3D: VariationDef = VariationDef {
    name: "affine3D",
    aliases: &[],
    display_name: "Affine 3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("translateX", "Translate X", unlimited_float, 0.0, -10.0, 10.0, "X-axis translation (added to the final output)."),
        param!("translateY", "Translate Y", unlimited_float, 0.0, -10.0, 10.0, "Y-axis translation."),
        param!("translateZ", "Translate Z", unlimited_float, 0.0, -10.0, 10.0, "Z-axis translation (3D only)."),
        param!("scaleX", "Scale X", unlimited_float, 1.0, -10.0, 10.0, "X-axis scale (multiplies x before rotation)."),
        param!("scaleY", "Scale Y", unlimited_float, 1.0, -10.0, 10.0, "Y-axis scale."),
        param!("scaleZ", "Scale Z", unlimited_float, 1.0, -10.0, 10.0, "Z-axis scale."),
        param!("rotateX", "Rotate X", unlimited_float, 0.0, -360.0, 360.0, "Rotation around the X axis (pitch), in degrees."),
        param!("rotateY", "Rotate Y", unlimited_float, 0.0, -360.0, 360.0, "Rotation around the Y axis (yaw), in degrees."),
        param!("rotateZ", "Rotate Z", unlimited_float, 0.0, -360.0, 360.0, "Rotation around the Z axis (roll), in degrees."),
        param!("shearXY", "Shear XY", unlimited_float, 0.0, -10.0, 10.0, "Shear factor applied to Y in the X output."),
        param!("shearXZ", "Shear XZ", unlimited_float, 0.0, -10.0, 10.0, "Shear factor applied to Z in the X output."),
        param!("shearYX", "Shear YX", unlimited_float, 0.0, -10.0, 10.0, "Shear factor applied to X in the Y output."),
        param!("shearYZ", "Shear YZ", unlimited_float, 0.0, -10.0, 10.0, "Shear factor applied to Z in the Y output."),
        param!("shearZX", "Shear ZX", unlimited_float, 0.0, -10.0, 10.0, "Shear factor applied to X in the Z output."),
        param!("shearZY", "Shear ZY", unlimited_float, 0.0, -10.0, 10.0, "Shear factor applied to Y in the Z output."),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 7,
    wgsl_init: Some(r#"
fn init_affine3D(user: array<f32, 15>) -> array<f32, 7> {
    let d2r = 0.017453292519943295;
    let rx = user[6];
    let ry = user[7];
    let rz = user[8];
    let shxy = user[9];
    let shxz = user[10];
    let shyx = user[11];
    let shyz = user[12];
    let shzx = user[13];
    let shzy = user[14];
    let has_shear = abs(shxy) + abs(shxz) + abs(shyx) + abs(shyz) + abs(shzx) + abs(shzy) > 1e-6;
    var out: array<f32, 7>;
    out[0] = sin(rx * d2r);                       // _sinX
    out[1] = cos(rx * d2r);                       // _cosX
    out[2] = sin(ry * d2r);                       // _sinY
    out[3] = cos(ry * d2r);                       // _cosY
    out[4] = sin(rz * d2r);                       // _sinZ
    out[5] = cos(rz * d2r);                       // _cosZ
    out[6] = select(0.0, 1.0, has_shear);         // _hasShear (0/1)
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_affine3D(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let tx = get_param(xform_id, variation_id, 0u);
    let ty = get_param(xform_id, variation_id, 1u);
    let sx_p = get_param(xform_id, variation_id, 3u);
    let sy_p = get_param(xform_id, variation_id, 4u);
    let sz_p = get_param(xform_id, variation_id, 5u);
    let shxy = get_param(xform_id, variation_id, 9u);
    let shxz = get_param(xform_id, variation_id, 10u);
    let shyx = get_param(xform_id, variation_id, 11u);
    let shyz = get_param(xform_id, variation_id, 12u);
    let shzx = get_param(xform_id, variation_id, 13u);
    let shzy = get_param(xform_id, variation_id, 14u);
    let sinx = get_param(xform_id, variation_id, 15u);
    let cosx = get_param(xform_id, variation_id, 16u);
    let siny = get_param(xform_id, variation_id, 17u);
    let cosy = get_param(xform_id, variation_id, 18u);
    let sinz = get_param(xform_id, variation_id, 19u);
    let cosz = get_param(xform_id, variation_id, 20u);
    let has_shear = get_param(xform_id, variation_id, 21u) > 0.5;

    let x = p.x;
    let y = p.y;
    let z = 0.0;

    if (has_shear) {
        let mx = shxy * sy_p * y + shxz * sz_p * z + sx_p * x;
        let my = shyx * sx_p * x + shyz * sz_p * z + sy_p * y;
        let mz = shzx * sx_p * x + shzy * sy_p * y + sz_p * z;
        let nx = cosz * (cosy * mx + siny * (sinx * my + cosx * mz)) - sinz * (cosx * my - sinx * mz) + tx;
        let ny = sinz * (cosy * mx + siny * (sinx * my + cosx * mz)) + cosz * (cosx * my - sinx * mz) + ty;
        return vec2<f32>(nx, ny);
    }
    let nx = cosz * (cosy * sx_p * x + siny * (cosx * sz_p * z + sinx * sy_p * y)) - sinz * (cosx * sy_p * y - sinx * sz_p * z) + tx;
    let ny = sinz * (cosy * sx_p * x + siny * (cosx * sz_p * z + sinx * sy_p * y)) + cosz * (cosx * sy_p * y - sinx * sz_p * z) + ty;
    return vec2<f32>(nx, ny);
}
"#,
    wgsl_3d: Some(r#"
fn variation_affine3D(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let tx = get_param(xform_id, variation_id, 0u);
    let ty = get_param(xform_id, variation_id, 1u);
    let tz = get_param(xform_id, variation_id, 2u);
    let sx_p = get_param(xform_id, variation_id, 3u);
    let sy_p = get_param(xform_id, variation_id, 4u);
    let sz_p = get_param(xform_id, variation_id, 5u);
    let shxy = get_param(xform_id, variation_id, 9u);
    let shxz = get_param(xform_id, variation_id, 10u);
    let shyx = get_param(xform_id, variation_id, 11u);
    let shyz = get_param(xform_id, variation_id, 12u);
    let shzx = get_param(xform_id, variation_id, 13u);
    let shzy = get_param(xform_id, variation_id, 14u);
    let sinx = get_param(xform_id, variation_id, 15u);
    let cosx = get_param(xform_id, variation_id, 16u);
    let siny = get_param(xform_id, variation_id, 17u);
    let cosy = get_param(xform_id, variation_id, 18u);
    let sinz = get_param(xform_id, variation_id, 19u);
    let cosz = get_param(xform_id, variation_id, 20u);
    let has_shear = get_param(xform_id, variation_id, 21u) > 0.5;

    let x = p.x;
    let y = p.y;
    let z = p.z;

    if (has_shear) {
        let mx = shxy * sy_p * y + shxz * sz_p * z + sx_p * x;
        let my = shyx * sx_p * x + shyz * sz_p * z + sy_p * y;
        let mz = shzx * sx_p * x + shzy * sy_p * y + sz_p * z;
        let nx = cosz * (cosy * mx + siny * (sinx * my + cosx * mz)) - sinz * (cosx * my - sinx * mz) + tx;
        let ny = sinz * (cosy * mx + siny * (sinx * my + cosx * mz)) + cosz * (cosx * my - sinx * mz) + ty;
        let nz = -siny * mx + cosy * (sinx * my + cosx * mz) + tz;
        return vec3<f32>(nx, ny, nz);
    }
    let nx = cosz * (cosy * sx_p * x + siny * (cosx * sz_p * z + sinx * sy_p * y)) - sinz * (cosx * sy_p * y - sinx * sz_p * z) + tx;
    let ny = sinz * (cosy * sx_p * x + siny * (cosx * sz_p * z + sinx * sy_p * y)) + cosz * (cosx * sy_p * y - sinx * sz_p * z) + ty;
    let nz = -siny * sx_p * x + cosy * (cosx * sz_p * z + sinx * sy_p * y) + tz;
    return vec3<f32>(nx, ny, nz);
}
"#),
};
