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
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// 3D extension of `popcorn2` — modulates XY by `0.5 · (coord + scale ·
/// sin(tan(c · other_coord)))` (the standard popcorn2 form, halved) and
/// adds a Z output combining `sin(tan(c)) · atan2(y, x)` with an optional
/// Z-dependent term scaled by `popcorn2_3D_z`. The accumulator-read path in
/// the cpp (`otherZ ≠ 0`) is unreachable in our model since `FPz` starts at
/// 0 each iteration, so we always take the `otherZ == 0` branch.
///
/// # Authors
/// - Larry Berlin
pub static POPCORN2_3D: VariationDef = VariationDef {
    name: "popcorn2_3D",
    aliases: &[],
    display_name: "Popcorn2 3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsTransform, Feature::AlwaysZ],
    parameters: &[
        param!("popcorn2_3D_x", "X", unlimited_float, 0.1, -10.0, 10.0, "X-axis sine amplitude (multiplies `sin(tan(c · y))` in the X output)."),
        param!("popcorn2_3D_y", "Y", unlimited_float, 0.1, -10.0, 10.0, "Y-axis sine amplitude (multiplies `sin(tan(c · x))` in the Y output)."),
        param!("popcorn2_3D_z", "Z", unlimited_float, 0.1, -10.0, 10.0, "Z-axis amplitude on the secondary `sin(tan(c)) · temp_tz` term."),
        param!("popcorn2_3D_c", "C", unlimited_float, 3.0, -10.0, 10.0, "Frequency multiplier inside `tan(c · coord)` for the XY modulators and `tan(c)` for the Z term."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_popcorn2_3D(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let xp = get_param(xform_id, variation_id, 0u);
    let yp = get_param(xform_id, variation_id, 1u);
    let cp = get_param(xform_id, variation_id, 3u);
    // Metal's fast tan goes Inf/NaN where its cos flushes to zero at a
    // pole, and at arguments past f32 resolution; see the 3D body.
    var tan_y = tan(cp * p.y);
    tan_y = select(0.0, tan_y, abs(tan_y) <= 1e32);
    var tan_x = tan(cp * p.x);
    tan_x = select(0.0, tan_x, abs(tan_x) <= 1e32);
    let nx = 0.5 * (p.x + xp * sin(tan_y));
    let ny = 0.5 * (p.y + yp * sin(tan_x));
    return vec2<f32>(nx, ny);
}
"#,
    wgsl_3d: r#"
fn variation_popcorn2_3D(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let xp = get_param(xform_id, variation_id, 0u);
    let yp = get_param(xform_id, variation_id, 1u);
    let zp = get_param(xform_id, variation_id, 2u);
    let cp = get_param(xform_id, variation_id, 3u);
    let w = transforms[xform_id].variations[variation_id];

    // Metal's fast tan goes Inf/NaN where its cos flushes to zero at a
    // pole, and at arguments past f32 resolution; sin(that) then poisons
    // the point where NVIDIA — whose f32 tan is bounded ~2e7 by argument
    // granularity — stays finite. Same guard as popcorn (c338d988):
    // Inf/NaN fails abs(t) <= 1e32 and falls to 0; every finite tan
    // passes through, so Windows is bit-identical.
    var tan_y = tan(cp * p.y);
    tan_y = select(0.0, tan_y, abs(tan_y) <= 1e32);
    var tan_x = tan(cp * p.x);
    tan_x = select(0.0, tan_x, abs(tan_x) <= 1e32);
    var tan_c = tan(cp);
    tan_c = select(0.0, tan_c, abs(tan_c) <= 1e32);
    let nx = 0.5 * (p.x + xp * sin(tan_y));
    let ny = 0.5 * (p.y + yp * sin(tan_x));

    let ratio = min(abs(w), 1.0);
    let sin_tan_c = sin(tan_c);
    // ff_atan2: the second use is UNCONDITIONAL, so a zero-pair input
    // NaN'd z on Metal (mixed-sign zeros; see utilities.wgsl) — measured
    // via the probe at every zero pair.
    let temp_tz = select(p.z, sin_tan_c * ff_atan2(p.y, p.x), abs(p.z) < 1e-30);
    let temp_pz_over_w = ratio * sin_tan_c * ff_atan2(p.y, p.x);
    let nz = temp_pz_over_w + ratio * zp * sin_tan_c * temp_tz;
    return vec3<f32>(nx, ny, nz);
}
"#,
};
