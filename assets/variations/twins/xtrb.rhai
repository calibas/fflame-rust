/// TriBorders — triangular-grid dual tessellation in trilinear coordinates.
/// Maps the input point to a triangular cell whose interior angles are set
/// by `xtrb_a` and `xtrb_b` (the third follows from the angle-sum law),
/// optionally hex-folds it through a band of width `xtrb_width`, then
/// projects via a Julia-N style angular sampling (`xtrb_power` equally-
/// spaced angles, radial exponent `xtrb_dist`) back to Cartesian. The body
/// has six branches depending on the relative ordering of the three
/// trilinear offsets, each with a hex-fold sub-branch gated by
/// `width3 = 1 - w²`.
///
/// # Authors
/// - Xyrus02

// XTrB (TriBorders), the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    xtrb_power: #{
        value: 2, min: 1, max: 50,
        caption: "Power",
        hint: "Julia-N branch count (integer). Each iteration picks one of `|power|` equally-spaced angles from the projected coordinate, scaling the polar angle by `1/power`.",
    },
    xtrb_radius: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Radius",
        hint: "Triangle inscribed radius — sets the overall scale of the triangular cells in the tessellation.",
    },
    xtrb_width: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "Width",
        hint: "Hex-fold band width within each cell. 0 = no hex fold (pure triangle dual); larger values push more of the cell area through the hex sub-branches before the Julia projection.",
    },
    xtrb_dist: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Dist",
        hint: "Radial exponent multiplier. Combines with `xtrb_power` as `dist / (2·power)` to set the radial blow-up power.",
    },
    xtrb_a: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "A",
        hint: "Angle parameter A (radians). Internally `0.047 + a` becomes one of the triangle's interior angles; the third angle is derived from the angle-sum law `π - angle_b - angle_c`.",
    },
    xtrb_b: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "B",
        hint: "Angle parameter B (radians). Internally `0.047 + b` becomes another interior angle.",
    },
};

const INFO = #{ title: "XTrB (TriBorders)", name: "rhai_xtrb", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let pi = 3.14159265358979;
    let power = to_float(p.xtrb_power);
    let radius = p.xtrb_radius;
    let width = p.xtrb_width;
    let dist = p.xtrb_dist;
    let a_param = p.xtrb_a;
    let b_param = p.xtrb_b;
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
    let safe_ca2 = (if abs(cos_a2) < 1.0e-30 { 1.0e-30 } else { cos_a2 });
    let safe_cb2 = (if abs(cos_b2) < 1.0e-30 { 1.0e-30 } else { cos_b2 });
    let safe_cc2 = (if abs(cos_c2) < 1.0e-30 { 1.0e-30 } else { cos_c2 });
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
    let safe_a = (if abs(a_side) < 1.0e-30 { 1.0e-30 } else { a_side });
    let safe_b = (if abs(b_side) < 1.0e-30 { 1.0e-30 } else { b_side });
    let safe_c = (if abs(c_side) < 1.0e-30 { 1.0e-30 } else { c_side });
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
    let safe_bc = (if abs(s_bc) < 1.0e-30 { 1.0e-30 } else { s_bc });
    let safe_ab = (if abs(s_ab) < 1.0e-30 { 1.0e-30 } else { s_ab });
    let safe_ac = (if abs(s_ac) < 1.0e-30 { 1.0e-30 } else { s_ac });
    let s2bc = s2 / safe_bc / 6.0;
    let s2ab = s2 / safe_ab / 6.0;
    let s2ac = s2 / safe_ac / 6.0;
    let abs_n = abs(power);
    let safe_pow = (if abs(power) < 1.0e-30 { 1.0e-30 } else { power });
    let c_n = dist / safe_pow / 2.0;
    #{ sin_c: sin_c, cos_c: cos_c, s2a: s2a, s2b: s2b, s2c: s2c, ac: ac, bc: bc, ab: ab, ba: ba, cb: cb, ca: ca, ha: ha, hb: hb, hc: hc, s2ab: s2ab, s2ac: s2ac, s2bc: s2bc, width1: width1, width2: width2, width3: width3, abs_n: abs_n, c_n: c_n }
}

