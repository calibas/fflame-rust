//! xtrb (TriBorders) — Cyberxaos / Zueuk
//!
//! Triangle-grid dual tessellation in trilinear coordinates: the input
//! point is mapped to a triangular cell, optionally hex-folded by the
//! `width` band, then projected via a julia-N style power back to
//! Cartesian.
//!
//! 6 user params + 22 init slots = 28 total. Over the old 16-slot
//! ceiling — port enabled by the packed-variation-params buffer.
//!
//! Slot map:
//!   0: xtrb_power (int)   1: xtrb_radius   2: xtrb_width
//!   3: xtrb_dist          4: xtrb_a        5: xtrb_b
//!   6: sinC               7: cosC
//!   8..10: S2a, S2b, S2c
//!   11..16: ac, bc, ab, ba, cb, ca
//!   17..19: Ha, Hb, Hc
//!   20..22: S2ab, S2ac, S2bc
//!   23..25: width1 = 1-w,  width2 = 2w,  width3 = 1-w²
//!   26: absN              27: cN = dist / (power · 2)
//!
//! 3 RNG calls per body (one in InverseTrilinear, one in each of two
//! Hex paths — only one fires per iter, but allocate three names). Body
//! drops the cpp `VVAR` factor since our outer multiplier supplies it.
//!
//! Source: `output/jwildfire-vars/output/xtrb.cpp` (Zueuk 2014).

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

