/// The Apollonian gasket Kleinian group (Indra's Pearls Ch. 7) with QC
/// deformation.
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5

// Apollonian Gasket, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    avoid_reversal: #{
        value: true,
        caption: "Avoid Reversal",
        hint: "Skip a generator's inverse immediately after applying it (no g·g⁻¹ cancellation) — the Indra's Pearls chaos-game rule. Off = all four generators equally likely every call.",
    },
    qc_deform: #{
        value: false,
        caption: "QC Deform",
        hint: "Apply Bagula's triquasiconformal conjugation C = dk(δ)·s0·qf(θ+iη) to every generator — the 'even → uneven' gasket deformation. Conjugation is a Möbius change of coordinates: circles stay circles and tangencies stay tangencies, but the packing warps out of its symmetric frame. Off = the exact even gasket.",
    },
    conj_angle: #{
        value: 45.0, min: -360.0, max: 360.0, type: "angle",
        caption: "Angle",
        hint: "Elliptic rotation θ in the conjugator qf = rotate(θ + iη) (QC Deform on). Sweeping it slides the gasket through uneven framings.",
    },
    conj_hyper: #{
        value: 0.0, min: -360.0, max: 360.0, type: "angle",
        caption: "Hyper Angle",
        hint: "Hyperbolic rotation η (imaginary angle) in qf = rotate(θ + iη) (QC Deform on). Compresses the packing toward a limit point.",
    },
    qc_strength: #{
        value: 1.0, min: 0.1, max: 2.0,
        caption: "QC Strength",
        hint: "Quasiconformal δ in dk = [[1+iδ,1],[1,1−iδ]] (QC Deform on).",
    },
    dc_mode: #{
        value: 0, choices: ["Off", "Generator"],
        caption: "Color Mode",
        hint: "Direct-color source (needs the transform's Direct Color > 0). Generator: each of the four group elements has its own palette position, blended through a persistent color register at Color Speed.",
    },
    dc_scale: #{
        value: 1.0, min: 0.1, max: 8.0,
        caption: "Color Scale",
        hint: "Palette-index multiplier for the Generator color (wrapped with fract).",
    },
    color_speed: #{
        value: 0.5, min: 0.0, max: 1.0,
        caption: "Color Speed",
        hint: "How hard each generator pulls the color register toward its own palette position.",
    },
    space: #{
        value: 0, choices: ["Euclidean", "Hyperbolic H3"],
        caption: "Space",
        hint: "3D render mode only. Euclidean: the flat gasket in the xy plane (z passes through). Hyperbolic H3: the Poincaré extension — the same SL(2,C) matrices act on upper-half-space (the point as a quaternion x+yi+t·j), filling the gasket out to the Apollonian sphere packing. The z-slice at t→0 is the 2D picture.",
    },
    symmetry: #{
        value: 0, choices: ["None", "2-Fold Point", "2-Fold Mirror", "4-Fold"],
        caption: "Symmetry",
        hint: "Random output symmetrization each call: 2-Fold Point = {z,−z}, 2-Fold Mirror = {z,conj z}, 4-Fold = both.",
    },
};

const INFO = #{ title: "Apollonian Gasket", name: "rhai_apollonian_gasket", category: "advanced_2d" };

fn init(p) {
    #{ st0: 0.0, creg: 0.0 }
}

fn su_cmul(x, y) {
    complex(x.re * y.re - x.im * y.im, x.re * y.im + x.im * y.re)
}

fn su_cdiv(x, y) {
    let dn = y.norm_sqr() + 1.0e-30;
    complex(x.re * y.re + x.im * y.im, x.im * y.re - x.re * y.im) / dn
}

fn su_matinv(P) {
    let det = su_cmul(P[0], P[3]) - su_cmul(P[1], P[2]);
    [su_cdiv(P[3], det), su_cdiv(-P[1], det), su_cdiv(-P[2], det), su_cdiv(P[0], det)]
}

fn su_matmul(P, Q) {
    [su_cmul(P[0], Q[0]) + su_cmul(P[1], Q[2]), su_cmul(P[0], Q[1]) + su_cmul(P[1], Q[3]), su_cmul(P[2], Q[0]) + su_cmul(P[3], Q[2]), su_cmul(P[2], Q[1]) + su_cmul(P[3], Q[3])]
}

