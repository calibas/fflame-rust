//! `jubiq4d` — full 4D jubiq (original; corrects `jubiQ`'s dropped
//! dimension).
//!
//! Same structure as Brad Stefanov's `jubiQ` — quaternionic Möbius
//! `M(q) = (A·q + B)·(C·q + D)⁻¹`, then the julian2-style angular
//! pick + radial scaling on the XY output with an affine wrapper, plus
//! the preserve-z radial-inversion term — but with the quaternion
//! carried honestly through all four dimensions
//! (`Feature::NeedsW`): input `q = (p.x, p.y, p.z, point_w)` instead
//! of pinning k = 0, and the output k-component (which classic jubiQ
//! never even computes) feeds `point_w_out`. The Möbius stage is then
//! a genuine GL(2,ℍ) fractional-linear action on ℍP¹ ≅ S⁴ (formulas
//! and the composition-equals-matrix-product property verified
//! numerically); the julian2 chimera on top is kept exactly as the
//! original designed it — 2D angular/radial on the (t, i) output
//! components, z from the (j) component plus the radial inversion.
//!
//! The 2D body is identical to `jubiQ`'s (w is 3D-only; with
//! z = w = 0 the formulas coincide). Parameter set matches `jubiQ` so
//! settings copy across.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

pub static JUBIQ4D: VariationDef = VariationDef {
    name: "jubiq4d",
    aliases: &[],
    display_name: "Jubiq 4D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::NeedsW, Feature::AlwaysZ],
    parameters: &[
        param!("power", "Power", int, 1.0, -50.0, 50.0, "Julia angular branch count (integer). 0 collapses the variation to the origin. Otherwise: picks one of |power| equally-spaced angles per iteration and divides the polar angle by power."),
        param!("dist", "Distance", unlimited_float, 1.0, -10.0, 10.0, "Radial exponent multiplier. Combines with power to set the radial power as 0.5·dist/power."),
        param!("a", "A", unlimited_float, 1.0, -10.0, 10.0, "Output XY affine wrapper, component A (X-to-X scale)."),
        param!("b", "B", unlimited_float, 0.0, -10.0, 10.0, "Output XY affine wrapper, component B (Y-to-X scale)."),
        param!("c", "C", unlimited_float, 0.0, -10.0, 10.0, "Output XY affine wrapper, component C (X-to-Y scale)."),
        param!("d", "D", unlimited_float, 1.0, -10.0, 10.0, "Output XY affine wrapper, component D (Y-to-Y scale)."),
        param!("e", "E", unlimited_float, 0.0, -10.0, 10.0, "Output XY affine wrapper, component E (X translation)."),
        param!("f", "F", unlimited_float, 0.0, -10.0, 10.0, "Output XY affine wrapper, component F (Y translation)."),
        param!("qat", "QA t", unlimited_float, 0.0, -10.0, 10.0, "Möbius numerator quaternion A, scalar (t) component. Unlike classic jubiQ the input k rides the per-thread w register and the output k feeds back."),
        param!("qax", "QA x", unlimited_float, 0.0, -10.0, 10.0, "Möbius numerator quaternion A, i component."),
        param!("qay", "QA y", unlimited_float, 0.0, -10.0, 10.0, "Möbius numerator quaternion A, j component."),
        param!("qaz", "QA z", unlimited_float, 0.0, -10.0, 10.0, "Möbius numerator quaternion A, k component."),
        param!("qbt", "QB t", unlimited_float, 0.0, -10.0, 10.0, "Möbius numerator quaternion B, scalar (t) component."),
        param!("qbx", "QB x", unlimited_float, 0.0, -10.0, 10.0, "Möbius numerator quaternion B, i component."),
        param!("qby", "QB y", unlimited_float, 0.0, -10.0, 10.0, "Möbius numerator quaternion B, j component."),
        param!("qbz", "QB z", unlimited_float, 0.0, -10.0, 10.0, "Möbius numerator quaternion B, k component."),
        param!("qct", "QC t", unlimited_float, 0.0, -10.0, 10.0, "Möbius denominator quaternion C, scalar (t) component."),
        param!("qcx", "QC x", unlimited_float, 0.0, -10.0, 10.0, "Möbius denominator quaternion C, i component."),
        param!("qcy", "QC y", unlimited_float, 0.0, -10.0, 10.0, "Möbius denominator quaternion C, j component."),
        param!("qcz", "QC z", unlimited_float, 0.0, -10.0, 10.0, "Möbius denominator quaternion C, k component."),
        param!("qdt", "QD t", unlimited_float, 1.0, -10.0, 10.0, "Möbius denominator quaternion D, scalar (t) component."),
        param!("qdx", "QD x", unlimited_float, 0.0, -10.0, 10.0, "Möbius denominator quaternion D, i component."),
        param!("qdy", "QD y", unlimited_float, 0.0, -10.0, 10.0, "Möbius denominator quaternion D, j component."),
        param!("qdz", "QD z", unlimited_float, 0.0, -10.0, 10.0, "Möbius denominator quaternion D, k component."),
        param!("k_input", "K Input", float, 1.0, 0.0, 1.0, "How much of the per-thread w register feeds the quaternion's k input. 1 = the honest 4D Möbius stage; 0 = xyz output reproduces classic jubiQ exactly (k output still writes to w). Animate for a projection→group-action morph."),
    ],
    init_param_count: 2,
    wgsl_init: Some(r#"
fn init_jubiq4d(user: array<f32, 25>) -> array<f32, 2> {
    let power = user[0];
    let dist = user[1];
    let safe_power = select(power, 1e-30, abs(power) < 1e-30);
    var out: array<f32, 2>;
    out[0] = abs(power);                  // _absN
    out[1] = dist / safe_power * 0.5;     // _cN
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    // With z = w = 0 the full quaternion formulas reduce exactly to
    // jubiQ's 2D body.
    wgsl_2d: r#"
fn variation_jubiq4d(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let power = i32(get_param(xform_id, variation_id, 0u));
    if (power == 0) {
        return vec2<f32>(0.0, 0.0);
    }
    let a = get_param(xform_id, variation_id, 2u);
    let b = get_param(xform_id, variation_id, 3u);
    let c = get_param(xform_id, variation_id, 4u);
    let d_p = get_param(xform_id, variation_id, 5u);
    let ep = get_param(xform_id, variation_id, 6u);
    let fp = get_param(xform_id, variation_id, 7u);
    let qat = get_param(xform_id, variation_id, 8u);
    let qax = get_param(xform_id, variation_id, 9u);
    let qay = get_param(xform_id, variation_id, 10u);
    let qaz = get_param(xform_id, variation_id, 11u);
    let qbt = get_param(xform_id, variation_id, 12u);
    let qbx = get_param(xform_id, variation_id, 13u);
    let qby = get_param(xform_id, variation_id, 14u);
    let qbz = get_param(xform_id, variation_id, 15u);
    let qct = get_param(xform_id, variation_id, 16u);
    let qcx = get_param(xform_id, variation_id, 17u);
    let qcy = get_param(xform_id, variation_id, 18u);
    let qcz = get_param(xform_id, variation_id, 19u);
    let qdt = get_param(xform_id, variation_id, 20u);
    let qdx = get_param(xform_id, variation_id, 21u);
    let qdy = get_param(xform_id, variation_id, 22u);
    let qdz = get_param(xform_id, variation_id, 23u);
    let abs_n = get_param(xform_id, variation_id, 25u);
    let cn = get_param(xform_id, variation_id, 26u);
    let two_pi = 6.28318530717959;

    let t2 = p.x; let x2 = p.y;
    let nt = qat * t2 - qax * x2 + qbt;
    let nx = qat * x2 + qax * t2 + qbx;
    let ny = qay * t2 + qaz * x2 + qby;
    let nz = qaz * t2 - qay * x2 + qbz;
    let dt = qct * t2 - qcx * x2 + qdt;
    let dx = qct * x2 + qcx * t2 + qdx;
    let dy = qcy * t2 + qcz * x2 + qdy;
    let dz = qcz * t2 - qcy * x2 + qdz;
    let denom = dt * dt + dx * dx + dy * dy + dz * dz;
    let safe_denom = select(denom, 1e-30, denom < 1e-30);
    let ni = 1.0 / safe_denom;

    let x = a * p.x + b * p.y + (nt * dt + nx * dx + ny * dy + nz * dz) * ni + ep;
    let y = c * p.x + d_p * p.y + (nx * dt - nt * dx - ny * dz + nz * dy) * ni + fp;
    let abs_ni = max(i32(abs_n), 1);
    let pick = i32(rng_nextf(rng) * f32(abs_ni));
    let safe_pow = select(f32(power), 1e-30, abs(f32(power)) < 1e-30);
    let angle = (atan2(y, x) + two_pi * f32(pick)) / safe_pow;
    let r = pow(max(x * x + y * y, 1e-30), cn);
    return vec2<f32>(r * cos(angle), r * sin(angle));
}
"#,
    // Full 4D Möbius stage (q = (p.x, p.y, p.z, point_w), all product
    // terms, verified against reference quaternion algebra), then the
    // original julian2 chimera unchanged: angular/radial on the (t,i)
    // output components + affine wrap; z = (j) component + preserve-z
    // radial inversion; the (k) component — never computed by classic
    // jubiQ — feeds point_w_out.
    wgsl_3d: r#"
fn variation_jubiq4d(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let power = i32(get_param(xform_id, variation_id, 0u));
    if (power == 0) {
        point_w_out = point_w;
        return vec3<f32>(0.0, 0.0, p.z);
    }
    let a = get_param(xform_id, variation_id, 2u);
    let b = get_param(xform_id, variation_id, 3u);
    let c = get_param(xform_id, variation_id, 4u);
    let d_p = get_param(xform_id, variation_id, 5u);
    let ep = get_param(xform_id, variation_id, 6u);
    let fp = get_param(xform_id, variation_id, 7u);
    let qat = get_param(xform_id, variation_id, 8u);
    let qax = get_param(xform_id, variation_id, 9u);
    let qay = get_param(xform_id, variation_id, 10u);
    let qaz = get_param(xform_id, variation_id, 11u);
    let qbt = get_param(xform_id, variation_id, 12u);
    let qbx = get_param(xform_id, variation_id, 13u);
    let qby = get_param(xform_id, variation_id, 14u);
    let qbz = get_param(xform_id, variation_id, 15u);
    let qct = get_param(xform_id, variation_id, 16u);
    let qcx = get_param(xform_id, variation_id, 17u);
    let qcy = get_param(xform_id, variation_id, 18u);
    let qcz = get_param(xform_id, variation_id, 19u);
    let qdt = get_param(xform_id, variation_id, 20u);
    let qdx = get_param(xform_id, variation_id, 21u);
    let qdy = get_param(xform_id, variation_id, 22u);
    let qdz = get_param(xform_id, variation_id, 23u);
    let abs_n = get_param(xform_id, variation_id, 25u);
    let cn = get_param(xform_id, variation_id, 26u);
    let two_pi = 6.28318530717959;

    let k_in = get_param(xform_id, variation_id, 24u);
    let t2 = p.x; let x2 = p.y; let y2 = p.z; let z2 = point_w * k_in;
    let nt = qat * t2 - qax * x2 - qay * y2 - qaz * z2 + qbt;
    let nx = qat * x2 + qax * t2 + qay * z2 - qaz * y2 + qbx;
    let ny = qat * y2 - qax * z2 + qay * t2 + qaz * x2 + qby;
    let nz = qat * z2 + qax * y2 - qay * x2 + qaz * t2 + qbz;
    let dt = qct * t2 - qcx * x2 - qcy * y2 - qcz * z2 + qdt;
    let dx = qct * x2 + qcx * t2 + qcy * z2 - qcz * y2 + qdx;
    let dy = qct * y2 - qcx * z2 + qcy * t2 + qcz * x2 + qdy;
    let dz = qct * z2 + qcx * y2 - qcy * x2 + qcz * t2 + qdz;
    let denom = dt * dt + dx * dx + dy * dy + dz * dz;
    let safe_denom = select(denom, 1e-30, denom < 1e-30);
    let ni = 1.0 / safe_denom;

    let x = a * p.x + b * p.y + (nt * dt + nx * dx + ny * dy + nz * dz) * ni + ep;
    let y = c * p.x + d_p * p.y + (nx * dt - nt * dx - ny * dz + nz * dy) * ni + fp;
    let abs_ni = max(i32(abs_n), 1);
    let pick = i32(rng_nextf(rng) * f32(abs_ni));
    let safe_pow = select(f32(power), 1e-30, abs(f32(power)) < 1e-30);
    let angle = (atan2(y, x) + two_pi * f32(pick)) / safe_pow;
    let r = pow(max(x * x + y * y, 1e-30), cn);

    let z_half = p.z * 0.5;
    let r2d = p.x * p.x + p.y * p.y;
    let r3d = sqrt(r2d + z_half * z_half);
    let safe_r3d = max(r3d, 1e-30);
    let r2_inv = 1.0 / (sqrt(safe_r3d) * safe_r3d);
    let nz_term = (ny * dt - nt * dy - nz * dx + nx * dz) * ni;

    point_w_out = (nz * dt - nt * dz + ny * dx - nx * dy) * ni;
    return vec3<f32>(r * cos(angle), r * sin(angle), nz_term + r2_inv * z_half);
}
"#,
};
