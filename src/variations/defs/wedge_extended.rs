//! Wedge family extensions
//!
//! Two upstream cousins of the basic `wedge` already in our registry:
//!   - `wedge_julia` (apo plugin pack) — N-th-root-branched wedge
//!   - `wedge_sph`   (apo plugin pack) — wedge applied in spherical-style
//!                                       inversion space
//!
//! Sources:
//!   - output/jwildfire-vars/output/wedge_julia.cpp
//!   - output/jwildfire-vars/output/wedge_sph.cpp
//!
//! Notes on faithfulness:
//!   - Both port the cpp's `atan2(FTx, FTy)` ordering (swapped from
//!     Java's `getPrecalcAtanYX() = atan2(y, x)`). Same systematic
//!     porter swap as `log_db`, `cpow2/3`, etc. — preserved.
//!   - `wedge_julia` upstream computes `ca = cos(sa)` (cos-of-sin-of-a,
//!     NOT cos(a)). Both the C++ port and the JWildfire Java original
//!     have this; appears to be original-author intent or a long-lived
//!     quirk. Preserved.
//!   - VVAR factors out cleanly through the outer multiplier in both.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// wedge_julia: N-th-root-branched wedge
//   Body:
//     r       = (x² + y²)^(dist/power/2)
//     t_rnd   = floor(|power| · rand)               (random N-th-root branch)
//     a       = (atan2(x, y) + 2π · t_rnd) / power  (upstream cpp swap)
//     c       = floor((count · a + π) / (2π))
//     a       = a · cf + c · angle                  (cf = 1 − angle·count/(2π))
//     out     = (cos(sin(a)), sin(a)) · r           (cos(sin(a)) — quirk, see notes)
// =============================================================================
/// N-th-root-branched wedge — combines JuliaN angular branching (`power`,
/// random branch per iteration) with wedge sectoring (`angle`, `count`).
///
/// # Authors
/// - Apophysis Plugin Pack
pub static WEDGE_JULIA: VariationDef = VariationDef {
    name: "wedge_julia",
    aliases: &[],
    display_name: "Wedge Julia",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng],
    parameters: &[
        param!("power", "Power", unlimited_float, 7.0, -20.0, 20.0, "Number of Julia branches. Higher = more arms."),
        param!("dist", "Distance", unlimited_float, 0.2, -10.0, 10.0, "Radial distance scaling — pushes arms inward or outward."),
        param!("count", "Count", unlimited_float, 2.0, -10.0, 10.0, "Number of wedge sectors around the center."),
        param!("angle", "Angle", unlimited_float, 0.3, -10.0, 10.0, "Wedge sector rotation, in radians."),
    ],
    // 3 derived values at slots 4..7:
    //   4: cf  (1 − angle · count / (2π))
    //   5: r_n (|power|)
    //   6: cn  (dist / power / 2)
    init_param_count: 3,
    wgsl_init: Some(r#"
fn init_wedge_julia(user: array<f32, 4>) -> array<f32, 3> {
    let power = user[0];
    let dist = user[1];
    let count = user[2];
    let angle = user[3];
    let inv_two_pi = 0.15915494309189535;
    let safe_power = select(power, 1e-30, power == 0.0);
    var out: array<f32, 3>;
    out[0] = 1.0 - angle * count * inv_two_pi;
    out[1] = abs(power);
    out[2] = dist / safe_power * 0.5;
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_wedge_julia(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let power = get_param(xform_id, variation_id, 0u);
    let count = get_param(xform_id, variation_id, 2u);
    let angle = get_param(xform_id, variation_id, 3u);
    let cf = get_param(xform_id, variation_id, 4u);
    let r_n = get_param(xform_id, variation_id, 5u);
    let cn = get_param(xform_id, variation_id, 6u);
    let inv_two_pi = 0.15915494309189535;
    let two_pi = 6.28318530717959;

    let r = pow(max(p.x * p.x + p.y * p.y, 1e-30), cn);
    let t_rnd = floor(r_n * rng_nextf(rng));
    let safe_power = select(power, 1e-30, power == 0.0);
    var a = (atan2(p.x, p.y) + two_pi * t_rnd) / safe_power;
    let c = floor((count * a + 3.14159265358979) * inv_two_pi);
    a = a * cf + c * angle;
    let sa = sin(a);
    let ca = cos(sa);  // upstream quirk: cos-of-sin, not cos(a)
    return vec2<f32>(r * ca, r * sa);
}
"#,
    wgsl_3d: r#"
fn variation_wedge_julia(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let power = get_param(xform_id, variation_id, 0u);
    let count = get_param(xform_id, variation_id, 2u);
    let angle = get_param(xform_id, variation_id, 3u);
    let cf = get_param(xform_id, variation_id, 4u);
    let r_n = get_param(xform_id, variation_id, 5u);
    let cn = get_param(xform_id, variation_id, 6u);
    let inv_two_pi = 0.15915494309189535;
    let two_pi = 6.28318530717959;

    let r = pow(max(p.x * p.x + p.y * p.y, 1e-30), cn);
    let t_rnd = floor(r_n * rng_nextf(rng));
    let safe_power = select(power, 1e-30, power == 0.0);
    var a = (atan2(p.x, p.y) + two_pi * t_rnd) / safe_power;
    let c = floor((count * a + 3.14159265358979) * inv_two_pi);
    a = a * cf + c * angle;
    let sa = sin(a);
    let ca = cos(sa);
    return vec3<f32>(r * ca, r * sa, p.z);
}
"#,
};

// =============================================================================
// wedge_sph: wedge applied in spherical-inversion space
//   Body:
//     r        = 1 / (sqrt(x² + y²) + ε)
//     a        = atan2(x, y) + swirl · r          (upstream cpp swap)
//     c        = floor((count · a + π) / (2π))
//     a        = a · cf + c · angle               (cf = 1 − angle·count/(2π))
//     r        = r + hole                         (weight applied outside)
//     out      = r · (cos(a), sin(a))
// =============================================================================
/// Wedge applied in spherical-inversion space — like Wedge but the input
/// is first inverted through the unit circle, producing a wrapped/folded
/// version of the wedge pattern.
///
/// # Authors
/// - Apophysis Plugin Pack
pub static WEDGE_SPH: VariationDef = VariationDef {
    name: "wedge_sph",
    aliases: &[],
    display_name: "Wedge Sph",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[],
    parameters: &[
        param!("angle", "Angle", unlimited_float, 0.2, -10.0, 10.0, "Wedge sector rotation, in radians."),
        param!("hole", "Hole", unlimited_float, 0.2, -10.0, 10.0, "Radial offset added after the inversion. Positive opens a hole at the center; negative compresses inward."),
        param!("count", "Count", unlimited_float, 2.0, -10.0, 10.0, "Number of wedge sectors around the center."),
        param!("swirl", "Swirl", unlimited_float, 0.3, -10.0, 10.0, "Extra rotation that grows with distance — gives the wedges a spiral."),
    ],
    // 1 derived value at slot 4:
    //   4: cf  (1 − angle · count / (2π))
    init_param_count: 1,
    wgsl_init: Some(r#"
fn init_wedge_sph(user: array<f32, 4>) -> array<f32, 1> {
    let angle = user[0];
    let count = user[2];
    let inv_two_pi = 0.15915494309189535;
    var out: array<f32, 1>;
    out[0] = 1.0 - angle * count * inv_two_pi;
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_wedge_sph(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let angle = get_param(xform_id, variation_id, 0u);
    let hole = get_param(xform_id, variation_id, 1u);
    let count = get_param(xform_id, variation_id, 2u);
    let swirl = get_param(xform_id, variation_id, 3u);
    let cf = get_param(xform_id, variation_id, 4u);
    let inv_two_pi = 0.15915494309189535;

    let r0 = 1.0 / (sqrt(p.x * p.x + p.y * p.y) + 1e-6);
    var a = atan2(p.x, p.y) + swirl * r0;
    let c = floor((count * a + 3.14159265358979) * inv_two_pi);
    a = a * cf + c * angle;
    let r = r0 + hole;
    return vec2<f32>(r * cos(a), r * sin(a));
}
"#,
    wgsl_3d: r#"
fn variation_wedge_sph(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let angle = get_param(xform_id, variation_id, 0u);
    let hole = get_param(xform_id, variation_id, 1u);
    let count = get_param(xform_id, variation_id, 2u);
    let swirl = get_param(xform_id, variation_id, 3u);
    let cf = get_param(xform_id, variation_id, 4u);
    let inv_two_pi = 0.15915494309189535;

    let r0 = 1.0 / (sqrt(p.x * p.x + p.y * p.y) + 1e-6);
    var a = atan2(p.x, p.y) + swirl * r0;
    let c = floor((count * a + 3.14159265358979) * inv_two_pi);
    a = a * cf + c * angle;
    let r = r0 + hole;
    return vec3<f32>(r * cos(a), r * sin(a), p.z);
}
"#,
};
