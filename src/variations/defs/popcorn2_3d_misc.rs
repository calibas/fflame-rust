//! popcorn2_3D (Larry Berlin, 2009)
//!
//! 3D mod of the 2D popcorn2 plugin. Modulates XY by half-amplitude
//! sin(tan(c·FT_other)) and Z by sin(tan(c))·(atan2 + popcorn_z·FTz)
//! with a `tmpVV = sgn(w)·w²` factor (capped at w when |w|>1).
//!
//! 4 user params (popcorn2_3D_x, popcorn2_3D_y, popcorn2_3D_z,
//! popcorn2_3D_c). No init slots.
//!
//! cpp's body reads `FPz` (otherZ) and `FTz` (inZ) and branches:
//!   - otherZ == 0: tempPZ = tmpVV·sin(tan(c))·atan2(FTy, FTx)
//!   - otherZ != 0: tempPZ = FPz  (ACCUMULATOR READ — blocked path)
//!   - inZ == 0:    tempTZ = tmpVV·sin(tan(c))·atan2(FTy, FTx)
//!   - inZ != 0:    tempTZ = FTz
//!
//! In our model `FPz` starts at 0 each iteration, so we always take
//! the `otherZ == 0` branch (compromise documented in glynnsshape's
//! "popcorn2_3d blocked due to accumulator reads" — recovered here
//! by picking the consistent default branch). For inZ we can read
//! `p.z` and branch correctly.
//!
//! Body has needs_transform divide-out: the cpp `tmpVV` depends on w
//! non-linearly. Using w/|w|≤1 → tmpVV/w = min(|w|, 1) absorbs the
//! complication.
//!
//! Source: `output/jwildfire-vars/output/popcorn2_3d.cpp`.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

pub static POPCORN2_3D: VariationDef = VariationDef {
    name: "popcorn2_3D",
    display_name: "Popcorn2 3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("popcorn2_3D_x", "X", unlimited_float, 0.1, -10.0, 10.0),
        param!("popcorn2_3D_y", "Y", unlimited_float, 0.1, -10.0, 10.0),
        param!("popcorn2_3D_z", "Z", unlimited_float, 0.1, -10.0, 10.0),
        param!("popcorn2_3D_c", "C", unlimited_float, 3.0, -10.0, 10.0),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_popcorn2_3D(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let xp = get_param(xform_id, variation_id, 0u);
    let yp = get_param(xform_id, variation_id, 1u);
    let cp = get_param(xform_id, variation_id, 3u);
    let nx = 0.5 * (p.x + xp * sin(tan(cp * p.y)));
    let ny = 0.5 * (p.y + yp * sin(tan(cp * p.x)));
    return vec2<f32>(nx, ny);
}
"#,
    wgsl_3d: Some(r#"
fn variation_popcorn2_3D(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let xp = get_param(xform_id, variation_id, 0u);
    let yp = get_param(xform_id, variation_id, 1u);
    let zp = get_param(xform_id, variation_id, 2u);
    let cp = get_param(xform_id, variation_id, 3u);
    let w = transforms[xform_id].variations[variation_id];

    let nx = 0.5 * (p.x + xp * sin(tan(cp * p.y)));
    let ny = 0.5 * (p.y + yp * sin(tan(cp * p.x)));

    let ratio = min(abs(w), 1.0);
    let sin_tan_c = sin(tan(cp));
    let temp_tz = select(p.z, sin_tan_c * atan2(p.y, p.x), abs(p.z) < 1e-30);
    let temp_pz_over_w = ratio * sin_tan_c * atan2(p.y, p.x);
    let nz = temp_pz_over_w + ratio * zp * sin_tan_c * temp_tz;
    return vec3<f32>(nx, ny, nz);
}
"#),
};
