//! jac_asn (Dark-Beam)
//!
//! Inverse Jacobi `sn` (and cn/sc/dn variants) using Norbert Rosch's
//! incomplete elliptic integral of the first kind via Landen
//! transformations.
//!
//! 3 user params:
//!   - jac_asn_kr (real part of modulus)
//!   - jac_asn_ki (imag part of modulus)
//!   - jac_asn_type (int 0-7)
//!     - 0/4: invdn (elliptic F)
//!     - 1/5: inverse jacobi sn (default)
//!     - 2/6: inverse jacobi cn
//!     - 3/7: inverse jacobi sc
//!     types > 3 swap modulus and phi.
//!
//! No init slots. Uses needs_transform to read weight w; cpp's
//! `geo(VVAR)` initialization is replaced by `geo = (1, 0)` so the
//! body factors cleanly through the outer multiplier. The Z output
//! has needs_transform divide-out (cpp `FPz += FTz` without VVAR).
//!
//! Body uses a complex-arithmetic toolkit (`jac_cmul`, `jac_cdiv`,
//! `jac_cabs`, `jac_clog`, `jac_csqrt`, `jac_csin`, `jac_ccos`,
//! `jac_casin`, `jac_cacos`, `jac_casinh`) implemented inline. Names
//! prefixed with `jac_` to avoid collisions with other variations
//! sharing the dynamic shader.
//!
//! The Landen-transform iteration is bounded at 10 iters, with an
//! early-exit when `|1 - modul|² < ε`.
//!
//! Source: `output/jwildfire-vars/output/jac_asn.cpp`.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

