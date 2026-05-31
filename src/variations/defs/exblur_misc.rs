//! `exblur` (zephyrtronium 2010, ported by dark-beam 2014)
//!
//! 3D Gaussian blur centered at an offset origin, scaled by a power
//! of squared-distance. Computes spherical coordinates (theta, phi)
//! around the offset, then jitters by two random angles + a random
//! scalar that approximates a Gaussian via sum-of-4-uniforms minus 2.
//!
//!   - 5 user params: dist, r, x_origin, y_origin, z_origin
//!   - 0 init slots
//!
//! cpp uses a persistent `_r[4]` buffer cycled across iterations
//! (perf optimization for the Gaussian). We use 4 fresh randoms
//! per call (statistically equivalent without persistent state),
//! same approach as `pre_blur3D` (batch 56).
//!
//! Body has w in `rr = w · pow(n, dist) · (gauss_sum - 2)`. Output
//! `rr · (sv · cu + rsrv · cru)` — clean factor through outer.
//!
//! Source: `output/jwildfire-vars/output/exblur.cpp`.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// 3D Gaussian blur centered at an offset origin — computes spherical
/// coordinates around `(x_origin, -y_origin, z_origin)`, jitters by two
/// random angles plus a Gaussian-approximated scalar (sum of 4 uniforms −
/// 2), and scales by `pow(distance², dist)`. The blur intensity grows with
/// distance from the offset origin.
///
/// # Authors
/// - zephyrtronium
/// - DarkBeam
pub static EXBLUR: VariationDef = VariationDef {
    name: "exblur",
    aliases: &[],
    display_name: "Ex Blur",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("dist", "Dist", unlimited_float, 0.5, -10.0, 10.0, "Power exponent on the squared-distance scaling. Controls how the blur intensity grows with distance from the origin."),
        param!("r", "R", unlimited_float, 0.0, -10.0, 10.0, "Scaling factor on the perpendicular jitter."),
        param!("x_origin", "X Origin", unlimited_float, 0.0, -10.0, 10.0, "X center of the blur (subtracted from input X)."),
        param!("y_origin", "Y Origin", unlimited_float, 0.0, -10.0, 10.0, "Y center of the blur — note the upstream sign convention: added to input Y rather than subtracted."),
        param!("z_origin", "Z Origin", unlimited_float, 0.0, -10.0, 10.0, "Z center of the blur (subtracted from input Z; 3D only)."),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_exblur(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let dist = get_param(xform_id, variation_id, 0u);
    let r_p = get_param(xform_id, variation_id, 1u);
    let x_origin = get_param(xform_id, variation_id, 2u);
    let y_origin = get_param(xform_id, variation_id, 3u);
    let two_pi = 6.28318530717959;

    let ox = p.x - x_origin;
    let oy = p.y + y_origin;
    let n = max(ox * ox + oy * oy, 1e-30);
    let gauss = rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) - 2.0;
    let rr = pow(n, dist) * gauss;
    let theta = atan2(oy, ox);
    let phi = 1.5707963267948966; // acos(0) — 2D fallback when oz = 0
    let cu = cos(theta);
    let su = sin(theta);
    let sv = sin(phi);
    let theta_u = rng_nextf(rng) * two_pi;
    let cru = cos(theta_u);
    let sru = sin(theta_u);
    let theta_v = rng_nextf(rng) * two_pi;
    let srv = sin(theta_v);
    let rsrv = r_p * srv;
    return vec2<f32>(rr * (sv * cu + rsrv * cru), rr * (sv * su + rsrv * sru));
}
"#,
    wgsl_3d: r#"
fn variation_exblur(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let dist = get_param(xform_id, variation_id, 0u);
    let r_p = get_param(xform_id, variation_id, 1u);
    let x_origin = get_param(xform_id, variation_id, 2u);
    let y_origin = get_param(xform_id, variation_id, 3u);
    let z_origin = get_param(xform_id, variation_id, 4u);
    let two_pi = 6.28318530717959;

    let ox = p.x - x_origin;
    let oy = p.y + y_origin;
    let oz = p.z - z_origin;
    let n = max(ox * ox + oy * oy + oz * oz, 1e-30);
    let gauss = rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) - 2.0;
    let rr = pow(n, dist) * gauss;
    let theta = atan2(oy, ox);
    let phi = acos(clamp(oz / sqrt(n), -1.0, 1.0));
    let cu = cos(theta);
    let su = sin(theta);
    let cv = cos(phi);
    let sv = sin(phi);
    let theta_u = rng_nextf(rng) * two_pi;
    let cru = cos(theta_u);
    let sru = sin(theta_u);
    let theta_v = rng_nextf(rng) * two_pi;
    let crv = cos(theta_v);
    let srv = sin(theta_v);
    let rsrv = r_p * srv;
    return vec3<f32>(rr * (sv * cu + rsrv * cru),
                     rr * (sv * su + rsrv * sru),
                     rr * (cv + r_p * crv));
}
"#,
};
