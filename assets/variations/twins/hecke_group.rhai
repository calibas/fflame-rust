/// Hecke group H(q) orbit chaos game; q = 3 is the modular group
/// PSL(2,ℤ).
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5

// Hecke Group, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    q: #{
        value: 3, min: 3, max: 24,
        caption: "Q",
        hint: "Hecke parameter: λ = 2·cos(π/q). q = 3 is the modular group PSL(2,ℤ) (Farey / Ford circles); larger q widens the translation toward the λ = 2 limit.",
    },
    avoid_reversal: #{
        value: true,
        caption: "Avoid Reversal",
        hint: "Skip the inverse of the previous generator (T after T⁻¹ and vice versa; S is its own inverse and is never blocked). Off = all three generators equally likely.",
    },
    dc_mode: #{
        value: 0, choices: ["Off", "Generator"],
        caption: "Color Mode",
        hint: "Direct-color source (needs the transform's Direct Color > 0). Generator: S / T / T⁻¹ each have their own palette position, blended through a persistent color register at Color Speed.",
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
        hint: "3D render mode only. Euclidean: the Möbius orbit in the xy plane (z passes through). Hyperbolic H3: the Poincaré extension into upper half-space — the Ford-sphere fan. The t→0 slice is the 2D picture.",
    },
    symmetry: #{
        value: 0, choices: ["None", "2-Fold Point", "2-Fold Mirror", "4-Fold"],
        caption: "Symmetry",
        hint: "Random output symmetrization each call: 2-Fold Point = {z,−z}, 2-Fold Mirror = {z,conj z}, 4-Fold = both.",
    },
};

const INFO = #{ title: "Hecke Group", name: "rhai_hecke_group", category: "advanced_2d" };

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

fn su_apply_plain(m, z) {
    su_cdiv(su_cmul(m[0], z) + m[1], su_cmul(m[2], z) + m[3])
}

fn hecke_gen(k, lam) {
    let t1 = k;
    if t1 == u32(0) {
        [complex(0.0, 0.0), complex(-1.0, 0.0), complex(1.0, 0.0), complex(0.0, 0.0)]
    } else if t1 == u32(1) {
        [complex(1.0, 0.0), complex(lam, 0.0), complex(0.0, 0.0), complex(1.0, 0.0)]
    } else {
        [complex(1.0, 0.0), complex(-lam, 0.0), complex(0.0, 0.0), complex(1.0, 0.0)]
    }
}

fn hecke_inverse_of(k) {
    if (k == u32(1)) {
        return u32(2);
    }
    if (k == u32(2)) {
        return u32(1);
    }
    u32(0)
}

fn su_qmul(a, b) {
    vec4(a.x * b.x - a.y * b.y - a.z * b.z - a.w * b.w, a.x * b.y + a.y * b.x + a.z * b.w - a.w * b.z, a.x * b.z - a.y * b.w + a.z * b.x + a.w * b.y, a.x * b.w + a.y * b.z - a.z * b.y + a.w * b.x)
}

fn su_qinv(a) {
    let n = (a.x * a.x + a.y * a.y + a.z * a.z + a.w * a.w) + 1.0e-30;
    vec4(a.x, -a.y, -a.z, -a.w) / n
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
    let q = max(3.0, to_float(p.q));
    let avoid = to_float(p.avoid_reversal) > 0.5;
    let dc_mode = u32(to_float(p.dc_mode));
    let dc_scale = p.dc_scale;
    let color_speed = p.color_speed;
    let lam = 2.0 * cos(3.14159265359 / q);
    let k = min(u32(rand() * 3.0), u32(2));
    if (avoid) {
        let prev = u32(this.st0);
        if (prev >= u32(1) && prev < u32(3) && k == hecke_inverse_of(prev)) {
            k = (k + u32(1)) % u32(3);
        }
    }
    this.st0 = to_float(k);
    if (dc_mode == u32(1)) {
        let creg = this.creg;
        creg = mix(creg, fract((to_float(k) + 0.5) / 3.0 * dc_scale), color_speed);
        this.creg = creg;
        color = creg;
    }
    let out = su_apply_plain(hecke_gen(k, lam), z);
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
    let q = max(3.0, to_float(p.q));
    let avoid = to_float(p.avoid_reversal) > 0.5;
    let dc_mode = u32(to_float(p.dc_mode));
    let dc_scale = p.dc_scale;
    let color_speed = p.color_speed;
    let lam = 2.0 * cos(3.14159265359 / q);
    let k = min(u32(rand() * 3.0), u32(2));
    if (avoid) {
        let prev = u32(this.st0);
        if (prev >= u32(1) && prev < u32(3) && k == hecke_inverse_of(prev)) {
            k = (k + u32(1)) % u32(3);
        }
    }
    this.st0 = to_float(k);
    if (dc_mode == u32(1)) {
        let creg = this.creg;
        creg = mix(creg, fract((to_float(k) + 0.5) / 3.0 * dc_scale), color_speed);
        this.creg = creg;
        color = creg;
    }
    let g = hecke_gen(k, lam);
    let space = u32(to_float(p.space));
    let out = vec3(0.0, 0.0, 0.0);
    if (space == u32(1)) {
        out = su_apply_plain3(g, v);
    } else {
        let t1 = su_apply_plain(g, complex(v.x, v.y));
        out = vec3(t1.re, t1.im, v.z);
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