pub static XTRB: VariationDef = VariationDef {
    name: "xtrb",
    display_name: "XTrB (TriBorders)",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("xtrb_power", "Power", int, 2.0, 1.0, 50.0),
        param!("xtrb_radius", "Radius", unlimited_float, 1.0, -10.0, 10.0),
        param!("xtrb_width", "Width", unlimited_float, 0.5, -10.0, 10.0),
        param!("xtrb_dist", "Dist", unlimited_float, 1.0, -10.0, 10.0),
        param!("xtrb_a", "A", unlimited_float, 1.0, -10.0, 10.0),
        param!("xtrb_b", "B", unlimited_float, 1.0, -10.0, 10.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 22,
    wgsl_init: Some(r#"
fn init_xtrb(user: array<f32, 6>) -> array<f32, 22> {
    let pi = 3.14159265358979;
    let power = user[0];
    let radius = user[1];
    let width = user[2];
    let dist = user[3];
    let a_param = user[4];
    let b_param = user[5];

    let angle_br = 0.047 + a_param;
    let angle_cr = 0.047 + b_param;
    let angle_ar = pi - angle_br - angle_cr;

    let sin_a2 = sin(0.5 * angle_ar);
    let cos_a2 = cos(0.5 * angle_ar);
    let sin_b2 = sin(0.5 * angle_br);
    let cos_b2 = cos(0.5 * angle_br);
    let sin_c2 = sin(0.5 * angle_cr);
    let cos_c2 = cos(0.5 * angle_cr);
    let sin_c = sin(angle_cr);
    let cos_c = cos(angle_cr);

    let safe_ca2 = select(cos_a2, 1e-30, abs(cos_a2) < 1e-30);
    let safe_cb2 = select(cos_b2, 1e-30, abs(cos_b2) < 1e-30);
    let safe_cc2 = select(cos_c2, 1e-30, abs(cos_c2) < 1e-30);
    let tan_a2 = sin_a2 / safe_ca2;
    let tan_b2 = sin_b2 / safe_cb2;
    let tan_c2 = sin_c2 / safe_cc2;

    let a_side = radius * (tan_c2 + tan_b2);
    let b_side = radius * (tan_c2 + tan_a2);
    let c_side = radius * (tan_b2 + tan_a2);

    let width1 = 1.0 - width;
    let width2 = 2.0 * width;
    let width3 = 1.0 - width * width;

    let s2 = radius * (a_side + b_side + c_side);
    let safe_a = select(a_side, 1e-30, abs(a_side) < 1e-30);
    let safe_b = select(b_side, 1e-30, abs(b_side) < 1e-30);
    let safe_c = select(c_side, 1e-30, abs(c_side) < 1e-30);
    let ha = s2 / safe_a / 6.0;
    let hb = s2 / safe_b / 6.0;
    let hc = s2 / safe_c / 6.0;
    let ab = a_side / safe_b;
    let ac = a_side / safe_c;
    let ba = b_side / safe_a;
    let bc = b_side / safe_c;
    let ca = c_side / safe_a;
    let cb = c_side / safe_b;
    let s2a = 6.0 * ha;
    let s2b = 6.0 * hb;
    let s2c = 6.0 * hc;
    let s_bc = b_side + c_side;
    let s_ab = a_side + b_side;
    let s_ac = a_side + c_side;
    let safe_bc = select(s_bc, 1e-30, abs(s_bc) < 1e-30);
    let safe_ab = select(s_ab, 1e-30, abs(s_ab) < 1e-30);
    let safe_ac = select(s_ac, 1e-30, abs(s_ac) < 1e-30);
    let s2bc = s2 / safe_bc / 6.0;
    let s2ab = s2 / safe_ab / 6.0;
    let s2ac = s2 / safe_ac / 6.0;

    let abs_n = abs(power);
    let safe_pow = select(power, 1e-30, abs(power) < 1e-30);
    let c_n = dist / safe_pow / 2.0;

    var out: array<f32, 22>;
    out[0] = sin_c;
    out[1] = cos_c;
    out[2] = s2a;
    out[3] = s2b;
    out[4] = s2c;
    out[5] = ac;
    out[6] = bc;
    out[7] = ab;
    out[8] = ba;
    out[9] = cb;
    out[10] = ca;
    out[11] = ha;
    out[12] = hb;
    out[13] = hc;
    out[14] = s2ab;
    out[15] = s2ac;
    out[16] = s2bc;
    out[17] = width1;
    out[18] = width2;
    out[19] = width3;
    out[20] = abs_n;
    out[21] = c_n;
    return out;
}
"#),
    wgsl_2d: r#"
fn variation_xtrb(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let pi = 3.14159265358979;
    let two_pi = 6.28318530717959;
    let power = get_param(xform_id, variation_id, 0u);
    let radius = get_param(xform_id, variation_id, 1u);
    let width = get_param(xform_id, variation_id, 2u);
    let sin_c = get_param(xform_id, variation_id, 6u);
    let cos_c = get_param(xform_id, variation_id, 7u);
    let s2a = get_param(xform_id, variation_id, 8u);
    let s2b = get_param(xform_id, variation_id, 9u);
    let s2c = get_param(xform_id, variation_id, 10u);
    let ac = get_param(xform_id, variation_id, 11u);
    let bc = get_param(xform_id, variation_id, 12u);
    let ab = get_param(xform_id, variation_id, 13u);
    let ba = get_param(xform_id, variation_id, 14u);
    let cb = get_param(xform_id, variation_id, 15u);
    let ca = get_param(xform_id, variation_id, 16u);
    let ha = get_param(xform_id, variation_id, 17u);
    let hb = get_param(xform_id, variation_id, 18u);
    let hc = get_param(xform_id, variation_id, 19u);
    let s2ab = get_param(xform_id, variation_id, 20u);
    let s2ac = get_param(xform_id, variation_id, 21u);
    let s2bc = get_param(xform_id, variation_id, 22u);
    let width1 = get_param(xform_id, variation_id, 23u);
    let width2 = get_param(xform_id, variation_id, 24u);
    let width3 = get_param(xform_id, variation_id, 25u);
    let abs_n = get_param(xform_id, variation_id, 26u);
    let c_n = get_param(xform_id, variation_id, 27u);
    let safe_s2a = select(s2a, 1e-30, abs(s2a) < 1e-30);
    let safe_s2b = select(s2b, 1e-30, abs(s2b) < 1e-30);
    let safe_sin_c = select(sin_c, 1e-30, abs(sin_c) < 1e-30);
    let safe_power = select(power, 1e-30, abs(power) < 1e-30);

    // DirectTrilinear
    let u = p.y + radius;
    let v = p.x * sin_c - p.y * cos_c + radius;
    var alpha = u;
    var beta = v;

    let m = floor(alpha / safe_s2a);
    var off_al = alpha - m * s2a;
    let n_idx = floor(beta / safe_s2b);
    var off_be = beta - n_idx * s2b;
    var off_ga = s2c - ac * off_al - bc * off_be;

    var flipped = false;
    if (off_ga <= 0.0) {
        off_al = s2a - off_al;
        off_be = s2b - off_be;
        off_ga = -off_ga;
        flipped = true;
    }

    // Hex(off_al, off_be, off_ga) → (al1, be1)
    let r_rand = rng_nextf(rng);
    var al1 = 0.0;
    var be1 = 0.0;
    if (off_be < off_al) {
        if (off_ga < off_be) {
            // branch 1.1
            var de1 = 0.0;
            var ga1 = 0.0;
            if (r_rand >= width3) {
                de1 = width * off_be;
                ga1 = width * off_ga;
            } else {
                let safe_be = select(off_be, 1e-30, abs(off_be) < 1e-30);
                ga1 = width1 * off_ga + width2 * hc * off_ga / safe_be;
                de1 = width1 * off_be + width2 * s2ab * (3.0 - off_ga / safe_be);
            }
            al1 = s2a - ba * de1 - ca * ga1;
            be1 = de1;
        } else if (off_ga < off_al) {
            // branch 1.2
            var de1 = 0.0;
            var ga1 = 0.0;
            if (r_rand >= width3) {
                ga1 = width * off_ga;
                de1 = width * off_be;
            } else {
                let safe_ga = select(off_ga, 1e-30, abs(off_ga) < 1e-30);
                de1 = width1 * off_be + width2 * hb * off_be / safe_ga;
                ga1 = width1 * off_ga + width2 * s2ac * (3.0 - off_be / safe_ga);
            }
            al1 = s2a - ba * de1 - ca * ga1;
            be1 = de1;
        } else {
            // branch 1.3
            if (r_rand >= width3) {
                al1 = width * off_al;
                be1 = width * off_be;
            } else {
                let safe_al = select(off_al, 1e-30, abs(off_al) < 1e-30);
                be1 = width1 * off_be + width2 * hb * off_be / safe_al;
                al1 = width1 * off_al + width2 * s2ac * (3.0 - off_be / safe_al);
            }
        }
    } else {
        if (off_ga < off_al) {
            // branch 2.1
            var de1 = 0.0;
            var ga1 = 0.0;
            if (r_rand >= width3) {
                de1 = width * off_al;
                ga1 = width * off_ga;
            } else {
                let safe_al = select(off_al, 1e-30, abs(off_al) < 1e-30);
                ga1 = width1 * off_ga + width2 * hc * off_ga / safe_al;
                de1 = width1 * off_al + width2 * s2ab * (3.0 - off_ga / safe_al);
            }
            be1 = s2b - ab * de1 - cb * ga1;
            al1 = de1;
        } else if (off_ga < off_be) {
            // branch 2.2
            var de1 = 0.0;
            var ga1 = 0.0;
            if (r_rand >= width3) {
                ga1 = width * off_ga;
                de1 = width * off_al;
            } else {
                let safe_ga = select(off_ga, 1e-30, abs(off_ga) < 1e-30);
                de1 = width1 * off_al + width2 * ha * off_al / safe_ga;
                ga1 = width1 * off_ga + width2 * s2bc * (3.0 - off_al / safe_ga);
            }
            be1 = s2b - ab * de1 - cb * ga1;
            al1 = de1;
        } else {
            // branch 2.3
            if (r_rand >= width3) {
                be1 = width * off_be;
                al1 = width * off_al;
            } else {
                let safe_be = select(off_be, 1e-30, abs(off_be) < 1e-30);
                al1 = width1 * off_al + width2 * ha * off_al / safe_be;
                be1 = width1 * off_be + width2 * s2bc * (3.0 - off_al / safe_be);
            }
        }
    }

    if (flipped) {
        al1 = s2a - al1;
        be1 = s2b - be1;
    }

    alpha = al1 + m * s2a;
    beta = be1 + n_idx * s2b;

    // InverseTrilinear
    let inx = (beta - radius + (alpha - radius) * cos_c) / safe_sin_c;
    let iny = alpha - radius;
    let n_int = max(floor(rng_nextf(rng) * abs_n), 0.0);
    let angle = atan2(iny, inx) + two_pi * n_int / safe_power;
    let r = pow(max(inx * inx + iny * iny, 1e-30), c_n);
    return vec2<f32>(r * cos(angle), r * sin(angle));
}
"#,
    wgsl_3d: Some(r#"
fn variation_xtrb(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let pi = 3.14159265358979;
    let two_pi = 6.28318530717959;
    let power = get_param(xform_id, variation_id, 0u);
    let radius = get_param(xform_id, variation_id, 1u);
    let width = get_param(xform_id, variation_id, 2u);
    let sin_c = get_param(xform_id, variation_id, 6u);
    let cos_c = get_param(xform_id, variation_id, 7u);
    let s2a = get_param(xform_id, variation_id, 8u);
    let s2b = get_param(xform_id, variation_id, 9u);
    let s2c = get_param(xform_id, variation_id, 10u);
    let ac = get_param(xform_id, variation_id, 11u);
    let bc = get_param(xform_id, variation_id, 12u);
    let ab = get_param(xform_id, variation_id, 13u);
    let ba = get_param(xform_id, variation_id, 14u);
    let cb = get_param(xform_id, variation_id, 15u);
    let ca = get_param(xform_id, variation_id, 16u);
    let ha = get_param(xform_id, variation_id, 17u);
    let hb = get_param(xform_id, variation_id, 18u);
    let hc = get_param(xform_id, variation_id, 19u);
    let s2ab = get_param(xform_id, variation_id, 20u);
    let s2ac = get_param(xform_id, variation_id, 21u);
    let s2bc = get_param(xform_id, variation_id, 22u);
    let width1 = get_param(xform_id, variation_id, 23u);
    let width2 = get_param(xform_id, variation_id, 24u);
    let width3 = get_param(xform_id, variation_id, 25u);
    let abs_n = get_param(xform_id, variation_id, 26u);
    let c_n = get_param(xform_id, variation_id, 27u);
    let safe_s2a = select(s2a, 1e-30, abs(s2a) < 1e-30);
    let safe_s2b = select(s2b, 1e-30, abs(s2b) < 1e-30);
    let safe_sin_c = select(sin_c, 1e-30, abs(sin_c) < 1e-30);
    let safe_power = select(power, 1e-30, abs(power) < 1e-30);

    let u = p.y + radius;
    let v = p.x * sin_c - p.y * cos_c + radius;
    var alpha = u;
    var beta = v;

    let m = floor(alpha / safe_s2a);
    var off_al = alpha - m * s2a;
    let n_idx = floor(beta / safe_s2b);
    var off_be = beta - n_idx * s2b;
    var off_ga = s2c - ac * off_al - bc * off_be;

    var flipped = false;
    if (off_ga <= 0.0) {
        off_al = s2a - off_al;
        off_be = s2b - off_be;
        off_ga = -off_ga;
        flipped = true;
    }

    let r_rand = rng_nextf(rng);
    var al1 = 0.0;
    var be1 = 0.0;
    if (off_be < off_al) {
        if (off_ga < off_be) {
            var de1 = 0.0;
            var ga1 = 0.0;
            if (r_rand >= width3) {
                de1 = width * off_be;
                ga1 = width * off_ga;
            } else {
                let safe_be = select(off_be, 1e-30, abs(off_be) < 1e-30);
                ga1 = width1 * off_ga + width2 * hc * off_ga / safe_be;
                de1 = width1 * off_be + width2 * s2ab * (3.0 - off_ga / safe_be);
            }
            al1 = s2a - ba * de1 - ca * ga1;
            be1 = de1;
        } else if (off_ga < off_al) {
            var de1 = 0.0;
            var ga1 = 0.0;
            if (r_rand >= width3) {
                ga1 = width * off_ga;
                de1 = width * off_be;
            } else {
                let safe_ga = select(off_ga, 1e-30, abs(off_ga) < 1e-30);
                de1 = width1 * off_be + width2 * hb * off_be / safe_ga;
                ga1 = width1 * off_ga + width2 * s2ac * (3.0 - off_be / safe_ga);
            }
            al1 = s2a - ba * de1 - ca * ga1;
            be1 = de1;
        } else {
            if (r_rand >= width3) {
                al1 = width * off_al;
                be1 = width * off_be;
            } else {
                let safe_al = select(off_al, 1e-30, abs(off_al) < 1e-30);
                be1 = width1 * off_be + width2 * hb * off_be / safe_al;
                al1 = width1 * off_al + width2 * s2ac * (3.0 - off_be / safe_al);
            }
        }
    } else {
        if (off_ga < off_al) {
            var de1 = 0.0;
            var ga1 = 0.0;
            if (r_rand >= width3) {
                de1 = width * off_al;
                ga1 = width * off_ga;
            } else {
                let safe_al = select(off_al, 1e-30, abs(off_al) < 1e-30);
                ga1 = width1 * off_ga + width2 * hc * off_ga / safe_al;
                de1 = width1 * off_al + width2 * s2ab * (3.0 - off_ga / safe_al);
            }
            be1 = s2b - ab * de1 - cb * ga1;
            al1 = de1;
        } else if (off_ga < off_be) {
            var de1 = 0.0;
            var ga1 = 0.0;
            if (r_rand >= width3) {
                ga1 = width * off_ga;
                de1 = width * off_al;
            } else {
                let safe_ga = select(off_ga, 1e-30, abs(off_ga) < 1e-30);
                de1 = width1 * off_al + width2 * ha * off_al / safe_ga;
                ga1 = width1 * off_ga + width2 * s2bc * (3.0 - off_al / safe_ga);
            }
            be1 = s2b - ab * de1 - cb * ga1;
            al1 = de1;
        } else {
            if (r_rand >= width3) {
                be1 = width * off_be;
                al1 = width * off_al;
            } else {
                let safe_be = select(off_be, 1e-30, abs(off_be) < 1e-30);
                al1 = width1 * off_al + width2 * ha * off_al / safe_be;
                be1 = width1 * off_be + width2 * s2bc * (3.0 - off_al / safe_be);
            }
        }
    }

    if (flipped) {
        al1 = s2a - al1;
        be1 = s2b - be1;
    }

    alpha = al1 + m * s2a;
    beta = be1 + n_idx * s2b;

    let inx = (beta - radius + (alpha - radius) * cos_c) / safe_sin_c;
    let iny = alpha - radius;
    let n_int = max(floor(rng_nextf(rng) * abs_n), 0.0);
    let angle = atan2(iny, inx) + two_pi * n_int / safe_power;
    let r = pow(max(inx * inx + iny * iny, 1e-30), c_n);
    return vec3<f32>(r * cos(angle), r * sin(angle), p.z);
}
"#),
};
