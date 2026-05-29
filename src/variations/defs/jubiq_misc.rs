//! jubiq (Brad Stefanov)
//!
//! A mix of julian2 (Xyrus02) and Mobiq (zephyrtronium): quaternion
//! Mobius transformation in 4D, then Julia-style angular sampling +
//! radial scaling, plus an extra preserve-z radial inversion term.
//!
//! 24 user params:
//!   - power (int), dist
//!   - a, b, c, d, e, f (julian2-style 2D affine wrapper)
//!   - qat, qax, qay, qaz (Mobiq numerator quaternion A)
//!   - qbt, qbx, qby, qbz (Mobiq numerator quaternion B)
//!   - qct, qcx, qcy, qcz (Mobiq denominator quaternion C)
//!   - qdt, qdx, qdy, qdz (Mobiq denominator quaternion D)
//! + 2 init slots:
//!   - _absN = |power|
//!   - _cN = dist / power · 0.5
//!
//! 26 slots total — was over the old 16-slot ceiling. Port enabled
//! by the packed-variation-params buffer.
//!
//! Body factors cleanly through outer multiplier (cpp's `ni = VVAR /
//! denom` and `r = VVAR · pow(...)` and `r2 = VVAR / ...` all carry
//! the factor through). RNG (1 call/iter for the angular Julia
//! branch).
//!
//! Source: `output/jwildfire-vars/output/jubiq.cpp`.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Hybrid of julian2 and Mobiq — first applies a quaternion Möbius
/// transform `M(q) = (A·q + B) / (C·q + D)` in 4D (with `q = (FTx, FTy,
/// FTz, 0)`), then a `julian`-style angular pick (one of `|power|` equally-
/// spaced angles, scaled by `dist`) and radial scaling, then wraps the XY
/// output with an affine (`a..f`). In 3D, the Z output adds the quaternion
/// Z component to a preserve-Z radial inversion term derived from `1 /
/// (r3d^1.5)`. Mathematically a chimera; visually a way to combine the
/// quaternion-Möbius distortion of `mobiq` with the radial-symmetry blow-up
/// of `julian2`.
///
/// # Authors
/// - Brad Stefanov
pub static JUBIQ: VariationDef = VariationDef {
    name: "jubiQ",
    aliases: &[],
    display_name: "Jubiq",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("power", "Power", int, 1.0, -50.0, 50.0, "Julia angular branch count (integer). 0 collapses the variation to the origin. Otherwise: picks one of `|power|` equally-spaced angles per iteration and divides the polar angle by `power`."),
        param!("dist", "Distance", unlimited_float, 1.0, -10.0, 10.0, "Radial exponent multiplier. Combines with `power` to set the radial power as `0.5 · dist / power` (negative `dist` flips the radial sense)."),
        param!("a", "A", unlimited_float, 1.0, -10.0, 10.0, "Output XY affine wrapper, component A (X-to-X scale). Applied to the input `(p.x, p.y)` and added to the Möbius output."),
        param!("b", "B", unlimited_float, 0.0, -10.0, 10.0, "Output XY affine wrapper, component B (Y-to-X scale)."),
        param!("c", "C", unlimited_float, 0.0, -10.0, 10.0, "Output XY affine wrapper, component C (X-to-Y scale)."),
        param!("d", "D", unlimited_float, 1.0, -10.0, 10.0, "Output XY affine wrapper, component D (Y-to-Y scale)."),
        param!("e", "E", unlimited_float, 0.0, -10.0, 10.0, "Output XY affine wrapper, component E (X translation)."),
        param!("f", "F", unlimited_float, 0.0, -10.0, 10.0, "Output XY affine wrapper, component F (Y translation)."),
        param!("qat", "QA t", unlimited_float, 0.0, -10.0, 10.0, "Möbius numerator quaternion A (`M(q) = (A·q + B) / (C·q + D)`), scalar (t) component."),
        param!("qax", "QA x", unlimited_float, 0.0, -10.0, 10.0, "Möbius numerator quaternion A (`M(q) = (A·q + B) / (C·q + D)`), X component."),
        param!("qay", "QA y", unlimited_float, 0.0, -10.0, 10.0, "Möbius numerator quaternion A (`M(q) = (A·q + B) / (C·q + D)`), Y component."),
        param!("qaz", "QA z", unlimited_float, 0.0, -10.0, 10.0, "Möbius numerator quaternion A (`M(q) = (A·q + B) / (C·q + D)`), Z component."),
        param!("qbt", "QB t", unlimited_float, 0.0, -10.0, 10.0, "Möbius numerator quaternion B, scalar (t) component."),
        param!("qbx", "QB x", unlimited_float, 0.0, -10.0, 10.0, "Möbius numerator quaternion B, X component."),
        param!("qby", "QB y", unlimited_float, 0.0, -10.0, 10.0, "Möbius numerator quaternion B, Y component."),
        param!("qbz", "QB z", unlimited_float, 0.0, -10.0, 10.0, "Möbius numerator quaternion B, Z component."),
        param!("qct", "QC t", unlimited_float, 0.0, -10.0, 10.0, "Möbius denominator quaternion C, scalar (t) component."),
        param!("qcx", "QC x", unlimited_float, 0.0, -10.0, 10.0, "Möbius denominator quaternion C, X component."),
        param!("qcy", "QC y", unlimited_float, 0.0, -10.0, 10.0, "Möbius denominator quaternion C, Y component."),
        param!("qcz", "QC z", unlimited_float, 0.0, -10.0, 10.0, "Möbius denominator quaternion C, Z component."),
        param!("qdt", "QD t", unlimited_float, 1.0, -10.0, 10.0, "Möbius denominator quaternion D, scalar (t) component."),
        param!("qdx", "QD x", unlimited_float, 0.0, -10.0, 10.0, "Möbius denominator quaternion D, X component."),
        param!("qdy", "QD y", unlimited_float, 0.0, -10.0, 10.0, "Möbius denominator quaternion D, Y component."),
        param!("qdz", "QD z", unlimited_float, 0.0, -10.0, 10.0, "Möbius denominator quaternion D, Z component."),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 2,
    wgsl_init: Some(r#"
fn init_jubiq(user: array<f32, 24>) -> array<f32, 2> {
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
    needs_accum: false,
    wgsl_2d: r#"
fn variation_jubiQ(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
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
    let abs_n = get_param(xform_id, variation_id, 24u);
    let cn = get_param(xform_id, variation_id, 25u);
    let two_pi = 6.28318530717959;

    // 2D Mobiq sub: treat z=0 in the quaternion math (z becomes the FTz=0 case).
    let t2 = p.x; let x2 = p.y; let y2 = 0.0;
    let nt = qat * t2 - qax * x2 - qay * y2 + qbt;
    let nx = qat * x2 + qax * t2 - qaz * y2 + qbx;
    let ny = qat * y2 + qay * t2 + qaz * x2 + qby;
    let nz = qaz * t2 + qax * y2 - qay * x2 + qbz;
    let dt = qct * t2 - qcx * x2 - qcy * y2 + qdt;
    let dx = qct * x2 + qcx * t2 - qcz * y2 + qdx;
    let dy = qct * y2 + qcy * t2 + qcz * x2 + qdy;
    let dz = qcz * t2 + qcx * y2 - qcy * x2 + qdz;
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
    wgsl_3d: Some(r#"
fn variation_jubiQ(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let power = i32(get_param(xform_id, variation_id, 0u));
    if (power == 0) {
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
    let abs_n = get_param(xform_id, variation_id, 24u);
    let cn = get_param(xform_id, variation_id, 25u);
    let two_pi = 6.28318530717959;

    let t2 = p.x; let x2 = p.y; let y2 = p.z;
    let nt = qat * t2 - qax * x2 - qay * y2 + qbt;
    let nx = qat * x2 + qax * t2 - qaz * y2 + qbx;
    let ny = qat * y2 + qay * t2 + qaz * x2 + qby;
    let nz = qaz * t2 + qax * y2 - qay * x2 + qbz;
    let dt = qct * t2 - qcx * x2 - qcy * y2 + qdt;
    let dx = qct * x2 + qcx * t2 - qcz * y2 + qdx;
    let dy = qct * y2 + qcy * t2 + qcz * x2 + qdy;
    let dz = qcz * t2 + qcx * y2 - qcy * x2 + qdz;
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

    // Z output: cpp combines the quaternion z component with a preserve-z
    // radial inversion. The "z = FTz / 2" line and the r2 inversion reduce
    // to: nz_term + (1 / (sqrt(r3d) * r3d)) * (FTz/2). Body factors cleanly.
    let z_half = p.z * 0.5;
    let r2d = p.x * p.x + p.y * p.y;
    let r3d = sqrt(r2d + z_half * z_half);
    let safe_r3d = max(r3d, 1e-30);
    let r2_inv = 1.0 / (sqrt(safe_r3d) * safe_r3d);
    let nz_term = (ny * dt - nt * dy - nz * dx + nx * dz) * ni;

    return vec3<f32>(r * cos(angle), r * sin(angle), nz_term + r2_inv * z_half);
}
"#),
};
