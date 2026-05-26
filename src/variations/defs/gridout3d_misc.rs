//! gridout3D (Faber + Faber 2007-2008, ported by Dark-Beam 2017,
//! variables added by Brad Stefanov)
//!
//! Eight-region grid offset based on the sign of `rint(x)·xx` and
//! `rint(y)·yy` and the comparison between |x| and |y|. Each region
//! adds (or subtracts, or multiplies for some Z cases) a per-region
//! constant triplet to (x, y, z).
//!
//! 26 user params:
//!   - xx, yy: rounding gates for x/y sign comparison
//!   - xa..xh, ya..yh, za..zh: per-region offsets (8 regions × 3 axes)
//!
//! No init slots. Body factors cleanly through outer multiplier.
//! 3D variation (full3D) since it modifies z.
//!
//! Quirks preserved from cpp body:
//!   - region D (y<=0, x<=0, y>x): y output uses `- yd` not `+ yd`
//!   - regions E/F/G/H (y>0): z output uses `* ze..*zh` (multiply)
//!     instead of the `+ za..+zd` used by y<=0 regions
//!   - region E: x output uses `- xe`; region G: x uses `- xg`;
//!     region H: y uses `- yh`
//!
//! 26 user params places this over the old 16-slot ceiling — port
//! enabled by the packed-variation-params buffer (this branch).
//!
//! Source: `output/jwildfire-vars/output/gridout3d.cpp`
//! (Java-recovered; cpp PluginVarCalc empty unported_stub).

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Eight-region grid offset — partitions the XY plane into 8 wedges based
/// on `round(p.x)·xx` and `round(p.y)·yy` (the rounding turns the param
/// space into a coarse grid), then adds or subtracts a per-region offset
/// triplet to `(x, y, z)`. The 8 regions split each quadrant along its
/// diagonal; for `y ≤ 0` (regions A-D) the Z offset is added, for `y > 0`
/// (regions E-H) the Z offset is multiplied into Z instead.
///
/// Region table (input X/Y signs after gate-rounding by `xx`/`yy`):
///
/// | Region | Condition (`y`, `x` are gate-rounded) | X op | Y op | Z op |
/// |---|---|---|---|---|
/// | A | `y ≤ 0`, `x > 0`, `-y ≥ x`  | `+xa` | `+ya` | `+za` |
/// | B | `y ≤ 0`, `x > 0`, `-y < x`  | `+xb` | `+yb` | `+zb` |
/// | C | `y ≤ 0`, `x ≤ 0`, `y ≤ x`   | `+xc` | `+yc` | `+zc` |
/// | D | `y ≤ 0`, `x ≤ 0`, `y > x`   | `+xd` | `-yd` | `+zd` |
/// | E | `y > 0`, `x > 0`, `y ≥ x`   | `-xe` | `+ye` | `·ze` |
/// | F | `y > 0`, `x > 0`, `y < x`   | `+xf` | `+yf` | `·zf` |
/// | G | `y > 0`, `x ≤ 0`, `y ≥ -x`  | `-xg` | `+yg` | `·zg` |
/// | H | `y > 0`, `x ≤ 0`, `y < -x`  | `+xh` | `-yh` | `·zh` |
///
/// Sign flips on `-yd`, `-xe`, `-xg`, `-yh` and the add-vs-multiply Z split
/// between A-D and E-H are preserved verbatim from upstream.
///
/// # Authors
/// - Michael Faber
/// - Joel Faber
/// - DarkBeam
/// - Brad Stefanov
pub static GRIDOUT_3D: VariationDef = VariationDef {
    name: "gridout3D",
    display_name: "Gridout 3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("xx", "X Gate", unlimited_float, 1.0, -10.0, 10.0, "X gate scale: input `x` is rounded then multiplied by `xx` before the region comparison. Setting to 0 collapses all regions onto `x = 0` (only the y-half decision still applies)."),
        param!("yy", "Y Gate", unlimited_float, 1.0, -10.0, 10.0, "Y gate scale: input `y` is rounded then multiplied by `yy` before the region comparison. Setting to 0 collapses all regions onto `y = 0` (only the x-sign decision still applies)."),
        param!("xa", "XA", unlimited_float, 1.0, -10.0, 10.0, "Region A (y ≤ 0, x > 0, -y ≥ x) X offset, added to X."),
        param!("xb", "XB", unlimited_float, 0.0, -10.0, 10.0, "Region B (y ≤ 0, x > 0, -y < x) X offset, added to X."),
        param!("xc", "XC", unlimited_float, 1.0, -10.0, 10.0, "Region C (y ≤ 0, x ≤ 0, y ≤ x) X offset, added to X."),
        param!("xd", "XD", unlimited_float, 0.0, -10.0, 10.0, "Region D (y ≤ 0, x ≤ 0, y > x) X offset, added to X."),
        param!("xe", "XE", unlimited_float, 1.0, -10.0, 10.0, "Region E (y > 0, x > 0, y ≥ x) X offset, subtracted from X."),
        param!("xf", "XF", unlimited_float, 0.0, -10.0, 10.0, "Region F (y > 0, x > 0, y < x) X offset, added to X."),
        param!("xg", "XG", unlimited_float, 1.0, -10.0, 10.0, "Region G (y > 0, x ≤ 0, y ≥ -x) X offset, subtracted from X."),
        param!("xh", "XH", unlimited_float, 0.0, -10.0, 10.0, "Region H (y > 0, x ≤ 0, y < -x) X offset, added to X."),
        param!("ya", "YA", unlimited_float, 0.0, -10.0, 10.0, "Region A (y ≤ 0, x > 0, -y ≥ x) Y offset, added to Y."),
        param!("yb", "YB", unlimited_float, 1.0, -10.0, 10.0, "Region B (y ≤ 0, x > 0, -y < x) Y offset, added to Y."),
        param!("yc", "YC", unlimited_float, 0.0, -10.0, 10.0, "Region C (y ≤ 0, x ≤ 0, y ≤ x) Y offset, added to Y."),
        param!("yd", "YD", unlimited_float, 1.0, -10.0, 10.0, "Region D (y ≤ 0, x ≤ 0, y > x) Y offset, subtracted from Y."),
        param!("ye", "YE", unlimited_float, 0.0, -10.0, 10.0, "Region E (y > 0, x > 0, y ≥ x) Y offset, added to Y."),
        param!("yf", "YF", unlimited_float, 1.0, -10.0, 10.0, "Region F (y > 0, x > 0, y < x) Y offset, added to Y."),
        param!("yg", "YG", unlimited_float, 0.0, -10.0, 10.0, "Region G (y > 0, x ≤ 0, y ≥ -x) Y offset, added to Y."),
        param!("yh", "YH", unlimited_float, 1.0, -10.0, 10.0, "Region H (y > 0, x ≤ 0, y < -x) Y offset, subtracted from Y."),
        param!("za", "ZA", unlimited_float, 0.0, -10.0, 10.0, "Region A (y ≤ 0, x > 0, -y ≥ x) Z offset, added to Z."),
        param!("zb", "ZB", unlimited_float, 0.0, -10.0, 10.0, "Region B (y ≤ 0, x > 0, -y < x) Z offset, added to Z."),
        param!("zc", "ZC", unlimited_float, 0.0, -10.0, 10.0, "Region C (y ≤ 0, x ≤ 0, y ≤ x) Z offset, added to Z."),
        param!("zd", "ZD", unlimited_float, 0.0, -10.0, 10.0, "Region D (y ≤ 0, x ≤ 0, y > x) Z offset, added to Z."),
        param!("ze", "ZE", unlimited_float, 1.0, -10.0, 10.0, "Region E (y > 0, x > 0, y ≥ x) Z offset, multiplied into Z (regions E-H multiply; A-D add)."),
        param!("zf", "ZF", unlimited_float, 1.0, -10.0, 10.0, "Region F (y > 0, x > 0, y < x) Z offset, multiplied into Z (regions E-H multiply; A-D add)."),
        param!("zg", "ZG", unlimited_float, 1.0, -10.0, 10.0, "Region G (y > 0, x ≤ 0, y ≥ -x) Z offset, multiplied into Z (regions E-H multiply; A-D add)."),
        param!("zh", "ZH", unlimited_float, 1.0, -10.0, 10.0, "Region H (y > 0, x ≤ 0, y < -x) Z offset, multiplied into Z (regions E-H multiply; A-D add)."),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_gridout3D(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let xx = get_param(xform_id, variation_id, 0u);
    let yy = get_param(xform_id, variation_id, 1u);
    let xa = get_param(xform_id, variation_id, 2u);
    let xb = get_param(xform_id, variation_id, 3u);
    let xc = get_param(xform_id, variation_id, 4u);
    let xd = get_param(xform_id, variation_id, 5u);
    let xe = get_param(xform_id, variation_id, 6u);
    let xf = get_param(xform_id, variation_id, 7u);
    let xg = get_param(xform_id, variation_id, 8u);
    let xh = get_param(xform_id, variation_id, 9u);
    let ya = get_param(xform_id, variation_id, 10u);
    let yb = get_param(xform_id, variation_id, 11u);
    let yc = get_param(xform_id, variation_id, 12u);
    let yd = get_param(xform_id, variation_id, 13u);
    let ye = get_param(xform_id, variation_id, 14u);
    let yf = get_param(xform_id, variation_id, 15u);
    let yg = get_param(xform_id, variation_id, 16u);
    let yh = get_param(xform_id, variation_id, 17u);
    // 2D: emit just x/y; the 8-region z handling lives in the 3D body
    let x = round(p.x) * xx;
    let y = round(p.y) * yy;
    if (y <= 0.0) {
        if (x > 0.0) {
            if (-y >= x) { return vec2<f32>(p.x + xa, p.y + ya); }
            return vec2<f32>(p.x + xb, p.y + yb);
        }
        if (y <= x) { return vec2<f32>(p.x + xc, p.y + yc); }
        return vec2<f32>(p.x + xd, p.y - yd);
    }
    if (x > 0.0) {
        if (y >= x) { return vec2<f32>(p.x - xe, p.y + ye); }
        return vec2<f32>(p.x + xf, p.y + yf);
    }
    if (y >= -x) { return vec2<f32>(p.x - xg, p.y + yg); }
    return vec2<f32>(p.x + xh, p.y - yh);
}
"#,
    wgsl_3d: Some(r#"
fn variation_gridout3D(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let xx = get_param(xform_id, variation_id, 0u);
    let yy = get_param(xform_id, variation_id, 1u);
    let xa = get_param(xform_id, variation_id, 2u);
    let xb = get_param(xform_id, variation_id, 3u);
    let xc = get_param(xform_id, variation_id, 4u);
    let xd = get_param(xform_id, variation_id, 5u);
    let xe = get_param(xform_id, variation_id, 6u);
    let xf = get_param(xform_id, variation_id, 7u);
    let xg = get_param(xform_id, variation_id, 8u);
    let xh = get_param(xform_id, variation_id, 9u);
    let ya = get_param(xform_id, variation_id, 10u);
    let yb = get_param(xform_id, variation_id, 11u);
    let yc = get_param(xform_id, variation_id, 12u);
    let yd = get_param(xform_id, variation_id, 13u);
    let ye = get_param(xform_id, variation_id, 14u);
    let yf = get_param(xform_id, variation_id, 15u);
    let yg = get_param(xform_id, variation_id, 16u);
    let yh = get_param(xform_id, variation_id, 17u);
    let za = get_param(xform_id, variation_id, 18u);
    let zb = get_param(xform_id, variation_id, 19u);
    let zc = get_param(xform_id, variation_id, 20u);
    let zd = get_param(xform_id, variation_id, 21u);
    let ze = get_param(xform_id, variation_id, 22u);
    let zf = get_param(xform_id, variation_id, 23u);
    let zg = get_param(xform_id, variation_id, 24u);
    let zh = get_param(xform_id, variation_id, 25u);

    let x = round(p.x) * xx;
    let y = round(p.y) * yy;
    if (y <= 0.0) {
        if (x > 0.0) {
            if (-y >= x) { return vec3<f32>(p.x + xa, p.y + ya, p.z + za); }
            return vec3<f32>(p.x + xb, p.y + yb, p.z + zb);
        }
        if (y <= x) { return vec3<f32>(p.x + xc, p.y + yc, p.z + zc); }
        return vec3<f32>(p.x + xd, p.y - yd, p.z + zd);
    }
    if (x > 0.0) {
        if (y >= x) { return vec3<f32>(p.x - xe, p.y + ye, p.z * ze); }
        return vec3<f32>(p.x + xf, p.y + yf, p.z * zf);
    }
    if (y >= -x) { return vec3<f32>(p.x - xg, p.y + yg, p.z * zg); }
    return vec3<f32>(p.x + xh, p.y - yh, p.z * zh);
}
"#),
};
