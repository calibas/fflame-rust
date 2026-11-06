/// Inverse Jacobi elliptic functions — implements `asn`, `acn`, `asc`, and
/// `invdn` (the incomplete elliptic integral of the first kind) using
/// Norbert Rosch's Landen-transformation method. `jac_asn_type` selects
/// which inverse function to apply; types > 3 swap the modulus and phi. The
/// body uses a self-contained inline complex-arithmetic toolkit and bounds
/// the Landen iteration at 10 steps with early-exit when the modulus
/// converges to 1.
///
/// # Authors
/// - DarkBeam

// Jac asn, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    jac_asn_kr: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "Modulus Real",
        hint: "Real part of the elliptic modulus k. Together with `jac_asn_ki` defines the complex modulus used by the Jacobi inverse.",
    },
    jac_asn_ki: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Modulus Imag",
        hint: "Imaginary part of the elliptic modulus k.",
    },
    jac_asn_type: #{
        value: 1, choices: ["Inverse DN", "Inverse SN", "Inverse CN", "Inverse SC", "Inverse DN (swap)", "Inverse SN (swap)", "Inverse CN (swap)", "Inverse SC (swap)"],
        caption: "Type",
        hint: "Which inverse Jacobi function to compute. The `(swap)` variants additionally swap the modulus and phi after the initial branch — same function-kind axis, different parameter roles.",
    },
};

const INFO = #{ title: "Jac asn", name: "rhai_jac_asn", category: "advanced_2d", phase: "any", z: "preserve_z" };

fn jac_cmul(a, b) {
    complex(a.re * b.re - a.im * b.im, a.re * b.im + a.im * b.re)
}

fn jac_cdiv(a, b) {
    let d = b.re * b.re + b.im * b.im;
    let safe_d = (if d < 1.0e-30 { 1.0e-30 } else { d });
    complex((a.re * b.re + a.im * b.im) / safe_d, (a.im * b.re - a.re * b.im) / safe_d)
}

fn jac_cabs(z) {
    sqrt(z.re * z.re + z.im * z.im)
}

fn jac_clog(z) {
    let r = jac_cabs(z);
    let safe_r = (if r < 1.0e-30 { 1.0e-30 } else { r });
    complex(ln(safe_r), atan2(z.im, z.re))
}

fn jac_csqrt(z) {
    let r = jac_cabs(z);
    let real_p = sqrt(max(0.0, (r + z.re) * 0.5));
    let imag_p = sqrt(max(0.0, (r - z.re) * 0.5));
    let s = (if z.im < 0.0 { -1.0 } else { 1.0 });
    complex(real_p, s * imag_p)
}

fn jac_csin(z) {
    complex(sin(z.re) * cosh(z.im), cos(z.re) * sinh(z.im))
}

fn jac_ccos(z) {
    complex(cos(z.re) * cosh(z.im), -sin(z.re) * sinh(z.im))
}

fn jac_casin(z) {
    let z2 = jac_cmul(z, z);
    let one_minus_z2 = complex(1.0 - z2.re, -z2.im);
    let sq = jac_csqrt(one_minus_z2);
    let iz = complex(-z.im, z.re);
    let lg = jac_clog(iz + sq);
    complex(lg.im, -lg.re)
}

fn jac_cacos(z) {
    let z2 = jac_cmul(z, z);
    let one_minus_z2 = complex(1.0 - z2.re, -z2.im);
    let sq = jac_csqrt(one_minus_z2);
    let i_sq = complex(-sq.im, sq.re);
    let lg = jac_clog(z + i_sq);
    complex(lg.im, -lg.re)
}

fn jac_casinh(z) {
    let z2 = jac_cmul(z, z);
    let z2p1 = complex(z2.re + 1.0, z2.im);
    let sq = jac_csqrt(z2p1);
    jac_clog(z + sq)
}

fn jac_asn_inner(in_p, kr, ki, jtype) {
    let pi_4 = 0.7853981633974483;
    let phi = complex(in_p.re, in_p.im);
    let modul = complex(kr, ki);
    let case_idx = jtype - (jtype / 4) * 4;
    if (case_idx == 0) {
        let s = jac_csin(phi);
        let m_s = jac_cmul(modul, s);
        phi = 0.5 * (phi + jac_casin(m_s));
    } else if (case_idx == 2) {
        let z2 = jac_cmul(phi, phi);
        let one_minus_z2 = complex(1.0 - z2.re, -z2.im);
        let sq = jac_csqrt(one_minus_z2);
        let m_sq = jac_cmul(modul, sq);
        phi = 0.5 * (jac_cacos(phi) + jac_casin(m_sq));
    } else if (case_idx == 3) {
        let i_asinh = jac_casinh(phi);
        let i_asinh_rot = complex(-i_asinh.im, i_asinh.re);
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
    let geo = complex(1.0, 0.0);
    let one = complex(1.0, 0.0);
    let two = complex(2.0, 0.0);
    for j in 0..10 {
        let one_minus_m = one - modul;
        if (one_minus_m.re * one_minus_m.re + one_minus_m.im * one_minus_m.im < 1.0e-16) {
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
    let arg = complex(half_phi.re + pi_4, half_phi.im);
    let tan_arg = jac_cdiv(jac_csin(arg), jac_ccos(arg));
    let log_tan = jac_clog(tan_arg);
    jac_cmul(geo, log_tan)
}

fn variation(z, p) {
    let kr = p.jac_asn_kr;
    let ki = p.jac_asn_ki;
    let jtype = to_int(to_float(p.jac_asn_type));
    jac_asn_inner(z, kr, ki, jtype)
}

fn variation3(v, p, ctx) {
    let kr = p.jac_asn_kr;
    let ki = p.jac_asn_ki;
    let jtype = to_int(to_float(p.jac_asn_type));
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let r = jac_asn_inner(complex(v.x, v.y), kr, ki, jtype);
    vec3(r.re, r.im, v.z * inv_w)
}
