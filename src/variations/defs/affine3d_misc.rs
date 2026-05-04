//! affine3D (Framelet)
//!
//! General 3D affine transform with separate yaw/pitch/roll rotations,
//! per-axis scales, optional shear, and translation. 15 user params:
//!   - translate: tX, tY, tZ
//!   - scale:     sX, sY, sZ
//!   - rotate:    rX, rY, rZ (degrees)
//!   - shear:     sXY, sXZ, sYX, sYZ, sZX, sZY
//!
//! No init slots — sin/cos of the rotation angles are computed inline
//! per iteration. cpp's `_sinX/cosX/sinY/cosY/sinZ/cosZ` precompute is
//! skipped because storing them would push us past the 16-slot per-
//! variation budget. The GPU compiler can hoist sin/cos out of the
//! body since the rotation params are constant per flame.
//!
//! cpp's separate "_hasShear" boolean (stored in init) is replaced by
//! an inline `|sXY| + |sXZ| + … > ε` test. The clean affine path runs
//! when shear is below threshold; the full sheared path runs otherwise.
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

pub static AFFINE3D: VariationDef = VariationDef {
    name: "affine3D",
    display_name: "Affine 3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("translateX", "Translate X", unlimited_float, 0.0, -10.0, 10.0),
        param!("translateY", "Translate Y", unlimited_float, 0.0, -10.0, 10.0),
        param!("translateZ", "Translate Z", unlimited_float, 0.0, -10.0, 10.0),
        param!("scaleX", "Scale X", unlimited_float, 1.0, -10.0, 10.0),
        param!("scaleY", "Scale Y", unlimited_float, 1.0, -10.0, 10.0),
        param!("scaleZ", "Scale Z", unlimited_float, 1.0, -10.0, 10.0),
        param!("rotateX", "Rotate X", unlimited_float, 0.0, -360.0, 360.0),
        param!("rotateY", "Rotate Y", unlimited_float, 0.0, -360.0, 360.0),
        param!("rotateZ", "Rotate Z", unlimited_float, 0.0, -360.0, 360.0),
        param!("shearXY", "Shear XY", unlimited_float, 0.0, -10.0, 10.0),
        param!("shearXZ", "Shear XZ", unlimited_float, 0.0, -10.0, 10.0),
        param!("shearYX", "Shear YX", unlimited_float, 0.0, -10.0, 10.0),
        param!("shearYZ", "Shear YZ", unlimited_float, 0.0, -10.0, 10.0),
        param!("shearZX", "Shear ZX", unlimited_float, 0.0, -10.0, 10.0),
        param!("shearZY", "Shear ZY", unlimited_float, 0.0, -10.0, 10.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_affine3D(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let tx = get_param(xform_id, variation_id, 0u);
    let ty = get_param(xform_id, variation_id, 1u);
    let sx_p = get_param(xform_id, variation_id, 3u);
    let sy_p = get_param(xform_id, variation_id, 4u);
    let sz_p = get_param(xform_id, variation_id, 5u);
    let rx = get_param(xform_id, variation_id, 6u);
    let ry = get_param(xform_id, variation_id, 7u);
    let rz = get_param(xform_id, variation_id, 8u);
    let shxy = get_param(xform_id, variation_id, 9u);
    let shxz = get_param(xform_id, variation_id, 10u);
    let shyx = get_param(xform_id, variation_id, 11u);
    let shyz = get_param(xform_id, variation_id, 12u);
    let shzx = get_param(xform_id, variation_id, 13u);
    let shzy = get_param(xform_id, variation_id, 14u);

    let d2r = 0.017453292519943295;
    let sinx = sin(rx * d2r);
    let cosx = cos(rx * d2r);
    let siny = sin(ry * d2r);
    let cosy = cos(ry * d2r);
    let sinz = sin(rz * d2r);
    let cosz = cos(rz * d2r);

    let has_shear = abs(shxy) + abs(shxz) + abs(shyx) + abs(shyz) + abs(shzx) + abs(shzy) > 1e-6;

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
    let rx = get_param(xform_id, variation_id, 6u);
    let ry = get_param(xform_id, variation_id, 7u);
    let rz = get_param(xform_id, variation_id, 8u);
    let shxy = get_param(xform_id, variation_id, 9u);
    let shxz = get_param(xform_id, variation_id, 10u);
    let shyx = get_param(xform_id, variation_id, 11u);
    let shyz = get_param(xform_id, variation_id, 12u);
    let shzx = get_param(xform_id, variation_id, 13u);
    let shzy = get_param(xform_id, variation_id, 14u);

    let d2r = 0.017453292519943295;
    let sinx = sin(rx * d2r);
    let cosx = cos(rx * d2r);
    let siny = sin(ry * d2r);
    let cosy = cos(ry * d2r);
    let sinz = sin(rz * d2r);
    let cosz = cos(rz * d2r);

    let has_shear = abs(shxy) + abs(shxz) + abs(shyx) + abs(shyz) + abs(shzx) + abs(shzy) > 1e-6;

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