pub static JAC_ASN: VariationDef = VariationDef {
    name: "jac_asn",
    display_name: "Jac asn",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("jac_asn_kr", "Modulus Real", unlimited_float, 0.5, -10.0, 10.0),
        param!("jac_asn_ki", "Modulus Imag", unlimited_float, 0.0, -10.0, 10.0),
        param!("jac_asn_type", "Type", int, 1.0, 0.0, 7.0),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn jac_cmul(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(a.x * b.x - a.y * b.y, a.x * b.y + a.y * b.x);
}
fn jac_cdiv(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    let d = b.x * b.x + b.y * b.y;
    let safe_d = select(d, 1e-30, d < 1e-30);
    return vec2<f32>((a.x * b.x + a.y * b.y) / safe_d, (a.y * b.x - a.x * b.y) / safe_d);
}
fn jac_cabs(z: vec2<f32>) -> f32 {
    return sqrt(z.x * z.x + z.y * z.y);
}
fn jac_clog(z: vec2<f32>) -> vec2<f32> {
    let r = jac_cabs(z);
    let safe_r = select(r, 1e-30, r < 1e-30);
    return vec2<f32>(log(safe_r), atan2(z.y, z.x));
}
fn jac_csqrt(z: vec2<f32>) -> vec2<f32> {
    let r = jac_cabs(z);
    let real_p = sqrt(max((r + z.x) * 0.5, 0.0));
    let imag_p = sqrt(max((r - z.x) * 0.5, 0.0));
    let s = select(1.0, -1.0, z.y < 0.0);
    return vec2<f32>(real_p, s * imag_p);
}
fn jac_csin(z: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(sin(z.x) * cosh(z.y), cos(z.x) * sinh(z.y));
}
fn jac_ccos(z: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(cos(z.x) * cosh(z.y), -sin(z.x) * sinh(z.y));
}
fn jac_casin(z: vec2<f32>) -> vec2<f32> {
    let z2 = jac_cmul(z, z);
    let one_minus_z2 = vec2<f32>(1.0 - z2.x, -z2.y);
    let sq = jac_csqrt(one_minus_z2);
    let iz = vec2<f32>(-z.y, z.x);
    let lg = jac_clog(iz + sq);
    return vec2<f32>(lg.y, -lg.x);
}
fn jac_cacos(z: vec2<f32>) -> vec2<f32> {
    let z2 = jac_cmul(z, z);
    let one_minus_z2 = vec2<f32>(1.0 - z2.x, -z2.y);
    let sq = jac_csqrt(one_minus_z2);
    let i_sq = vec2<f32>(-sq.y, sq.x);
    let lg = jac_clog(z + i_sq);
    return vec2<f32>(lg.y, -lg.x);
}
fn jac_casinh(z: vec2<f32>) -> vec2<f32> {
    let z2 = jac_cmul(z, z);
    let z2p1 = vec2<f32>(z2.x + 1.0, z2.y);
    let sq = jac_csqrt(z2p1);
    return jac_clog(z + sq);
}

fn jac_asn_inner(in_p: vec2<f32>, kr: f32, ki: f32, jtype: i32) -> vec2<f32> {
    let pi_4 = 0.7853981633974483;
    var phi = vec2<f32>(in_p.x, in_p.y);
    var modul = vec2<f32>(kr, ki);

    let case_idx = jtype - (jtype / 4) * 4;
    if (case_idx == 0) {
        let s = jac_csin(phi);
        let m_s = jac_cmul(modul, s);
        phi = 0.5 * (phi + jac_casin(m_s));
    } else if (case_idx == 2) {
        let z2 = jac_cmul(phi, phi);
        let one_minus_z2 = vec2<f32>(1.0 - z2.x, -z2.y);
        let sq = jac_csqrt(one_minus_z2);
        let m_sq = jac_cmul(modul, sq);
        phi = 0.5 * (jac_cacos(phi) + jac_casin(m_sq));
    } else if (case_idx == 3) {
        let i_asinh = jac_casinh(phi);
        let i_asinh_rot = vec2<f32>(-i_asinh.y, i_asinh.x);
        let s = jac_csin(i_asinh_rot);
        let m_s = jac_cmul(modul, s);
        phi = 0.5 * (i_asinh_rot + jac_casin(m_s));
    } else {
        let m_phi = jac_cmul(modul, phi);
        phi = 0.5 * (jac_casin(phi) + jac_casin(m_phi));
    }

    if (jtype > 3) {
        let tmp = phi;
        phi = modul;
        modul = tmp;
    }

    var geo = vec2<f32>(1.0, 0.0);
    let one = vec2<f32>(1.0, 0.0);
    let two = vec2<f32>(2.0, 0.0);
    for (var j = 0; j < 10; j = j + 1) {
        let one_minus_m = one - modul;
        if (one_minus_m.x * one_minus_m.x + one_minus_m.y * one_minus_m.y < 1e-16) {
            break;
        }
        if (j > 0) {
            let s = jac_csin(phi);
            let m_s = jac_cmul(modul, s);
            phi = 0.5 * (phi + jac_casin(m_s));
        }
        let one_plus_m = one + modul;
        let geo1 = jac_cdiv(two, one_plus_m);
        geo = jac_cmul(geo, geo1);
        modul = jac_cmul(geo1, jac_csqrt(modul));
    }

    let half_phi = 0.5 * phi;
    let arg = vec2<f32>(half_phi.x + pi_4, half_phi.y);
    let tan_arg = jac_cdiv(jac_csin(arg), jac_ccos(arg));
    let log_tan = jac_clog(tan_arg);
    return jac_cmul(geo, log_tan);
}

fn variation_jac_asn(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let kr = get_param(xform_id, variation_id, 0u);
    let ki = get_param(xform_id, variation_id, 1u);
    let jtype = i32(get_param(xform_id, variation_id, 2u));
    return jac_asn_inner(p, kr, ki, jtype);
}
"#,
    wgsl_3d: Some(r#"
fn jac_cmul(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(a.x * b.x - a.y * b.y, a.x * b.y + a.y * b.x);
}
fn jac_cdiv(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    let d = b.x * b.x + b.y * b.y;
    let safe_d = select(d, 1e-30, d < 1e-30);
    return vec2<f32>((a.x * b.x + a.y * b.y) / safe_d, (a.y * b.x - a.x * b.y) / safe_d);
}
fn jac_cabs(z: vec2<f32>) -> f32 {
    return sqrt(z.x * z.x + z.y * z.y);
}
fn jac_clog(z: vec2<f32>) -> vec2<f32> {
    let r = jac_cabs(z);
    let safe_r = select(r, 1e-30, r < 1e-30);
    return vec2<f32>(log(safe_r), atan2(z.y, z.x));
}
fn jac_csqrt(z: vec2<f32>) -> vec2<f32> {
    let r = jac_cabs(z);
    let real_p = sqrt(max((r + z.x) * 0.5, 0.0));
    let imag_p = sqrt(max((r - z.x) * 0.5, 0.0));
    let s = select(1.0, -1.0, z.y < 0.0);
    return vec2<f32>(real_p, s * imag_p);
}
fn jac_csin(z: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(sin(z.x) * cosh(z.y), cos(z.x) * sinh(z.y));
}
fn jac_ccos(z: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(cos(z.x) * cosh(z.y), -sin(z.x) * sinh(z.y));
}
fn jac_casin(z: vec2<f32>) -> vec2<f32> {
    let z2 = jac_cmul(z, z);
    let one_minus_z2 = vec2<f32>(1.0 - z2.x, -z2.y);
    let sq = jac_csqrt(one_minus_z2);
    let iz = vec2<f32>(-z.y, z.x);
    let lg = jac_clog(iz + sq);
    return vec2<f32>(lg.y, -lg.x);
}
fn jac_cacos(z: vec2<f32>) -> vec2<f32> {
    let z2 = jac_cmul(z, z);
    let one_minus_z2 = vec2<f32>(1.0 - z2.x, -z2.y);
    let sq = jac_csqrt(one_minus_z2);
    let i_sq = vec2<f32>(-sq.y, sq.x);
    let lg = jac_clog(z + i_sq);
    return vec2<f32>(lg.y, -lg.x);
}
fn jac_casinh(z: vec2<f32>) -> vec2<f32> {
    let z2 = jac_cmul(z, z);
    let z2p1 = vec2<f32>(z2.x + 1.0, z2.y);
    let sq = jac_csqrt(z2p1);
    return jac_clog(z + sq);
}

fn jac_asn_inner(in_p: vec2<f32>, kr: f32, ki: f32, jtype: i32) -> vec2<f32> {
    let pi_4 = 0.7853981633974483;
    var phi = vec2<f32>(in_p.x, in_p.y);
    var modul = vec2<f32>(kr, ki);

    let case_idx = jtype - (jtype / 4) * 4;
    if (case_idx == 0) {
        let s = jac_csin(phi);
        let m_s = jac_cmul(modul, s);
        phi = 0.5 * (phi + jac_casin(m_s));
    } else if (case_idx == 2) {
        let z2 = jac_cmul(phi, phi);
        let one_minus_z2 = vec2<f32>(1.0 - z2.x, -z2.y);
        let sq = jac_csqrt(one_minus_z2);
        let m_sq = jac_cmul(modul, sq);
        phi = 0.5 * (jac_cacos(phi) + jac_casin(m_sq));
    } else if (case_idx == 3) {
        let i_asinh = jac_casinh(phi);
        let i_asinh_rot = vec2<f32>(-i_asinh.y, i_asinh.x);
        let s = jac_csin(i_asinh_rot);
        let m_s = jac_cmul(modul, s);
        phi = 0.5 * (i_asinh_rot + jac_casin(m_s));
    } else {
        let m_phi = jac_cmul(modul, phi);
        phi = 0.5 * (jac_casin(phi) + jac_casin(m_phi));
    }

    if (jtype > 3) {
        let tmp = phi;
        phi = modul;
        modul = tmp;
    }

    var geo = vec2<f32>(1.0, 0.0);
    let one = vec2<f32>(1.0, 0.0);
    let two = vec2<f32>(2.0, 0.0);
    for (var j = 0; j < 10; j = j + 1) {
        let one_minus_m = one - modul;
        if (one_minus_m.x * one_minus_m.x + one_minus_m.y * one_minus_m.y < 1e-16) {
            break;
        }
        if (j > 0) {
            let s = jac_csin(phi);
            let m_s = jac_cmul(modul, s);
            phi = 0.5 * (phi + jac_casin(m_s));
        }
        let one_plus_m = one + modul;
        let geo1 = jac_cdiv(two, one_plus_m);
        geo = jac_cmul(geo, geo1);
        modul = jac_cmul(geo1, jac_csqrt(modul));
    }

    let half_phi = 0.5 * phi;
    let arg = vec2<f32>(half_phi.x + pi_4, half_phi.y);
    let tan_arg = jac_cdiv(jac_csin(arg), jac_ccos(arg));
    let log_tan = jac_clog(tan_arg);
    return jac_cmul(geo, log_tan);
}

fn variation_jac_asn(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let kr = get_param(xform_id, variation_id, 0u);
    let ki = get_param(xform_id, variation_id, 1u);
    let jtype = i32(get_param(xform_id, variation_id, 2u));
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let r = jac_asn_inner(vec2<f32>(p.x, p.y), kr, ki, jtype);
    return vec3<f32>(r.x, r.y, p.z * inv_w);
}
"#),
};
