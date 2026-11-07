/// Five SL(2,ℂ) Lorentz generators as a Möbius chaos game (the "Dirac
/// five" set).
///
/// # Authors
/// - Roger Bagula
/// - Fractals for All
/// - Claude Fable 5

// Lorentz Möbius, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    theta: #{
        value: 1.5, min: -4.0, max: 4.0, free: true,
        caption: "Theta",
        hint: "Magnitude of the θ-slot generators (the notebook's 'rotation z/y/x' trio g3–g5). These exponentiate σ/2, so as Möbius maps they are HYPERBOLIC — boost-like squeezes toward antipodal fixed points (the notebook's labels are the reverse of the physics convention). Notebook value: 1.",
    },
    eta: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "Eta",
        hint: "Magnitude of the η-slot generators (the notebook's 'boost ±z' pair g1–g2). These exponentiate iσ/2, so as Möbius maps they are ELLIPTIC — rotations about the ±z celestial poles by η radians. Notebook value: 1.",
    },
    avoid_reversal: #{
        value: true,
        caption: "Avoid Reversal",
        hint: "Skip a generator's inverse immediately after applying it (no g·g⁻¹ cancellation; the inverse of generator k is k+5). Off = all ten elements equally likely every call.",
    },
    dc_mode: #{
        value: 0, choices: ["Off", "Generator"],
        caption: "Color Mode",
        hint: "Direct-color source (needs the transform's Direct Color > 0). Generator: each of the ten group elements has its own palette position, blended through a persistent color register at Color Speed.",
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
        hint: "3D render mode only. Euclidean: the celestial-sphere Möbius action in the xy plane (z passes through). Hyperbolic H3: the Poincaré extension — the same matrices act on upper-half-space (SL(2,C) = Isom(H³)). The t→0 slice is the 2D picture.",
    },
    symmetry: #{
        value: 0, choices: ["None", "2-Fold Point", "2-Fold Mirror", "4-Fold"],
        caption: "Symmetry",
        hint: "Random output symmetrization each call: 2-Fold Point = {z,−z}, 2-Fold Mirror = {z,conj z}, 4-Fold = both.",
    },
};

const INFO = #{ title: "Lorentz Möbius", name: "rhai_lorentz_mobius", category: "advanced_2d" };

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

fn lorentz_gen(k, theta, eta) {
    let base = k % u32(5);
    let sgn = (if k >= u32(5) { -1.0 } else { 1.0 });
    if (base == u32(0) || base == u32(1)) {
        let h = eta * 0.5 * sgn;
        if (base == u32(1)) {
            h = -h;
        }
        let c = cos(h);
        let s = sin(h);
        [complex(c, s), complex(0.0, 0.0), complex(0.0, 0.0), complex(c, -s)]
    } else {
        let h = theta * 0.5 * sgn;
        let ch = cosh(h);
        let sh = sinh(h);
        if (base == u32(2)) {
            [complex(ch + sh, 0.0), complex(0.0, 0.0), complex(0.0, 0.0), complex(ch - sh, 0.0)]
        } else if (base == u32(3)) {
            [complex(ch, 0.0), complex(0.0, -sh), complex(0.0, sh), complex(ch, 0.0)]
        } else {
            [complex(ch, 0.0), complex(sh, 0.0), complex(sh, 0.0), complex(ch, 0.0)]
        }
    }
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
    let theta = p.theta;
    let eta = p.eta;
    let avoid = to_float(p.avoid_reversal) > 0.5;
    let dc_mode = u32(to_float(p.dc_mode));
    let dc_scale = p.dc_scale;
    let color_speed = p.color_speed;
    let k = min(u32(rand() * 10.0), u32(9));
    if (avoid) {
        let prev = u32(this.st0);
        if (prev < u32(10) && k == (prev + u32(5)) % u32(10)) {
            k = (k + u32(1)) % u32(10);
        }
    }
    this.st0 = to_float(k);
    if (dc_mode == u32(1)) {
        let creg = this.creg;
        creg = mix(creg, fract((to_float(k) + 0.5) / 10.0 * dc_scale), color_speed);
        this.creg = creg;
        color = creg;
    }
    let out = su_apply_plain(lorentz_gen(k, theta, eta), z);
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
    let theta = p.theta;
    let eta = p.eta;
    let avoid = to_float(p.avoid_reversal) > 0.5;
    let dc_mode = u32(to_float(p.dc_mode));
    let dc_scale = p.dc_scale;
    let color_speed = p.color_speed;
    let k = min(u32(rand() * 10.0), u32(9));
    if (avoid) {
        let prev = u32(this.st0);
        if (prev < u32(10) && k == (prev + u32(5)) % u32(10)) {
            k = (k + u32(1)) % u32(10);
        }
    }
    this.st0 = to_float(k);
    if (dc_mode == u32(1)) {
        let creg = this.creg;
        creg = mix(creg, fract((to_float(k) + 0.5) / 10.0 * dc_scale), color_speed);
        this.creg = creg;
        color = creg;
    }
    let g = lorentz_gen(k, theta, eta);
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
