//! `lsystem_path_3D` — finite-depth 3D IFS path sampler (original).
//!
//! The 3D sibling of `lsystem_path`: draws the depth-k polyline of an
//! IFS whose maps trace a curve in visiting order, with full 3D affines
//! per map (twelve coefficients — the 2D version's six cannot carry
//! pitch or roll). No geometry is stored: vertex i is the composition
//! of k affines selected by the base-n digits of i, so `iterations`
//! stays a live parameter. The curve parameter t doubles as direct
//! color.
//!
//! AlwaysZ: the z it computes is the point — flattening it would erase
//! the third dimension the variation exists for. Flames using it should
//! also set `preserve_z` so z survives between iterations.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Finite-depth 3D IFS path: up to 12 full 3D affine maps composed by
/// the digits of the curve parameter. Written by the L-System scripts.
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5
pub static LSYSTEM_PATH_3D: VariationDef = VariationDef {
    name: "lsystem_path_3D",
    aliases: &[],
    display_name: "L-System Path 3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::WritesColor, Feature::AlwaysZ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    parameters: &[
        param!("iterations", "Iterations", int, 5.0, 1.0, 12.0, "L-system depth: the path has map_count^iterations segments. A live parameter."),
        param!("map_count", "Map Count", int, 4.0, 2.0, 12.0, "How many of the twelve map slots are in use. Set by the L-System script."),
        param!("connect", "Connect", bool, true, "Draw the connecting segments between consecutive vertices. Off plots only the vertices."),
        param!("dc", "Direct Color", bool, true, "Color by the curve parameter t (needs the transform's Direct Color at 1)."),
        param!("m0_xx", "M0 XX", unlimited_float, 1.0, -4.0, 4.0, "Map 0 affine coefficient xx. Normally written by the L-System script."),
        param!("m0_xy", "M0 XY", unlimited_float, 0.0, -4.0, 4.0, "Map 0 affine coefficient xy. Normally written by the L-System script."),
        param!("m0_xz", "M0 XZ", unlimited_float, 0.0, -4.0, 4.0, "Map 0 affine coefficient xz. Normally written by the L-System script."),
        param!("m0_yx", "M0 YX", unlimited_float, 0.0, -4.0, 4.0, "Map 0 affine coefficient yx. Normally written by the L-System script."),
        param!("m0_yy", "M0 YY", unlimited_float, 1.0, -4.0, 4.0, "Map 0 affine coefficient yy. Normally written by the L-System script."),
        param!("m0_yz", "M0 YZ", unlimited_float, 0.0, -4.0, 4.0, "Map 0 affine coefficient yz. Normally written by the L-System script."),
        param!("m0_zx", "M0 ZX", unlimited_float, 0.0, -4.0, 4.0, "Map 0 affine coefficient zx. Normally written by the L-System script."),
        param!("m0_zy", "M0 ZY", unlimited_float, 0.0, -4.0, 4.0, "Map 0 affine coefficient zy. Normally written by the L-System script."),
        param!("m0_zz", "M0 ZZ", unlimited_float, 1.0, -4.0, 4.0, "Map 0 affine coefficient zz. Normally written by the L-System script."),
        param!("m0_tx", "M0 TX", unlimited_float, 0.0, -4.0, 4.0, "Map 0 affine coefficient tx. Normally written by the L-System script."),
        param!("m0_ty", "M0 TY", unlimited_float, 0.0, -4.0, 4.0, "Map 0 affine coefficient ty. Normally written by the L-System script."),
        param!("m0_tz", "M0 TZ", unlimited_float, 0.0, -4.0, 4.0, "Map 0 affine coefficient tz. Normally written by the L-System script."),
        param!("m1_xx", "M1 XX", unlimited_float, 0.0, -4.0, 4.0, "Map 1 affine coefficient xx. Normally written by the L-System script."),
        param!("m1_xy", "M1 XY", unlimited_float, 0.0, -4.0, 4.0, "Map 1 affine coefficient xy. Normally written by the L-System script."),
        param!("m1_xz", "M1 XZ", unlimited_float, 0.0, -4.0, 4.0, "Map 1 affine coefficient xz. Normally written by the L-System script."),
        param!("m1_yx", "M1 YX", unlimited_float, 0.0, -4.0, 4.0, "Map 1 affine coefficient yx. Normally written by the L-System script."),
        param!("m1_yy", "M1 YY", unlimited_float, 0.0, -4.0, 4.0, "Map 1 affine coefficient yy. Normally written by the L-System script."),
        param!("m1_yz", "M1 YZ", unlimited_float, 0.0, -4.0, 4.0, "Map 1 affine coefficient yz. Normally written by the L-System script."),
        param!("m1_zx", "M1 ZX", unlimited_float, 0.0, -4.0, 4.0, "Map 1 affine coefficient zx. Normally written by the L-System script."),
        param!("m1_zy", "M1 ZY", unlimited_float, 0.0, -4.0, 4.0, "Map 1 affine coefficient zy. Normally written by the L-System script."),
        param!("m1_zz", "M1 ZZ", unlimited_float, 0.0, -4.0, 4.0, "Map 1 affine coefficient zz. Normally written by the L-System script."),
        param!("m1_tx", "M1 TX", unlimited_float, 0.0, -4.0, 4.0, "Map 1 affine coefficient tx. Normally written by the L-System script."),
        param!("m1_ty", "M1 TY", unlimited_float, 0.0, -4.0, 4.0, "Map 1 affine coefficient ty. Normally written by the L-System script."),
        param!("m1_tz", "M1 TZ", unlimited_float, 0.0, -4.0, 4.0, "Map 1 affine coefficient tz. Normally written by the L-System script."),
        param!("m2_xx", "M2 XX", unlimited_float, 0.0, -4.0, 4.0, "Map 2 affine coefficient xx. Normally written by the L-System script."),
        param!("m2_xy", "M2 XY", unlimited_float, 0.0, -4.0, 4.0, "Map 2 affine coefficient xy. Normally written by the L-System script."),
        param!("m2_xz", "M2 XZ", unlimited_float, 0.0, -4.0, 4.0, "Map 2 affine coefficient xz. Normally written by the L-System script."),
        param!("m2_yx", "M2 YX", unlimited_float, 0.0, -4.0, 4.0, "Map 2 affine coefficient yx. Normally written by the L-System script."),
        param!("m2_yy", "M2 YY", unlimited_float, 0.0, -4.0, 4.0, "Map 2 affine coefficient yy. Normally written by the L-System script."),
        param!("m2_yz", "M2 YZ", unlimited_float, 0.0, -4.0, 4.0, "Map 2 affine coefficient yz. Normally written by the L-System script."),
        param!("m2_zx", "M2 ZX", unlimited_float, 0.0, -4.0, 4.0, "Map 2 affine coefficient zx. Normally written by the L-System script."),
        param!("m2_zy", "M2 ZY", unlimited_float, 0.0, -4.0, 4.0, "Map 2 affine coefficient zy. Normally written by the L-System script."),
        param!("m2_zz", "M2 ZZ", unlimited_float, 0.0, -4.0, 4.0, "Map 2 affine coefficient zz. Normally written by the L-System script."),
        param!("m2_tx", "M2 TX", unlimited_float, 0.0, -4.0, 4.0, "Map 2 affine coefficient tx. Normally written by the L-System script."),
        param!("m2_ty", "M2 TY", unlimited_float, 0.0, -4.0, 4.0, "Map 2 affine coefficient ty. Normally written by the L-System script."),
        param!("m2_tz", "M2 TZ", unlimited_float, 0.0, -4.0, 4.0, "Map 2 affine coefficient tz. Normally written by the L-System script."),
        param!("m3_xx", "M3 XX", unlimited_float, 0.0, -4.0, 4.0, "Map 3 affine coefficient xx. Normally written by the L-System script."),
        param!("m3_xy", "M3 XY", unlimited_float, 0.0, -4.0, 4.0, "Map 3 affine coefficient xy. Normally written by the L-System script."),
        param!("m3_xz", "M3 XZ", unlimited_float, 0.0, -4.0, 4.0, "Map 3 affine coefficient xz. Normally written by the L-System script."),
        param!("m3_yx", "M3 YX", unlimited_float, 0.0, -4.0, 4.0, "Map 3 affine coefficient yx. Normally written by the L-System script."),
        param!("m3_yy", "M3 YY", unlimited_float, 0.0, -4.0, 4.0, "Map 3 affine coefficient yy. Normally written by the L-System script."),
        param!("m3_yz", "M3 YZ", unlimited_float, 0.0, -4.0, 4.0, "Map 3 affine coefficient yz. Normally written by the L-System script."),
        param!("m3_zx", "M3 ZX", unlimited_float, 0.0, -4.0, 4.0, "Map 3 affine coefficient zx. Normally written by the L-System script."),
        param!("m3_zy", "M3 ZY", unlimited_float, 0.0, -4.0, 4.0, "Map 3 affine coefficient zy. Normally written by the L-System script."),
        param!("m3_zz", "M3 ZZ", unlimited_float, 0.0, -4.0, 4.0, "Map 3 affine coefficient zz. Normally written by the L-System script."),
        param!("m3_tx", "M3 TX", unlimited_float, 0.0, -4.0, 4.0, "Map 3 affine coefficient tx. Normally written by the L-System script."),
        param!("m3_ty", "M3 TY", unlimited_float, 0.0, -4.0, 4.0, "Map 3 affine coefficient ty. Normally written by the L-System script."),
        param!("m3_tz", "M3 TZ", unlimited_float, 0.0, -4.0, 4.0, "Map 3 affine coefficient tz. Normally written by the L-System script."),
        param!("m4_xx", "M4 XX", unlimited_float, 0.0, -4.0, 4.0, "Map 4 affine coefficient xx. Normally written by the L-System script."),
        param!("m4_xy", "M4 XY", unlimited_float, 0.0, -4.0, 4.0, "Map 4 affine coefficient xy. Normally written by the L-System script."),
        param!("m4_xz", "M4 XZ", unlimited_float, 0.0, -4.0, 4.0, "Map 4 affine coefficient xz. Normally written by the L-System script."),
        param!("m4_yx", "M4 YX", unlimited_float, 0.0, -4.0, 4.0, "Map 4 affine coefficient yx. Normally written by the L-System script."),
        param!("m4_yy", "M4 YY", unlimited_float, 0.0, -4.0, 4.0, "Map 4 affine coefficient yy. Normally written by the L-System script."),
        param!("m4_yz", "M4 YZ", unlimited_float, 0.0, -4.0, 4.0, "Map 4 affine coefficient yz. Normally written by the L-System script."),
        param!("m4_zx", "M4 ZX", unlimited_float, 0.0, -4.0, 4.0, "Map 4 affine coefficient zx. Normally written by the L-System script."),
        param!("m4_zy", "M4 ZY", unlimited_float, 0.0, -4.0, 4.0, "Map 4 affine coefficient zy. Normally written by the L-System script."),
        param!("m4_zz", "M4 ZZ", unlimited_float, 0.0, -4.0, 4.0, "Map 4 affine coefficient zz. Normally written by the L-System script."),
        param!("m4_tx", "M4 TX", unlimited_float, 0.0, -4.0, 4.0, "Map 4 affine coefficient tx. Normally written by the L-System script."),
        param!("m4_ty", "M4 TY", unlimited_float, 0.0, -4.0, 4.0, "Map 4 affine coefficient ty. Normally written by the L-System script."),
        param!("m4_tz", "M4 TZ", unlimited_float, 0.0, -4.0, 4.0, "Map 4 affine coefficient tz. Normally written by the L-System script."),
        param!("m5_xx", "M5 XX", unlimited_float, 0.0, -4.0, 4.0, "Map 5 affine coefficient xx. Normally written by the L-System script."),
        param!("m5_xy", "M5 XY", unlimited_float, 0.0, -4.0, 4.0, "Map 5 affine coefficient xy. Normally written by the L-System script."),
        param!("m5_xz", "M5 XZ", unlimited_float, 0.0, -4.0, 4.0, "Map 5 affine coefficient xz. Normally written by the L-System script."),
        param!("m5_yx", "M5 YX", unlimited_float, 0.0, -4.0, 4.0, "Map 5 affine coefficient yx. Normally written by the L-System script."),
        param!("m5_yy", "M5 YY", unlimited_float, 0.0, -4.0, 4.0, "Map 5 affine coefficient yy. Normally written by the L-System script."),
        param!("m5_yz", "M5 YZ", unlimited_float, 0.0, -4.0, 4.0, "Map 5 affine coefficient yz. Normally written by the L-System script."),
        param!("m5_zx", "M5 ZX", unlimited_float, 0.0, -4.0, 4.0, "Map 5 affine coefficient zx. Normally written by the L-System script."),
        param!("m5_zy", "M5 ZY", unlimited_float, 0.0, -4.0, 4.0, "Map 5 affine coefficient zy. Normally written by the L-System script."),
        param!("m5_zz", "M5 ZZ", unlimited_float, 0.0, -4.0, 4.0, "Map 5 affine coefficient zz. Normally written by the L-System script."),
        param!("m5_tx", "M5 TX", unlimited_float, 0.0, -4.0, 4.0, "Map 5 affine coefficient tx. Normally written by the L-System script."),
        param!("m5_ty", "M5 TY", unlimited_float, 0.0, -4.0, 4.0, "Map 5 affine coefficient ty. Normally written by the L-System script."),
        param!("m5_tz", "M5 TZ", unlimited_float, 0.0, -4.0, 4.0, "Map 5 affine coefficient tz. Normally written by the L-System script."),
        param!("m6_xx", "M6 XX", unlimited_float, 0.0, -4.0, 4.0, "Map 6 affine coefficient xx. Normally written by the L-System script."),
        param!("m6_xy", "M6 XY", unlimited_float, 0.0, -4.0, 4.0, "Map 6 affine coefficient xy. Normally written by the L-System script."),
        param!("m6_xz", "M6 XZ", unlimited_float, 0.0, -4.0, 4.0, "Map 6 affine coefficient xz. Normally written by the L-System script."),
        param!("m6_yx", "M6 YX", unlimited_float, 0.0, -4.0, 4.0, "Map 6 affine coefficient yx. Normally written by the L-System script."),
        param!("m6_yy", "M6 YY", unlimited_float, 0.0, -4.0, 4.0, "Map 6 affine coefficient yy. Normally written by the L-System script."),
        param!("m6_yz", "M6 YZ", unlimited_float, 0.0, -4.0, 4.0, "Map 6 affine coefficient yz. Normally written by the L-System script."),
        param!("m6_zx", "M6 ZX", unlimited_float, 0.0, -4.0, 4.0, "Map 6 affine coefficient zx. Normally written by the L-System script."),
        param!("m6_zy", "M6 ZY", unlimited_float, 0.0, -4.0, 4.0, "Map 6 affine coefficient zy. Normally written by the L-System script."),
        param!("m6_zz", "M6 ZZ", unlimited_float, 0.0, -4.0, 4.0, "Map 6 affine coefficient zz. Normally written by the L-System script."),
        param!("m6_tx", "M6 TX", unlimited_float, 0.0, -4.0, 4.0, "Map 6 affine coefficient tx. Normally written by the L-System script."),
        param!("m6_ty", "M6 TY", unlimited_float, 0.0, -4.0, 4.0, "Map 6 affine coefficient ty. Normally written by the L-System script."),
        param!("m6_tz", "M6 TZ", unlimited_float, 0.0, -4.0, 4.0, "Map 6 affine coefficient tz. Normally written by the L-System script."),
        param!("m7_xx", "M7 XX", unlimited_float, 0.0, -4.0, 4.0, "Map 7 affine coefficient xx. Normally written by the L-System script."),
        param!("m7_xy", "M7 XY", unlimited_float, 0.0, -4.0, 4.0, "Map 7 affine coefficient xy. Normally written by the L-System script."),
        param!("m7_xz", "M7 XZ", unlimited_float, 0.0, -4.0, 4.0, "Map 7 affine coefficient xz. Normally written by the L-System script."),
        param!("m7_yx", "M7 YX", unlimited_float, 0.0, -4.0, 4.0, "Map 7 affine coefficient yx. Normally written by the L-System script."),
        param!("m7_yy", "M7 YY", unlimited_float, 0.0, -4.0, 4.0, "Map 7 affine coefficient yy. Normally written by the L-System script."),
        param!("m7_yz", "M7 YZ", unlimited_float, 0.0, -4.0, 4.0, "Map 7 affine coefficient yz. Normally written by the L-System script."),
        param!("m7_zx", "M7 ZX", unlimited_float, 0.0, -4.0, 4.0, "Map 7 affine coefficient zx. Normally written by the L-System script."),
        param!("m7_zy", "M7 ZY", unlimited_float, 0.0, -4.0, 4.0, "Map 7 affine coefficient zy. Normally written by the L-System script."),
        param!("m7_zz", "M7 ZZ", unlimited_float, 0.0, -4.0, 4.0, "Map 7 affine coefficient zz. Normally written by the L-System script."),
        param!("m7_tx", "M7 TX", unlimited_float, 0.0, -4.0, 4.0, "Map 7 affine coefficient tx. Normally written by the L-System script."),
        param!("m7_ty", "M7 TY", unlimited_float, 0.0, -4.0, 4.0, "Map 7 affine coefficient ty. Normally written by the L-System script."),
        param!("m7_tz", "M7 TZ", unlimited_float, 0.0, -4.0, 4.0, "Map 7 affine coefficient tz. Normally written by the L-System script."),
        param!("m8_xx", "M8 XX", unlimited_float, 0.0, -4.0, 4.0, "Map 8 affine coefficient xx. Normally written by the L-System script."),
        param!("m8_xy", "M8 XY", unlimited_float, 0.0, -4.0, 4.0, "Map 8 affine coefficient xy. Normally written by the L-System script."),
        param!("m8_xz", "M8 XZ", unlimited_float, 0.0, -4.0, 4.0, "Map 8 affine coefficient xz. Normally written by the L-System script."),
        param!("m8_yx", "M8 YX", unlimited_float, 0.0, -4.0, 4.0, "Map 8 affine coefficient yx. Normally written by the L-System script."),
        param!("m8_yy", "M8 YY", unlimited_float, 0.0, -4.0, 4.0, "Map 8 affine coefficient yy. Normally written by the L-System script."),
        param!("m8_yz", "M8 YZ", unlimited_float, 0.0, -4.0, 4.0, "Map 8 affine coefficient yz. Normally written by the L-System script."),
        param!("m8_zx", "M8 ZX", unlimited_float, 0.0, -4.0, 4.0, "Map 8 affine coefficient zx. Normally written by the L-System script."),
        param!("m8_zy", "M8 ZY", unlimited_float, 0.0, -4.0, 4.0, "Map 8 affine coefficient zy. Normally written by the L-System script."),
        param!("m8_zz", "M8 ZZ", unlimited_float, 0.0, -4.0, 4.0, "Map 8 affine coefficient zz. Normally written by the L-System script."),
        param!("m8_tx", "M8 TX", unlimited_float, 0.0, -4.0, 4.0, "Map 8 affine coefficient tx. Normally written by the L-System script."),
        param!("m8_ty", "M8 TY", unlimited_float, 0.0, -4.0, 4.0, "Map 8 affine coefficient ty. Normally written by the L-System script."),
        param!("m8_tz", "M8 TZ", unlimited_float, 0.0, -4.0, 4.0, "Map 8 affine coefficient tz. Normally written by the L-System script."),
        param!("m9_xx", "M9 XX", unlimited_float, 0.0, -4.0, 4.0, "Map 9 affine coefficient xx. Normally written by the L-System script."),
        param!("m9_xy", "M9 XY", unlimited_float, 0.0, -4.0, 4.0, "Map 9 affine coefficient xy. Normally written by the L-System script."),
        param!("m9_xz", "M9 XZ", unlimited_float, 0.0, -4.0, 4.0, "Map 9 affine coefficient xz. Normally written by the L-System script."),
        param!("m9_yx", "M9 YX", unlimited_float, 0.0, -4.0, 4.0, "Map 9 affine coefficient yx. Normally written by the L-System script."),
        param!("m9_yy", "M9 YY", unlimited_float, 0.0, -4.0, 4.0, "Map 9 affine coefficient yy. Normally written by the L-System script."),
        param!("m9_yz", "M9 YZ", unlimited_float, 0.0, -4.0, 4.0, "Map 9 affine coefficient yz. Normally written by the L-System script."),
        param!("m9_zx", "M9 ZX", unlimited_float, 0.0, -4.0, 4.0, "Map 9 affine coefficient zx. Normally written by the L-System script."),
        param!("m9_zy", "M9 ZY", unlimited_float, 0.0, -4.0, 4.0, "Map 9 affine coefficient zy. Normally written by the L-System script."),
        param!("m9_zz", "M9 ZZ", unlimited_float, 0.0, -4.0, 4.0, "Map 9 affine coefficient zz. Normally written by the L-System script."),
        param!("m9_tx", "M9 TX", unlimited_float, 0.0, -4.0, 4.0, "Map 9 affine coefficient tx. Normally written by the L-System script."),
        param!("m9_ty", "M9 TY", unlimited_float, 0.0, -4.0, 4.0, "Map 9 affine coefficient ty. Normally written by the L-System script."),
        param!("m9_tz", "M9 TZ", unlimited_float, 0.0, -4.0, 4.0, "Map 9 affine coefficient tz. Normally written by the L-System script."),
        param!("m10_xx", "M10 XX", unlimited_float, 0.0, -4.0, 4.0, "Map 10 affine coefficient xx. Normally written by the L-System script."),
        param!("m10_xy", "M10 XY", unlimited_float, 0.0, -4.0, 4.0, "Map 10 affine coefficient xy. Normally written by the L-System script."),
        param!("m10_xz", "M10 XZ", unlimited_float, 0.0, -4.0, 4.0, "Map 10 affine coefficient xz. Normally written by the L-System script."),
        param!("m10_yx", "M10 YX", unlimited_float, 0.0, -4.0, 4.0, "Map 10 affine coefficient yx. Normally written by the L-System script."),
        param!("m10_yy", "M10 YY", unlimited_float, 0.0, -4.0, 4.0, "Map 10 affine coefficient yy. Normally written by the L-System script."),
        param!("m10_yz", "M10 YZ", unlimited_float, 0.0, -4.0, 4.0, "Map 10 affine coefficient yz. Normally written by the L-System script."),
        param!("m10_zx", "M10 ZX", unlimited_float, 0.0, -4.0, 4.0, "Map 10 affine coefficient zx. Normally written by the L-System script."),
        param!("m10_zy", "M10 ZY", unlimited_float, 0.0, -4.0, 4.0, "Map 10 affine coefficient zy. Normally written by the L-System script."),
        param!("m10_zz", "M10 ZZ", unlimited_float, 0.0, -4.0, 4.0, "Map 10 affine coefficient zz. Normally written by the L-System script."),
        param!("m10_tx", "M10 TX", unlimited_float, 0.0, -4.0, 4.0, "Map 10 affine coefficient tx. Normally written by the L-System script."),
        param!("m10_ty", "M10 TY", unlimited_float, 0.0, -4.0, 4.0, "Map 10 affine coefficient ty. Normally written by the L-System script."),
        param!("m10_tz", "M10 TZ", unlimited_float, 0.0, -4.0, 4.0, "Map 10 affine coefficient tz. Normally written by the L-System script."),
        param!("m11_xx", "M11 XX", unlimited_float, 0.0, -4.0, 4.0, "Map 11 affine coefficient xx. Normally written by the L-System script."),
        param!("m11_xy", "M11 XY", unlimited_float, 0.0, -4.0, 4.0, "Map 11 affine coefficient xy. Normally written by the L-System script."),
        param!("m11_xz", "M11 XZ", unlimited_float, 0.0, -4.0, 4.0, "Map 11 affine coefficient xz. Normally written by the L-System script."),
        param!("m11_yx", "M11 YX", unlimited_float, 0.0, -4.0, 4.0, "Map 11 affine coefficient yx. Normally written by the L-System script."),
        param!("m11_yy", "M11 YY", unlimited_float, 0.0, -4.0, 4.0, "Map 11 affine coefficient yy. Normally written by the L-System script."),
        param!("m11_yz", "M11 YZ", unlimited_float, 0.0, -4.0, 4.0, "Map 11 affine coefficient yz. Normally written by the L-System script."),
        param!("m11_zx", "M11 ZX", unlimited_float, 0.0, -4.0, 4.0, "Map 11 affine coefficient zx. Normally written by the L-System script."),
        param!("m11_zy", "M11 ZY", unlimited_float, 0.0, -4.0, 4.0, "Map 11 affine coefficient zy. Normally written by the L-System script."),
        param!("m11_zz", "M11 ZZ", unlimited_float, 0.0, -4.0, 4.0, "Map 11 affine coefficient zz. Normally written by the L-System script."),
        param!("m11_tx", "M11 TX", unlimited_float, 0.0, -4.0, 4.0, "Map 11 affine coefficient tx. Normally written by the L-System script."),
        param!("m11_ty", "M11 TY", unlimited_float, 0.0, -4.0, 4.0, "Map 11 affine coefficient ty. Normally written by the L-System script."),
        param!("m11_tz", "M11 TZ", unlimited_float, 0.0, -4.0, 4.0, "Map 11 affine coefficient tz. Normally written by the L-System script."),
        param!("anchored", "Anchored", bool, false, "Vertex-chain mode: connect the images of the anchor point in consecutive cells instead of each cell's entry-to-exit span. Space-filling curves need this — their spans lie on the cell-edge lattice and overlap each other (three-way joins, phantom dead ends); the centre chain is the classic self-avoiding drawing."),
        param!("anchor_x", "Anchor X", unlimited_float, 0.5, -2.0, 2.0, "Anchor point x (the attractor's centre, set by the script)."),
        param!("anchor_y", "Anchor Y", unlimited_float, 0.0, -2.0, 2.0, "Anchor point y."),
        param!("anchor_z", "Anchor Z", unlimited_float, 0.0, -2.0, 2.0, "Anchor point z."),
    ],
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