fn variation(z, p) {
    let pi = 3.14159265358979;
    let two_pi = 6.28318530717959;
    let power = to_float(p.xtrb_power);
    let radius = p.xtrb_radius;
    let width = p.xtrb_width;
    let sin_c = p.sin_c;
    let cos_c = p.cos_c;
    let s2a = p.s2a;
    let s2b = p.s2b;
    let s2c = p.s2c;
    let ac = p.ac;
    let bc = p.bc;
    let ab = p.ab;
    let ba = p.ba;
    let cb = p.cb;
    let ca = p.ca;
    let ha = p.ha;
    let hb = p.hb;
    let hc = p.hc;
    let s2ab = p.s2ab;
    let s2ac = p.s2ac;
    let s2bc = p.s2bc;
    let width1 = p.width1;
    let width2 = p.width2;
    let width3 = p.width3;
    let abs_n = p.abs_n;
    let c_n = p.c_n;
    let safe_s2a = (if abs(s2a) < 1.0e-30 { 1.0e-30 } else { s2a });
    let safe_s2b = (if abs(s2b) < 1.0e-30 { 1.0e-30 } else { s2b });
    let safe_sin_c = (if abs(sin_c) < 1.0e-30 { 1.0e-30 } else { sin_c });
    let safe_power = (if abs(power) < 1.0e-30 { 1.0e-30 } else { power });
    let u = z.im + radius;
    let v = z.re * sin_c - z.im * cos_c + radius;
    let alpha = u;
    let beta = v;
    let m = to_float(floor(alpha / safe_s2a));
    let off_al = alpha - m * s2a;
    let n_idx = to_float(floor(beta / safe_s2b));
    let off_be = beta - n_idx * s2b;
    let off_ga = s2c - ac * off_al - bc * off_be;
    let flipped = false;
    if (off_ga <= 0.0) {
        off_al = s2a - off_al;
        off_be = s2b - off_be;
        off_ga = -off_ga;
        flipped = true;
    }
    let r_rand = rand();
    let al1 = 0.0;
    let be1 = 0.0;
    if (off_be < off_al) {
        if (off_ga < off_be) {
            let de1 = 0.0;
            let ga1 = 0.0;
            if (r_rand >= width3) {
                de1 = width * off_be;
                ga1 = width * off_ga;
            } else {
                let safe_be = (if abs(off_be) < 1.0e-30 { 1.0e-30 } else { off_be });
                ga1 = width1 * off_ga + width2 * hc * off_ga / safe_be;
                de1 = width1 * off_be + width2 * s2ab * (3.0 - off_ga / safe_be);
            }
            al1 = s2a - ba * de1 - ca * ga1;
            be1 = de1;
        } else if (off_ga < off_al) {
            let de1 = 0.0;
            let ga1 = 0.0;
            if (r_rand >= width3) {
                ga1 = width * off_ga;
                de1 = width * off_be;
            } else {
                let safe_ga = (if abs(off_ga) < 1.0e-30 { 1.0e-30 } else { off_ga });
                de1 = width1 * off_be + width2 * hb * off_be / safe_ga;
                ga1 = width1 * off_ga + width2 * s2ac * (3.0 - off_be / safe_ga);
            }
            al1 = s2a - ba * de1 - ca * ga1;
            be1 = de1;
        } else if (r_rand >= width3) {
            al1 = width * off_al;
            be1 = width * off_be;
        } else {
            let safe_al = (if abs(off_al) < 1.0e-30 { 1.0e-30 } else { off_al });
            be1 = width1 * off_be + width2 * hb * off_be / safe_al;
            al1 = width1 * off_al + width2 * s2ac * (3.0 - off_be / safe_al);
        }
    } else if (off_ga < off_al) {
        let de1 = 0.0;
        let ga1 = 0.0;
        if (r_rand >= width3) {
            de1 = width * off_al;
            ga1 = width * off_ga;
        } else {
            let safe_al = (if abs(off_al) < 1.0e-30 { 1.0e-30 } else { off_al });
            ga1 = width1 * off_ga + width2 * hc * off_ga / safe_al;
            de1 = width1 * off_al + width2 * s2ab * (3.0 - off_ga / safe_al);
        }
        be1 = s2b - ab * de1 - cb * ga1;
        al1 = de1;
    } else if (off_ga < off_be) {
        let de1 = 0.0;
        let ga1 = 0.0;
        if (r_rand >= width3) {
            ga1 = width * off_ga;
            de1 = width * off_al;
        } else {
            let safe_ga = (if abs(off_ga) < 1.0e-30 { 1.0e-30 } else { off_ga });
            de1 = width1 * off_al + width2 * ha * off_al / safe_ga;
            ga1 = width1 * off_ga + width2 * s2bc * (3.0 - off_al / safe_ga);
        }
        be1 = s2b - ab * de1 - cb * ga1;
        al1 = de1;
    } else if (r_rand >= width3) {
        be1 = width * off_be;
        al1 = width * off_al;
    } else {
        let safe_be = (if abs(off_be) < 1.0e-30 { 1.0e-30 } else { off_be });
        al1 = width1 * off_al + width2 * ha * off_al / safe_be;
        be1 = width1 * off_be + width2 * s2bc * (3.0 - off_al / safe_be);
    }
    if (flipped) {
        al1 = s2a - al1;
        be1 = s2b - be1;
    }
    alpha = al1 + m * s2a;
    beta = be1 + n_idx * s2b;
    let inx = (beta - radius + (alpha - radius) * cos_c) / safe_sin_c;
    let iny = alpha - radius;
    let n_int = max(0.0, to_float(floor(rand() * abs_n)));
    let angle = atan2(iny, inx) + two_pi * n_int / safe_power;
    let r = pow(max(1.0e-30, inx * inx + iny * iny), c_n);
    complex(r * cos(angle), r * sin(angle))
}
