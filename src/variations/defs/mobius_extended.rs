//! Möbius family extensions
//!
//! Ports of two upstream Möbius variations beyond the standard 2D
//! `mobius` already in our registry:
//!
//!   - `mobiusN` (eralex61, fixed by thargor6) — N-power Möbius:
//!     transform into z^power space, apply 2×2 complex Möbius, transform
//!     back via a random branch of the N-th root.
//!   - `mobiq` (zephyrtronium / Brad Stefanov) — quaternion Möbius:
//!     same (Az + B)/(Cz + D) form but a, b, c, d, z are quaternions.
//!     Treats input (x, y, z) as quaternion x + y·i + z·j + 0·k, so this
//!     is a true 3D variation that warps all three coordinates.
//!
//! Sources:
//!   - output/jwildfire-vars/output/mobiusN.cpp
//!   - output/jwildfire-vars/output/mobiq.cpp
//!
//! Note: `prepost_mobius` from upstream is a JWildfire priority-2
//! variation that runs both BEFORE the affine (inverse Möbius on FTx/FTy)
//! and AFTER it (Möbius on FPx/FPy with assignment, not accumulation).
//! This pattern doesn't fit our pre/normal/post phase model — our pre and
//! post variations live in separate slots and our normal phase
//! accumulates rather than assigns. Skipped pending an architectural
//! decision; tracked on the watchlist.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// mobiusN: N-power Möbius
//   User params: re_a, re_b, re_c, re_d, im_a, im_b, im_c, im_d, power, dist
//   Body:
//     z_exp = 4·dist / power
//     r = (sqrt(x²+y²) + ε)^z_exp
//     α = atan2(y, x) · power
//     (x', y') = r · (cos α, sin α)
//     Möbius (re_*, im_* matrix) on (x', y') → (x'', y'')
//     z_inv = 1 / z_exp
//     r' = sqrt(x''² + y''²)^z_inv
//     n = floor(power · rand())   (random branch of N-th root)
//     α' = (atan2(y'', x'') + n·2π) / floor(power)
//     out = r' · (cos α', sin α')
//   Notes:
//     - Upstream init clamps |power| < 1.0 → 1.0; we inline that clamp
//       per-iteration (negligible cost).
//     - VVAR factors out cleanly through the outer multiplier.
// =============================================================================
/// N-power Möbius — transforms into `z^power` space, applies a 2×2 complex
/// Möbius transformation `(Az + B)/(Cz + D)`, then transforms back via a
/// random branch of the N-th root.
///
/// # Authors
/// - eralex61
pub static MOBIUSN: VariationDef = VariationDef {
    name: "mobiusN",
    aliases: &[],
    display_name: "MobiusN",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("re_a", "Re A", unlimited_float, 1.0, -10.0, 10.0, "Real part of complex coefficient A (numerator multiplier)."),
        param!("re_b", "Re B", unlimited_float, 0.0, -10.0, 10.0, "Real part of complex coefficient B (numerator offset)."),
        param!("re_c", "Re C", unlimited_float, 0.0, -10.0, 10.0, "Real part of complex coefficient C (denominator multiplier)."),
        param!("re_d", "Re D", unlimited_float, 1.0, -10.0, 10.0, "Real part of complex coefficient D (denominator offset)."),
        param!("im_a", "Im A", unlimited_float, 0.0, -10.0, 10.0, "Imaginary part of complex coefficient A (numerator multiplier)."),
        param!("im_b", "Im B", unlimited_float, 0.0, -10.0, 10.0, "Imaginary part of complex coefficient B (numerator offset)."),
        param!("im_c", "Im C", unlimited_float, 0.0, -10.0, 10.0, "Imaginary part of complex coefficient C (denominator multiplier)."),
        param!("im_d", "Im D", unlimited_float, 0.0, -10.0, 10.0, "Imaginary part of complex coefficient D (denominator offset)."),
        param!("power", "Power", unlimited_float, 1.0, -10.0, 10.0, "Exponent for the `z^power` transform that wraps the Möbius operation. Higher values create more arms in the output."),
        param!("dist", "Distance", unlimited_float, 1.0, -10.0, 10.0, "Scales the radial component of the wrapping transform."),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_mobiusN(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let re_a = get_param(xform_id, variation_id, 0u);
    let re_b = get_param(xform_id, variation_id, 1u);
    let re_c = get_param(xform_id, variation_id, 2u);
    let re_d = get_param(xform_id, variation_id, 3u);
    let im_a = get_param(xform_id, variation_id, 4u);
    let im_b = get_param(xform_id, variation_id, 5u);
    let im_c = get_param(xform_id, variation_id, 6u);
    let im_d = get_param(xform_id, variation_id, 7u);
    let power_raw = get_param(xform_id, variation_id, 8u);
    let dist = get_param(xform_id, variation_id, 9u);

    var power = power_raw;
    if (abs(power) < 1.0) { power = 1.0; }

    let z_exp = 4.0 * dist / power;
    let r0 = sqrt(p.x * p.x + p.y * p.y) + 1e-6;
    let r1 = pow(r0, z_exp);
    let alpha = atan2(p.y, p.x) * power;
    let x1 = r1 * cos(alpha);
    let y1 = r1 * sin(alpha);

    let real_u = re_a * x1 - im_a * y1 + re_b;
    let imag_u = re_a * y1 + im_a * x1 + im_b;
    let real_v = re_c * x1 - im_c * y1 + re_d;
    let imag_v = re_c * y1 + im_c * x1 + im_d;
    let rad_v = max(real_v * real_v + imag_v * imag_v, 1e-30);

    let x2 = (real_u * real_v + imag_u * imag_v) / rad_v;
    let y2 = (imag_u * real_v - real_u * imag_v) / rad_v;

    let z_inv = 1.0 / z_exp;
    let r2 = pow(sqrt(x2 * x2 + y2 * y2), z_inv);
    let fp = floor(power);
    let safe_fp = select(fp, 1.0, fp == 0.0);
    let n = floor(power * rng_nextf(rng));
    let alpha2 = (atan2(y2, x2) + n * 6.28318530717959) / safe_fp;

    return vec2<f32>(r2 * cos(alpha2), r2 * sin(alpha2));
}
"#,
    wgsl_3d: Some(r#"
fn variation_mobiusN(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let re_a = get_param(xform_id, variation_id, 0u);
    let re_b = get_param(xform_id, variation_id, 1u);
    let re_c = get_param(xform_id, variation_id, 2u);
    let re_d = get_param(xform_id, variation_id, 3u);
    let im_a = get_param(xform_id, variation_id, 4u);
    let im_b = get_param(xform_id, variation_id, 5u);
    let im_c = get_param(xform_id, variation_id, 6u);
    let im_d = get_param(xform_id, variation_id, 7u);
    let power_raw = get_param(xform_id, variation_id, 8u);
    let dist = get_param(xform_id, variation_id, 9u);

    var power = power_raw;
    if (abs(power) < 1.0) { power = 1.0; }

    let z_exp = 4.0 * dist / power;
    let r0 = sqrt(p.x * p.x + p.y * p.y) + 1e-6;
    let r1 = pow(r0, z_exp);
    let alpha = atan2(p.y, p.x) * power;
    let x1 = r1 * cos(alpha);
    let y1 = r1 * sin(alpha);

    let real_u = re_a * x1 - im_a * y1 + re_b;
    let imag_u = re_a * y1 + im_a * x1 + im_b;
    let real_v = re_c * x1 - im_c * y1 + re_d;
    let imag_v = re_c * y1 + im_c * x1 + im_d;
    let rad_v = max(real_v * real_v + imag_v * imag_v, 1e-30);

    let x2 = (real_u * real_v + imag_u * imag_v) / rad_v;
    let y2 = (imag_u * real_v - real_u * imag_v) / rad_v;

    let z_inv = 1.0 / z_exp;
    let r2 = pow(sqrt(x2 * x2 + y2 * y2), z_inv);
    let fp = floor(power);
    let safe_fp = select(fp, 1.0, fp == 0.0);
    let n = floor(power * rng_nextf(rng));
    let alpha2 = (atan2(y2, x2) + n * 6.28318530717959) / safe_fp;

    return vec3<f32>(r2 * cos(alpha2), r2 * sin(alpha2), p.z);
}
"#),
};

// =============================================================================
// mobiq: quaternion Möbius (zephyrtronium)
//   User params: 16 quaternion components — q{a,b,c,d} × {t,x,y,z}
//     a = qat + qax·i + qay·j + qaz·k     (numerator multiplier)
//     b = qbt + qbx·i + qby·j + qbz·k     (numerator offset)
//     c = qct + qcx·i + qcy·j + qcz·k     (denominator multiplier)
//     d = qdt + qdx·i + qdy·j + qdz·k     (denominator offset)
//   Input quaternion: x = FTx + FTy·i + FTz·j + 0·k  (k-part is zero)
//   Output: (a·x + b) / (c·x + d) via right-division by quaternion norm.
//
//   Note: 16 user params is the maximum our buffer slot count allows,
//   leaving zero room for derived init values. The body therefore inlines
//   all the arithmetic — no init step needed.
// =============================================================================
/// Quaternion Möbius — same `(Az + B)/(Cz + D)` form as Möbius but with
/// quaternion-valued A, B, C, D and input. Treats `(x, y, z)` as a
/// quaternion with no k-component, producing true 3D output.
///
/// # Authors
/// - zephyrtronium
pub static MOBIQ: VariationDef = VariationDef {
    name: "mobiq",
    aliases: &[],
    display_name: "Mobiq",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("qat", "qa.t", unlimited_float, 1.0, -10.0, 10.0, "T (scalar) component of quaternion A (numerator multiplier)."),
        param!("qax", "qa.x", unlimited_float, 0.0, -10.0, 10.0, "X (i) component of quaternion A (numerator multiplier)."),
        param!("qay", "qa.y", unlimited_float, 0.0, -10.0, 10.0, "Y (j) component of quaternion A (numerator multiplier)."),
        param!("qaz", "qa.z", unlimited_float, 0.0, -10.0, 10.0, "Z (k) component of quaternion A (numerator multiplier)."),
        param!("qbt", "qb.t", unlimited_float, 0.0, -10.0, 10.0, "T (scalar) component of quaternion B (numerator offset)."),
        param!("qbx", "qb.x", unlimited_float, 0.0, -10.0, 10.0, "X (i) component of quaternion B (numerator offset)."),
        param!("qby", "qb.y", unlimited_float, 0.0, -10.0, 10.0, "Y (j) component of quaternion B (numerator offset)."),
        param!("qbz", "qb.z", unlimited_float, 0.0, -10.0, 10.0, "Z (k) component of quaternion B (numerator offset)."),
        param!("qct", "qc.t", unlimited_float, 0.0, -10.0, 10.0, "T (scalar) component of quaternion C (denominator multiplier)."),
        param!("qcx", "qc.x", unlimited_float, 0.0, -10.0, 10.0, "X (i) component of quaternion C (denominator multiplier)."),
        param!("qcy", "qc.y", unlimited_float, 0.0, -10.0, 10.0, "Y (j) component of quaternion C (denominator multiplier)."),
        param!("qcz", "qc.z", unlimited_float, 0.0, -10.0, 10.0, "Z (k) component of quaternion C (denominator multiplier)."),
        param!("qdt", "qd.t", unlimited_float, 1.0, -10.0, 10.0, "T (scalar) component of quaternion D (denominator offset)."),
        param!("qdx", "qd.x", unlimited_float, 0.0, -10.0, 10.0, "X (i) component of quaternion D (denominator offset)."),
        param!("qdy", "qd.y", unlimited_float, 0.0, -10.0, 10.0, "Y (j) component of quaternion D (denominator offset)."),
        param!("qdz", "qd.z", unlimited_float, 0.0, -10.0, 10.0, "Z (k) component of quaternion D (denominator offset)."),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    // 2D form: FTz = 0 substituted into the full quaternion math, returning
    // only (FPx, FPy). Some output components (ny, nz) remain non-zero from
    // q*y and q*z parameter columns, so we still compute the whole thing.
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_mobiq(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let qat = get_param(xform_id, variation_id, 0u);
    let qax = get_param(xform_id, variation_id, 1u);
    let qay = get_param(xform_id, variation_id, 2u);
    let qaz = get_param(xform_id, variation_id, 3u);
    let qbt = get_param(xform_id, variation_id, 4u);
    let qbx = get_param(xform_id, variation_id, 5u);
    let qby = get_param(xform_id, variation_id, 6u);
    let qbz = get_param(xform_id, variation_id, 7u);
    let qct = get_param(xform_id, variation_id, 8u);
    let qcx = get_param(xform_id, variation_id, 9u);
    let qcy = get_param(xform_id, variation_id, 10u);
    let qcz = get_param(xform_id, variation_id, 11u);
    let qdt = get_param(xform_id, variation_id, 12u);
    let qdx = get_param(xform_id, variation_id, 13u);
    let qdy = get_param(xform_id, variation_id, 14u);
    let qdz = get_param(xform_id, variation_id, 15u);

    let tx = p.x;
    let xx = p.y;
    let yy = 0.0;  // FTz substituted as 0 in 2D mode

    let nt = qat * tx - qax * xx - qay * yy + qbt;
    let nx = qat * xx + qax * tx - qaz * yy + qbx;
    let ny = qat * yy + qay * tx + qaz * xx + qby;
    let nz = qaz * tx + qax * yy - qay * xx + qbz;
    let dt = qct * tx - qcx * xx - qcy * yy + qdt;
    let dx = qct * xx + qcx * tx - qcz * yy + qdx;
    let dy = qct * yy + qcy * tx + qcz * xx + qdy;
    let dz = qcz * tx + qcx * yy - qcy * xx + qdz;
    let denom = max(dt * dt + dx * dx + dy * dy + dz * dz, 1e-30);
    let ni = 1.0 / denom;

    return vec2<f32>(
        (nt * dt + nx * dx + ny * dy + nz * dz) * ni,
        (nx * dt - nt * dx - ny * dz + nz * dy) * ni,
    );
}
"#,
    wgsl_3d: Some(r#"
fn variation_mobiq(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let qat = get_param(xform_id, variation_id, 0u);
    let qax = get_param(xform_id, variation_id, 1u);
    let qay = get_param(xform_id, variation_id, 2u);
    let qaz = get_param(xform_id, variation_id, 3u);
    let qbt = get_param(xform_id, variation_id, 4u);
    let qbx = get_param(xform_id, variation_id, 5u);
    let qby = get_param(xform_id, variation_id, 6u);
    let qbz = get_param(xform_id, variation_id, 7u);
    let qct = get_param(xform_id, variation_id, 8u);
    let qcx = get_param(xform_id, variation_id, 9u);
    let qcy = get_param(xform_id, variation_id, 10u);
    let qcz = get_param(xform_id, variation_id, 11u);
    let qdt = get_param(xform_id, variation_id, 12u);
    let qdx = get_param(xform_id, variation_id, 13u);
    let qdy = get_param(xform_id, variation_id, 14u);
    let qdz = get_param(xform_id, variation_id, 15u);

    let tx = p.x;
    let xx = p.y;
    let yy = p.z;

    let nt = qat * tx - qax * xx - qay * yy + qbt;
    let nx = qat * xx + qax * tx - qaz * yy + qbx;
    let ny = qat * yy + qay * tx + qaz * xx + qby;
    let nz = qaz * tx + qax * yy - qay * xx + qbz;
    let dt = qct * tx - qcx * xx - qcy * yy + qdt;
    let dx = qct * xx + qcx * tx - qcz * yy + qdx;
    let dy = qct * yy + qcy * tx + qcz * xx + qdy;
    let dz = qcz * tx + qcx * yy - qcy * xx + qdz;
    let denom = max(dt * dt + dx * dx + dy * dy + dz * dz, 1e-30);
    let ni = 1.0 / denom;

    return vec3<f32>(
        (nt * dt + nx * dx + ny * dy + nz * dz) * ni,
        (nx * dt - nt * dx - ny * dz + nz * dy) * ni,
        (ny * dt - nt * dy - nz * dx + nx * dz) * ni,
    );
}
"#),
};