const WGSL_2D: &str = r#"
// The digit-selected affine chain applied to one anchor point
// (least-significant digit innermost). The shader builder only carries
// top-level fn and const items, so no struct: both cell endpoints come
// from two calls with the entry/exit anchors — the SAME digit chain, so
// numeric error shifts a segment but never tilts it.
fn lsp3_point(xform_id: u32, variation_id: u32, idx: u32, iters: u32, n: u32, anchor: vec3<f32>) -> vec3<f32> {
    var v = anchor;
    var rem = idx;
    for (var j = 0u; j < iters; j = j + 1u) {
        let d = rem % n;
        rem = rem / n;
        let base = 4u + d * 12u;
        let m00 = get_param(xform_id, variation_id, base);
        let m01 = get_param(xform_id, variation_id, base + 1u);
        let m02 = get_param(xform_id, variation_id, base + 2u);
        let m10 = get_param(xform_id, variation_id, base + 3u);
        let m11 = get_param(xform_id, variation_id, base + 4u);
        let m12 = get_param(xform_id, variation_id, base + 5u);
        let m20 = get_param(xform_id, variation_id, base + 6u);
        let m21 = get_param(xform_id, variation_id, base + 7u);
        let m22 = get_param(xform_id, variation_id, base + 8u);
        let tx = get_param(xform_id, variation_id, base + 9u);
        let ty = get_param(xform_id, variation_id, base + 10u);
        let tz = get_param(xform_id, variation_id, base + 11u);
        v = vec3<f32>(
            m00 * v.x + m01 * v.y + m02 * v.z + tx,
            m10 * v.x + m11 * v.y + m12 * v.z + ty,
            m20 * v.x + m21 * v.y + m22 * v.z + tz,
        );
    }
    return v;
}

