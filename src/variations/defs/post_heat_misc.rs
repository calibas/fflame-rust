//! `post_heat` (zephyrtronium)
//!
//! Post-phase 3D heat-distortion: applies sin-based perturbations to
//! r/θ/φ spherical coordinates of the accumulator, then re-projects.
//!
//!   - 9 user params: theta_period, theta_phase, theta_amp,
//!                    phi_period, phi_phase, phi_amp,
//!                    r_period, r_phase, r_amp
//!   - 0 init slots: cpp precomputes 9 derived values, but with 9 user
//!                   params already filling the budget we'd risk
//!                   overflow. We compute them at runtime instead —
//!                   post-phase variations don't run as often as normal-
//!                   phase, so the per-iteration cost is acceptable.
//!
//! Body has w in `at`, `ap`, `ar` (used inside the sine perturbations),
//! so post-phase needs `needs_transform` to read w.
//!
//! cpp APO_VARIABLES is empty — porter omitted all 9 user params.
//! Recovered from the Java setParameter list (see end of cpp file).
//!
//! Source: `output/jwildfire-vars/output/post_heat.cpp`.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Post-phase 3D heat-distortion — converts the accumulated point to
/// spherical coordinates `(r, θ, φ)`, applies sin-based perturbations to
/// each (period/phase/amplitude per axis), then re-projects to cartesian.
/// Inspired by heat-distortion atmospheric effects: each spherical
/// coordinate gets `coord += amp · sin(2π·coord/period + phase/period)`.
///
/// # Authors
/// - zephyrtronium
pub static POST_HEAT: VariationDef = VariationDef {
    name: "post_heat",
    aliases: &[],
    display_name: "Post Heat",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Post,
    needs_rng: false,
    parameters: &[
        param!("theta_period", "Theta Period", unlimited_float, 0.0, -100.0, 100.0, "Period of the θ (azimuthal angle) sine perturbation. 0 disables theta perturbation."),
        param!("theta_phase", "Theta Phase", unlimited_float, 0.0, -10.0, 10.0, "Phase offset on the θ sine."),
        param!("theta_amp", "Theta Amp", unlimited_float, 0.0, -10.0, 10.0, "Amplitude of the θ sine perturbation."),
        param!("phi_period", "Phi Period", unlimited_float, 0.0, -100.0, 100.0, "Period of the φ (polar angle) sine perturbation. 0 disables phi perturbation."),
        param!("phi_phase", "Phi Phase", unlimited_float, 0.0, -10.0, 10.0, "Phase offset on the φ sine."),
        param!("phi_amp", "Phi Amp", unlimited_float, 0.0, -10.0, 10.0, "Amplitude of the φ sine perturbation."),
        param!("r_period", "R Period", unlimited_float, 0.0, -100.0, 100.0, "Period of the radial sine perturbation. 0 disables r perturbation."),
        param!("r_phase", "R Phase", unlimited_float, 0.0, -10.0, 10.0, "Phase offset on the r sine."),
        param!("r_amp", "R Amp", unlimited_float, 0.0, -10.0, 10.0, "Amplitude of the radial sine perturbation."),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_post_heat(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let theta_period = get_param(xform_id, variation_id, 0u);
    let theta_phase = get_param(xform_id, variation_id, 1u);
    let theta_amp = get_param(xform_id, variation_id, 2u);
    let r_period = get_param(xform_id, variation_id, 6u);
    let r_phase = get_param(xform_id, variation_id, 7u);
    let r_amp = get_param(xform_id, variation_id, 8u);
    let two_pi = 6.28318530717959;
    let w = transforms[xform_id].variations[variation_id];

    let tx = select(0.0, 1.0 / theta_period, abs(theta_period) > 1e-30);
    let rx = select(0.0, 1.0 / r_period, abs(r_period) > 1e-30);
    let at = w * theta_amp;
    let bt = two_pi * tx;
    let ct = theta_phase * tx;
    let ar = w * r_amp;
    let br = two_pi * rx;
    let cr = r_phase * rx;

    var r = sqrt(p.x * p.x + p.y * p.y);
    let atant = atan2(p.y, p.x);
    r = r + ar * sin(br * r + cr);
    let sint_arg = at * sin(bt * r + ct) + atant;
    return vec2<f32>(r * cos(sint_arg), r * sin(sint_arg));
}
"#,
    wgsl_3d: Some(r#"
fn variation_post_heat(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let theta_period = get_param(xform_id, variation_id, 0u);
    let theta_phase = get_param(xform_id, variation_id, 1u);
    let theta_amp = get_param(xform_id, variation_id, 2u);
    let phi_period = get_param(xform_id, variation_id, 3u);
    let phi_phase = get_param(xform_id, variation_id, 4u);
    let phi_amp = get_param(xform_id, variation_id, 5u);
    let r_period = get_param(xform_id, variation_id, 6u);
    let r_phase = get_param(xform_id, variation_id, 7u);
    let r_amp = get_param(xform_id, variation_id, 8u);
    let two_pi = 6.28318530717959;
    let w = transforms[xform_id].variations[variation_id];

    let tx = select(0.0, 1.0 / theta_period, abs(theta_period) > 1e-30);
    let px = select(0.0, 1.0 / phi_period, abs(phi_period) > 1e-30);
    let rx = select(0.0, 1.0 / r_period, abs(r_period) > 1e-30);
    let at = w * theta_amp;
    let bt = two_pi * tx;
    let ct = theta_phase * tx;
    let ap = w * phi_amp;
    let bp = two_pi * px;
    let cp = phi_phase * px;
    let ar = w * r_amp;
    let br = two_pi * rx;
    let cr = r_phase * rx;

    var r = sqrt(p.x * p.x + p.y * p.y + p.z * p.z);
    let atant = atan2(p.y, p.x);
    r = r + ar * sin(br * r + cr);

    let sint_arg = at * sin(bt * r + ct) + atant;
    let cost = cos(sint_arg);
    let sint = sin(sint_arg);

    let safe_r = select(0.0, p.z / r, abs(r) > 1e-30);
    let acosp = clamp(safe_r, -1.0, 1.0);
    let sinp_arg = ap * sin(bp * r + cp) + acos(acosp);
    let cosp = cos(sinp_arg);
    let sinp = sin(sinp_arg);

    return vec3<f32>(r * cost * sinp, r * sint * sinp, r * cosp);
}
"#),
};
