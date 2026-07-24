//! `mobiq4d` — full 4D quaternionic Möbius (original; corrects
//! `mobiq`'s dropped dimension).
//!
//! The classic `mobiq` (zephyrtronium / JWildfire) computes the
//! quaternionic Möbius map `q ↦ (A·q + B)·(C·q + D)⁻¹` with the input
//! quaternion `q = x + y·i + z·j + 0·k` — and then DROPS the output
//! k-component, which is nonzero for general quaternion coefficients
//! (the k = 0 slice of ℍ is not invariant). Iterating that projection
//! is not iterating a group action.
//!
//! This variation carries the k-component honestly in the per-thread
//! 4th coordinate (`Feature::NeedsW`): input `q = (p.x, p.y, p.z,
//! point_w)`, all four output components kept (`point_w_out` gets the
//! k-part). With the full 4D state the map IS the fractional-linear
//! action of GL(2,ℍ) on the quaternionic projective line ℍP¹ ≅ S⁴ —
//! composition corresponds to matrix product (verified numerically),
//! so iteration walks a genuine Möbius group of S⁴ = ∂H⁵, the exact
//! quaternionic analogue of PSL(2,ℂ) on the Riemann sphere. Restricted
//! to point_w = 0 input, the xyz output equals classic `mobiq` exactly;
//! the difference is that the k-component feeds back instead of
//! vanishing.
//!
//! The 2D body is identical to `mobiq`'s (the w register is 3D-only,
//! and with z = w = 0 the formulas coincide). Parameter set and
//! component conventions (flame x = quaternion scalar t) match `mobiq`
//! so settings can be copied across.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Honest 4D quaternionic Möbius (GL(2,ℍ) on S⁴) — carries the
/// k-component classic mobiq drops.
///
/// # Authors
/// - zephyrtronium
/// - Fractals for All
/// - Claude Fable 5
pub static MOBIQ4D: VariationDef = VariationDef {
    name: "mobiq4d",
    aliases: &[],
    display_name: "Mobiq 4D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsW, Feature::AlwaysZ],
    parameters: &[
        param!("qat", "qa.t", unlimited_float, 1.0, -10.0, 10.0, "Scalar (t) component of quaternion A (numerator multiplier). Conventions match mobiq: flame x = quaternion t, y = i, z = j, and the per-thread w register = k (the component mobiq drops)."),
        param!("qax", "qa.x", unlimited_float, 0.0, -10.0, 10.0, "i component of quaternion A."),
        param!("qay", "qa.y", unlimited_float, 0.0, -10.0, 10.0, "j component of quaternion A."),
        param!("qaz", "qa.z", unlimited_float, 0.0, -10.0, 10.0, "k component of quaternion A."),
        param!("qbt", "qb.t", unlimited_float, 0.0, -10.0, 10.0, "Scalar (t) component of quaternion B (numerator offset)."),
        param!("qbx", "qb.x", unlimited_float, 0.0, -10.0, 10.0, "i component of quaternion B."),
        param!("qby", "qb.y", unlimited_float, 0.0, -10.0, 10.0, "j component of quaternion B."),
        param!("qbz", "qb.z", unlimited_float, 0.0, -10.0, 10.0, "k component of quaternion B."),
        param!("qct", "qc.t", unlimited_float, 0.0, -10.0, 10.0, "Scalar (t) component of quaternion C (denominator multiplier)."),
        param!("qcx", "qc.x", unlimited_float, 0.0, -10.0, 10.0, "i component of quaternion C."),
        param!("qcy", "qc.y", unlimited_float, 0.0, -10.0, 10.0, "j component of quaternion C."),
        param!("qcz", "qc.z", unlimited_float, 0.0, -10.0, 10.0, "k component of quaternion C."),
        param!("qdt", "qd.t", unlimited_float, 1.0, -10.0, 10.0, "Scalar (t) component of quaternion D (denominator offset)."),
        param!("qdx", "qd.x", unlimited_float, 0.0, -10.0, 10.0, "i component of quaternion D."),
        param!("qdy", "qd.y", unlimited_float, 0.0, -10.0, 10.0, "j component of quaternion D."),
        param!("qdz", "qd.z", unlimited_float, 0.0, -10.0, 10.0, "k component of quaternion D."),
        param!("k_input", "K Input", float, 1.0, 0.0, 1.0, "How much of the per-thread w register feeds the quaternion's k input. 1 = the honest 4D group action; 0 = the xyz output reproduces classic mobiq EXACTLY (the k output still writes to w). Animating 0→1 morphs continuously from the classic projection's fuzz to the group action's crisp limit sets."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    // With z = w = 0 the full quaternion formulas reduce exactly to
    // mobiq's 2D body, so this is byte-for-byte that math.
    wgsl_2d: r#"
fn variation_mobiq4d(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
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

    let nt = qat * tx - qax * xx + qbt;
    let nx = qat * xx + qax * tx + qbx;
    let ny = qat * 0.0 + qay * tx + qaz * xx + qby;
    let nz = qaz * tx - qay * xx + qbz;
    let dt = qct * tx - qcx * xx + qdt;
    let dx = qct * xx + qcx * tx + qdx;
    let dy = qcy * tx + qcz * xx + qdy;
    let dz = qcz * tx - qcy * xx + qdz;
    let denom = max(dt * dt + dx * dx + dy * dy + dz * dz, 1e-30);
    let ni = 1.0 / denom;

    return vec2<f32>(
        (nt * dt + nx * dx + ny * dy + nz * dz) * ni,
        (nx * dt - nt * dx - ny * dz + nz * dy) * ni,
    );
}
"#,
    // Full 4D: q = (p.x, p.y, p.z, point_w), left products A·q + B and
    // C·q + D with ALL terms (the qa?·z terms mobiq omits because its
    // k is pinned to 0), output n·conj(d)/|d|² with all four components
    // kept — k feeds point_w_out. Formulas verified against reference
    // quaternion algebra; composition = matrix product.
    wgsl_3d: r#"
fn variation_mobiq4d(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
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
    let k_in = get_param(xform_id, variation_id, 16u);
    let zz = point_w * k_in;

    let nt = qat * tx - qax * xx - qay * yy - qaz * zz + qbt;
    let nx = qat * xx + qax * tx + qay * zz - qaz * yy + qbx;
    let ny = qat * yy - qax * zz + qay * tx + qaz * xx + qby;
    let nz = qat * zz + qax * yy - qay * xx + qaz * tx + qbz;
    let dt = qct * tx - qcx * xx - qcy * yy - qcz * zz + qdt;
    let dx = qct * xx + qcx * tx + qcy * zz - qcz * yy + qdx;
    let dy = qct * yy - qcx * zz + qcy * tx + qcz * xx + qdy;
    let dz = qct * zz + qcx * yy - qcy * xx + qcz * tx + qdz;
    let denom = max(dt * dt + dx * dx + dy * dy + dz * dz, 1e-30);
    let ni = 1.0 / denom;

    point_w_out = (nz * dt - nt * dz + ny * dx - nx * dy) * ni;
    return vec3<f32>(
        (nt * dt + nx * dx + ny * dy + nz * dz) * ni,
        (nx * dt - nt * dx - ny * dz + nz * dy) * ni,
        (ny * dt - nt * dy - nz * dx + nx * dz) * ni,
    );
}
"#,
};