fn variation_lsystem_path_3D(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    let iters = clamp(u32(get_param(xform_id, variation_id, 0u)), 1u, 12u);
    let n = clamp(u32(get_param(xform_id, variation_id, 1u)), 2u, 12u);
    let connect = get_param(xform_id, variation_id, 2u) > 0.5;
    let dc = get_param(xform_id, variation_id, 3u) > 0.5;

    let anchored = get_param(xform_id, variation_id, 148u) > 0.5;

    var total = 1u;
    for (var j = 0u; j < iters; j = j + 1u) {
        total = min(total * n, 16000000u);
    }
    let t = rng_nextf(rng);

    var out: vec3<f32>;
    if (anchored) {
        // Vertex chain through the anchor's image in each cell — the
        // classic space-filling drawing (anchor = attractor centre gives
        // cell centres). Cell spans lie on the cell-edge lattice and
        // OVERLAP each other; centres are self-avoiding.
        let anchor = vec3<f32>(
            get_param(xform_id, variation_id, 149u),
            get_param(xform_id, variation_id, 150u),
            get_param(xform_id, variation_id, 151u));
        let segs = max(total - 1u, 1u);
        let ts = t * f32(segs);
        let idx = min(u32(ts), segs - 1u);
        out = lsp3_point(xform_id, variation_id, idx, iters, n, anchor);
        if (connect) {
            let nxt = lsp3_point(xform_id, variation_id, idx + 1u, iters, n, anchor);
            out = mix(out, nxt, clamp(ts - f32(idx), 0.0, 1.0));
        }
    } else {
        let idx = min(u32(t * f32(total)), total - 1u);
        out = lsp3_point(xform_id, variation_id, idx, iters, n, vec3<f32>(0.0, 0.0, 0.0));
        if (connect) {
            let exit_p = lsp3_point(xform_id, variation_id, idx, iters, n, vec3<f32>(1.0, 0.0, 0.0));
            let frac = clamp(t * f32(total) - f32(idx), 0.0, 1.0);
            out = mix(out, exit_p, frac);
        }
    }
    if (dc) {
        *vc = t;
    }
    // 2D render: the xy shadow of the 3D path.
    return out.xy;
}
"#;

