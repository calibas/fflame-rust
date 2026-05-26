//! supershape3d (David Young)
//!
//! Two-axis Gielis super-shape mapped to a 3D surface, with optional
//! spiral phase advance and toroidal vs spherical projection.
//!
//! 16 user params (rho, phi, m1, m2, a1, a2, b1, b2, n1_1, n1_2,
//! n2_1, n2_2, n3_1, n3_2, spiral, toroidmap int) +
//! 10 init slots (_n1n_1, _n1n_2, _m4_1, _m4_2, _an2_1, _an2_2,
//! _bn3_1, _bn3_2, _rho_pi, _phi_pi).
//!
//! 26 slots total — over the old 16 ceiling. Port enabled by the
//! packed-variation-params buffer.
//!
//! Init formulas (cpp uses M_2_PI = 2/π):
//!   _n1n_k = -1 / n1_k                              (k = 1, 2)
//!   _an2_k = (1/|a_k|)^n2_k                          (k = 1, 2)
//!   _bn3_k = (1/|b_k|)^n3_k                          (k = 1, 2)
//!   _m4_k  = m_k / 4                                 (k = 1, 2)
//!   _rho_pi = rho · 2/π
//!   _phi_pi = phi · 2/π
//!
//! Body uses RNG (3 calls/iter for rho1, phi1, sign-of-phi1) and
//! factors cleanly through outer multiplier. Full3D.
//!
//! Source: `output/jwildfire-vars/output/supershape3d.cpp`.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Two-axis Gielis super-shape projected onto a 3D surface — each iteration
/// picks two random angles `ρ` and `φ` (uniform in `[0, rho]` and `[-phi,
/// phi]`) and evaluates the Gielis super-shape formula `r(θ) =
/// (|cos(m·θ/4)/a|^n2 + |sin(m·θ/4)/b|^n3)^(-1/n1)` independently for each
/// angle (subscript 1 for the ρ axis, 2 for the φ axis). The two radii `r1`
/// and `r2` then combine into either a spherical product (`r1·r2·cos·cos`,
/// etc.) or a toroidal sum (`cos·(r1 + r2·cos)`, etc.) based on
/// `toroidmap`. Optional `spiral` term advances the ρ-axis radius linearly
/// with the ρ angle to produce a spiraling output.
///
/// # Authors
/// - David Young
pub static SUPERSHAPE_3D: VariationDef = VariationDef {
    name: "superShape3d",
    display_name: "SuperShape 3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("rho", "Rho", unlimited_float, 9.9, -100.0, 100.0, "Angular range (radians) for uniform random sampling of the ρ axis. Larger values sweep through more of the shape per iteration; default 9.9 (~3π)."),
        param!("phi", "Phi", unlimited_float, 2.5, -100.0, 100.0, "Angular range (radians) for the φ axis (sampled in `[-phi, phi]`). Default 2.5 (~0.8π)."),
        param!("m1", "M1", unlimited_float, 6.0, -50.0, 50.0, "Petal count for the ρ-axis Gielis shape — controls rotational symmetry. Higher values produce more spokes."),
        param!("m2", "M2", unlimited_float, 3.0, -50.0, 50.0, "Petal count for the φ-axis Gielis shape."),
        param!("a1", "A1", unlimited_float, 1.0, -10.0, 10.0, "Cosine-term width scaling for the ρ-axis shape. Smaller values pinch the cos-side petals narrower."),
        param!("a2", "A2", unlimited_float, 1.0, -10.0, 10.0, "Cosine-term width scaling for the φ-axis shape."),
        param!("b1", "B1", unlimited_float, 1.0, -10.0, 10.0, "Sine-term width scaling for the ρ-axis shape."),
        param!("b2", "B2", unlimited_float, 1.0, -10.0, 10.0, "Sine-term width scaling for the φ-axis shape."),
        param!("n1_1", "N1.1", unlimited_float, 1.0, -10.0, 10.0, "Outer exponent for the ρ-axis shape: final radius is `pr^(-1/n1_1)`. Controls overall radial blow-up — small values produce sharp spikes, larger values rounder shapes."),
        param!("n1_2", "N1.2", unlimited_float, 1.0, -10.0, 10.0, "Outer exponent for the φ-axis shape."),
        param!("n2_1", "N2.1", unlimited_float, 1.0, -10.0, 10.0, "Cosine-term exponent for the ρ-axis shape — controls petal sharpness on the cos side."),
        param!("n2_2", "N2.2", unlimited_float, 1.0, -10.0, 10.0, "Cosine-term exponent for the φ-axis shape."),
        param!("n3_1", "N3.1", unlimited_float, 1.0, -10.0, 10.0, "Sine-term exponent for the ρ-axis shape — controls petal sharpness on the sin side."),
        param!("n3_2", "N3.2", unlimited_float, 1.0, -10.0, 10.0, "Sine-term exponent for the φ-axis shape."),
        param!("spiral", "Spiral", unlimited_float, 0.0, -10.0, 10.0, "Spiral phase advance: `r1` is incremented by `spiral · ρ`, producing a linear radial growth with angle. 0 disables."),
        param!("toroidmap", "Toroidal", int, 0.0, 0.0, 1.0, "0 = spherical projection (output = `r1·r2·cos·cos`, etc.). 1 = toroidal projection (output = `cos·(r1 + r2·cos)`, etc.). Both modes use `r2·sin(φ)` for Z."),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 10,
    wgsl_init: Some(r#"
fn init_superShape3d(user: array<f32, 16>) -> array<f32, 10> {
    let two_over_pi = 0.6366197723675814;  // 2/π
    let rho = user[0];
    let phi = user[1];
    let m1 = user[2];
    let m2 = user[3];
    let a1 = user[4];
    let a2 = user[5];
    let b1 = user[6];
    let b2 = user[7];
    let n1_1 = user[8];
    let n1_2 = user[9];
    let n2_1 = user[10];
    let n2_2 = user[11];
    let n3_1 = user[12];
    let n3_2 = user[13];
    var out: array<f32, 10>;
    out[0] = -1.0 / select(n1_1, 1e-30, abs(n1_1) < 1e-30);  // _n1n_1
    out[1] = -1.0 / select(n1_2, 1e-30, abs(n1_2) < 1e-30);  // _n1n_2
    out[2] = m1 * 0.25;                                       // _m4_1
    out[3] = m2 * 0.25;                                       // _m4_2
    out[4] = pow(abs(1.0 / select(a1, 1e-30, abs(a1) < 1e-30)), n2_1);  // _an2_1
    out[5] = pow(abs(1.0 / select(a2, 1e-30, abs(a2) < 1e-30)), n2_2);  // _an2_2
    out[6] = pow(abs(1.0 / select(b1, 1e-30, abs(b1) < 1e-30)), n3_1);  // _bn3_1
    out[7] = pow(abs(1.0 / select(b2, 1e-30, abs(b2) < 1e-30)), n3_2);  // _bn3_2
    out[8] = rho * two_over_pi;                               // _rho_pi
    out[9] = phi * two_over_pi;                               // _phi_pi
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_superShape3d(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let n2_1 = get_param(xform_id, variation_id, 10u);
    let n2_2 = get_param(xform_id, variation_id, 11u);
    let n3_1 = get_param(xform_id, variation_id, 12u);
    let n3_2 = get_param(xform_id, variation_id, 13u);
    let spiral = get_param(xform_id, variation_id, 14u);
    let toroidmap = i32(get_param(xform_id, variation_id, 15u));
    let n1n_1 = get_param(xform_id, variation_id, 16u);
    let n1n_2 = get_param(xform_id, variation_id, 17u);
    let m4_1 = get_param(xform_id, variation_id, 18u);
    let m4_2 = get_param(xform_id, variation_id, 19u);
    let an2_1 = get_param(xform_id, variation_id, 20u);
    let an2_2 = get_param(xform_id, variation_id, 21u);
    let bn3_1 = get_param(xform_id, variation_id, 22u);
    let bn3_2 = get_param(xform_id, variation_id, 23u);
    let rho_pi = get_param(xform_id, variation_id, 24u);
    let phi_pi = get_param(xform_id, variation_id, 25u);

    let rho1 = rng_nextf(rng) * rho_pi;
    var phi1 = rng_nextf(rng) * phi_pi;
    let sign_pick = rng_nextf(rng);
    if (sign_pick < 0.5) {
        phi1 = -phi1;
    }
    let sinr = sin(rho1);
    let cosr = cos(rho1);
    let sinp = sin(phi1);
    let cosp = cos(phi1);

    let a_r = m4_1 * rho1;
    let msinr = sin(a_r);
    let mcosr = cos(a_r);
    let a_p = m4_2 * phi1;
    let msinp = sin(a_p);
    let mcosp = cos(a_p);

    let pr1 = an2_1 * pow(max(abs(mcosr), 1e-30), n2_1) + bn3_1 * pow(max(abs(msinr), 1e-30), n3_1);
    let pr2 = an2_2 * pow(max(abs(mcosp), 1e-30), n2_2) + bn3_2 * pow(max(abs(msinp), 1e-30), n3_2);
    let r1 = pow(max(pr1, 1e-30), n1n_1) + spiral * rho1;
    let r2 = pow(max(pr2, 1e-30), n1n_2);

    if (toroidmap == 1) {
        return vec2<f32>(cosr * (r1 + r2 * cosp), sinr * (r1 + r2 * cosp));
    }
    return vec2<f32>(r1 * cosr * r2 * cosp, r1 * sinr * r2 * cosp);
}
"#,
    wgsl_3d: Some(r#"
fn variation_superShape3d(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let n2_1 = get_param(xform_id, variation_id, 10u);
    let n2_2 = get_param(xform_id, variation_id, 11u);
    let n3_1 = get_param(xform_id, variation_id, 12u);
    let n3_2 = get_param(xform_id, variation_id, 13u);
    let spiral = get_param(xform_id, variation_id, 14u);
    let toroidmap = i32(get_param(xform_id, variation_id, 15u));
    let n1n_1 = get_param(xform_id, variation_id, 16u);
    let n1n_2 = get_param(xform_id, variation_id, 17u);
    let m4_1 = get_param(xform_id, variation_id, 18u);
    let m4_2 = get_param(xform_id, variation_id, 19u);
    let an2_1 = get_param(xform_id, variation_id, 20u);
    let an2_2 = get_param(xform_id, variation_id, 21u);
    let bn3_1 = get_param(xform_id, variation_id, 22u);
    let bn3_2 = get_param(xform_id, variation_id, 23u);
    let rho_pi = get_param(xform_id, variation_id, 24u);
    let phi_pi = get_param(xform_id, variation_id, 25u);

    let rho1 = rng_nextf(rng) * rho_pi;
    var phi1 = rng_nextf(rng) * phi_pi;
    let sign_pick = rng_nextf(rng);
    if (sign_pick < 0.5) {
        phi1 = -phi1;
    }
    let sinr = sin(rho1);
    let cosr = cos(rho1);
    let sinp = sin(phi1);
    let cosp = cos(phi1);

    let a_r = m4_1 * rho1;
    let msinr = sin(a_r);
    let mcosr = cos(a_r);
    let a_p = m4_2 * phi1;
    let msinp = sin(a_p);
    let mcosp = cos(a_p);

    let pr1 = an2_1 * pow(max(abs(mcosr), 1e-30), n2_1) + bn3_1 * pow(max(abs(msinr), 1e-30), n3_1);
    let pr2 = an2_2 * pow(max(abs(mcosp), 1e-30), n2_2) + bn3_2 * pow(max(abs(msinp), 1e-30), n3_2);
    let r1 = pow(max(pr1, 1e-30), n1n_1) + spiral * rho1;
    let r2 = pow(max(pr2, 1e-30), n1n_2);

    if (toroidmap == 1) {
        return vec3<f32>(cosr * (r1 + r2 * cosp), sinr * (r1 + r2 * cosp), r2 * sinp);
    }
    return vec3<f32>(r1 * cosr * r2 * cosp, r1 * sinr * r2 * cosp, r2 * sinp);
}
"#),
};