fn su_conjugator(theta, eta, delta) {
    let ch = cosh(eta);
    let sh = sinh(eta);
    let ct = cos(theta);
    let st = sin(theta);
    let ca = complex(ct * ch, -st * sh);
    let sa = complex(st * ch, ct * sh);
    let qf = [ca, complex(-sa.re, -sa.im), sa, ca];
    let dk = [complex(1.0, delta), complex(1.0, 0.0), complex(1.0, 0.0), complex(1.0, -delta)];
    let s0 = [complex(vec4(0.7071068, 0.0, 0.0, -0.7071068).x, vec4(0.7071068, 0.0, 0.0, -0.7071068).y), complex(vec4(0.7071068, 0.0, 0.0, -0.7071068).z, vec4(0.7071068, 0.0, 0.0, -0.7071068).w), complex(vec4(0.0, -0.7071068, 0.7071068, 0.0).x, vec4(0.0, -0.7071068, 0.7071068, 0.0).y), complex(vec4(0.0, -0.7071068, 0.7071068, 0.0).z, vec4(0.0, -0.7071068, 0.7071068, 0.0).w)];
    su_matmul(su_matmul(dk, s0), qf)
}

fn su_apply_m(base, z, cj, cji) {
    let m = su_matmul(su_matmul(cj, base), cji);
    su_cdiv(su_cmul(m[0], z) + m[1], su_cmul(m[2], z) + m[3])
}

fn su_apply_plain(m, z) {
    su_cdiv(su_cmul(m[0], z) + m[1], su_cmul(m[2], z) + m[3])
}

fn apollonian_gen(k) {
    let t1 = k;
    if t1 == u32(0) {
        [complex(1.0, 0.0), complex(0.0, 0.0), complex(0.0, -2.0), complex(1.0, 0.0)]
    } else if t1 == u32(1) {
        [complex(1.0, -1.0), complex(1.0, 0.0), complex(1.0, 0.0), complex(1.0, 1.0)]
    } else if t1 == u32(2) {
        [complex(1.0, 0.0), complex(0.0, 0.0), complex(0.0, 2.0), complex(1.0, 0.0)]
    } else {
        [complex(1.0, 1.0), complex(-1.0, 0.0), complex(-1.0, 0.0), complex(1.0, -1.0)]
    }
}

fn su_qmul(a, b) {
    vec4(a.x * b.x - a.y * b.y - a.z * b.z - a.w * b.w, a.x * b.y + a.y * b.x + a.z * b.w - a.w * b.z, a.x * b.z - a.y * b.w + a.z * b.x + a.w * b.y, a.x * b.w + a.y * b.z - a.z * b.y + a.w * b.x)
}

fn su_qinv(a) {
    let n = (a.x * a.x + a.y * a.y + a.z * a.z + a.w * a.w) + 1.0e-30;
    vec4(a.x, -a.y, -a.z, -a.w) / n
}

fn su_apply_m3(base, p3, cj, cji) {
    let m = su_matmul(su_matmul(cj, base), cji);
    let t1 = m[0];
    let qa = vec4(t1.re, t1.im, 0.0, 0.0);
    let t2 = m[1];
    let qb = vec4(t2.re, t2.im, 0.0, 0.0);
    let t3 = m[2];
    let qc = vec4(t3.re, t3.im, 0.0, 0.0);
    let t4 = m[3];
    let qd = vec4(t4.re, t4.im, 0.0, 0.0);
    let q = vec4(p3.x, p3.y, abs(p3.z) + 1.0e-4, 0.0);
    let r = su_qmul(su_qmul(qa, q) + qb, su_qinv(su_qmul(qc, q) + qd));
    vec3(r.x, r.y, r.z)
}

fn su_apply_plain3(m, p3) {
    let t1 = m[0];
    let qa = vec4(t1.re, t1.im, 0.0, 0.0);
    let t2 = m[1];
    let qb = vec4(t2.re, t2.im, 0.0, 0.0);
    let t3 = m[2];
    let qc = vec4(t3.re, t3.im, 0.0, 0.0);
    let t4 = m[3];
    let qd = vec4(t4.re, t4.im, 0.0, 0.0);
    let q = vec4(p3.x, p3.y, abs(p3.z) + 1.0e-4, 0.0);
    let r = su_qmul(su_qmul(qa, q) + qb, su_qinv(su_qmul(qc, q) + qd));
    vec3(r.x, r.y, r.z)
}