const WGSL_3D: &str = r#"
// The digit-selected affine chain applied to one anchor point
// (least-significant digit innermost). The shader builder only carries
// top-level fn and const items, so no struct: both cell endpoints come
// from two calls with the entry/exit anchors — the SAME digit chain, so
// numeric error shifts a segment but never tilts it.
fn lsp3_point(xform_id: u32, variation_id: u32, idx: u32, iters: u32, n: u32, anchor: vec3<f32>) -> vec3<f32> {
    var v = anchor;
    var rem = idx;
    for (var j = 0u; j < iters; j = j + 1u) {
        let d = rem % n;
        rem = rem / n;
        let base = 4u + d * 12u;
        let m00 = get_param(xform_id, variation_id, base);
        let m01 = get_param(xform_id, variation_id, base + 1u);
        let m02 = get_param(xform_id, variation_id, base + 2u);
        let m10 = get_param(xform_id, variation_id, base + 3u);
        let m11 = get_param(xform_id, variation_id, base + 4u);
        let m12 = get_param(xform_id, variation_id, base + 5u);
        let m20 = get_param(xform_id, variation_id, base + 6u);
        let m21 = get_param(xform_id, variation_id, base + 7u);
        let m22 = get_param(xform_id, variation_id, base + 8u);
        let tx = get_param(xform_id, variation_id, base + 9u);
        let ty = get_param(xform_id, variation_id, base + 10u);
        let tz = get_param(xform_id, variation_id, base + 11u);
        v = vec3<f32>(
            m00 * v.x + m01 * v.y + m02 * v.z + tx,
            m10 * v.x + m11 * v.y + m12 * v.z + ty,
            m20 * v.x + m21 * v.y + m22 * v.z + tz,
        );
    }
    return v;
}

fn variation_lsystem_path_3D(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    let iters = clamp(u32(get_param(xform_id, variation_id, 0u)), 1u, 12u);
    let n = clamp(u32(get_param(xform_id, variation_id, 1u)), 2u, 12u);
    let connect = get_param(xform_id, variation_id, 2u) > 0.5;
    let dc = get_param(xform_id, variation_id, 3u) > 0.5;

    let anchored = get_param(xform_id, variation_id, 148u) > 0.5;

    var total = 1u;
    for (var j = 0u; j < iters; j = j + 1u) {
        total = min(total * n, 16000000u);
    }
    let t = rng_nextf(rng);

    var out: vec3<f32>;
    if (anchored) {
        // Vertex chain through the anchor's image in each cell — the
        // classic space-filling drawing (anchor = attractor centre gives
        // cell centres). Cell spans lie on the cell-edge lattice and
        // OVERLAP each other; centres are self-avoiding.
        let anchor = vec3<f32>(
            get_param(xform_id, variation_id, 149u),
            get_param(xform_id, variation_id, 150u),
            get_param(xform_id, variation_id, 151u));
        let segs = max(total - 1u, 1u);
        let ts = t * f32(segs);
        let idx = min(u32(ts), segs - 1u);
        out = lsp3_point(xform_id, variation_id, idx, iters, n, anchor);
        if (connect) {
            let nxt = lsp3_point(xform_id, variation_id, idx + 1u, iters, n, anchor);
            out = mix(out, nxt, clamp(ts - f32(idx), 0.0, 1.0));
        }
    } else {
        let idx = min(u32(t * f32(total)), total - 1u);
        out = lsp3_point(xform_id, variation_id, idx, iters, n, vec3<f32>(0.0, 0.0, 0.0));
        if (connect) {
            let exit_p = lsp3_point(xform_id, variation_id, idx, iters, n, vec3<f32>(1.0, 0.0, 0.0));
            let frac = clamp(t * f32(total) - f32(idx), 0.0, 1.0);
            out = mix(out, exit_p, frac);
        }
    }
    if (dc) {
        *vc = t;
    }
    return out;
}
"#;