fn variation(z, p, ctx) {
    let color = ctx.color;
    let avoid = to_float(p.avoid_reversal) > 0.5;
    let deform = to_float(p.qc_deform) > 0.5;
    let theta = p.conj_angle * 0.01745329252;
    let eta = p.conj_hyper * 0.01745329252;
    let delta = p.qc_strength;
    let dc_mode = u32(to_float(p.dc_mode));
    let dc_scale = p.dc_scale;
    let color_speed = p.color_speed;
    let k = min(u32(rand() * 4.0), u32(3));
    if (avoid) {
        let prev = u32(this.st0);
        if (prev < u32(4) && k == (prev + u32(2)) % u32(4)) {
            k = (k + u32(1)) % u32(4);
        }
    }
    this.st0 = to_float(k);
    if (dc_mode == u32(1)) {
        let creg = this.creg;
        creg = mix(creg, fract((to_float(k) + 0.5) / 4.0 * dc_scale), color_speed);
        this.creg = creg;
        color = creg;
    }
    let g = apollonian_gen(k);
    let out = complex(0.0, 0.0);
    if (deform) {
        let cj = su_conjugator(theta, eta, delta);
        out = su_apply_m(g, z, cj, su_matinv(cj));
    } else {
        out = su_apply_plain(g, z);
    }
    let symm = u32(to_float(p.symmetry));
    if (symm == u32(1)) {
        if (rand() < 0.5) {
            out = -out;
        }
    } else if (symm == u32(2)) {
        if (rand() < 0.5) {
            out = complex(out.re, -out.im);
        }
    } else if (symm == u32(3)) {
        let r = min(u32(rand() * 4.0), u32(3));
        if (r == u32(1)) {
            out = -out;
        } else if (r == u32(2)) {
            out = complex(out.re, -out.im);
        } else if (r == u32(3)) {
            out = complex(-out.re, out.im);
        }
    }
    #{ z: out, color: color }
}

fn variation3(v, p, ctx) {
    let color = ctx.color;
    let avoid = to_float(p.avoid_reversal) > 0.5;
    let deform = to_float(p.qc_deform) > 0.5;
    let theta = p.conj_angle * 0.01745329252;
    let eta = p.conj_hyper * 0.01745329252;
    let delta = p.qc_strength;
    let dc_mode = u32(to_float(p.dc_mode));
    let dc_scale = p.dc_scale;
    let color_speed = p.color_speed;
    let k = min(u32(rand() * 4.0), u32(3));
    if (avoid) {
        let prev = u32(this.st0);
        if (prev < u32(4) && k == (prev + u32(2)) % u32(4)) {
            k = (k + u32(1)) % u32(4);
        }
    }
    this.st0 = to_float(k);
    if (dc_mode == u32(1)) {
        let creg = this.creg;
        creg = mix(creg, fract((to_float(k) + 0.5) / 4.0 * dc_scale), color_speed);
        this.creg = creg;
        color = creg;
    }
    let g = apollonian_gen(k);
    let space = u32(to_float(p.space));
    let out = vec3(0.0, 0.0, 0.0);
    if (deform) {
        let cj = su_conjugator(theta, eta, delta);
        let cji = su_matinv(cj);
        if (space == u32(1)) {
            out = su_apply_m3(g, v, cj, cji);
        } else {
            let t1 = su_apply_m(g, complex(v.x, v.y), cj, cji);
            out = vec3(t1.re, t1.im, v.z);
        }
    } else if (space == u32(1)) {
        out = su_apply_plain3(g, v);
    } else {
        let t2 = su_apply_plain(g, complex(v.x, v.y));
        out = vec3(t2.re, t2.im, v.z);
    }
    let symm = u32(to_float(p.symmetry));
    if (symm == u32(1)) {
        if (rand() < 0.5) {
            out = vec3(-out.x, -out.y, out.z);
        }
    } else if (symm == u32(2)) {
        if (rand() < 0.5) {
            out = vec3(out.x, -out.y, out.z);
        }
    } else if (symm == u32(3)) {
        let r = min(u32(rand() * 4.0), u32(3));
        if (r == u32(1)) {
            out = vec3(-out.x, -out.y, out.z);
        } else if (r == u32(2)) {
            out = vec3(out.x, -out.y, out.z);
        } else if (r == u32(3)) {
            out = vec3(-out.x, out.y, out.z);
        }
    }
    #{ z: out, color: color }
}
