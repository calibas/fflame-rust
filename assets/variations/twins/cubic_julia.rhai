/// Julia sets of cubic polynomials via 3-branch (Cardano) inverse
/// iteration.
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5

// Cubic Julia, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    mode: #{
        value: 0, choices: ["Inverse (Julia)", "Forward"],
        caption: "Mode",
        hint: "Inverse: chaos game over the three closed-form inverse branches of f(z) = z³ + a·z + b (random Cardano root per call) — converges onto the Julia set of f. Forward: f itself — expanding, blend with contracting affines.",
    },
    a_re: #{
        value: -1.0, min: -3.0, max: 3.0, free: true,
        caption: "A re",
        hint: "Real part of the linear coefficient a in f(z) = z³ + a·z + b. Every cubic is affinely conjugate to this depressed form, so a and b cover the whole cubic family (the flame's affines supply the conjugation). a controls the two critical points ±√(−a/3).",
    },
    a_im: #{
        value: 0.0, min: -3.0, max: 3.0, free: true,
        caption: "A im",
        hint: "Imaginary part of a.",
    },
    b_re: #{
        value: 0.25, min: -3.0, max: 3.0, free: true,
        caption: "B re",
        hint: "Real part of the constant b — the Julia parameter (the c of z²+c, one degree up). Small |b| keeps the set connected-ish; large |b| shatters it into dust.",
    },
    b_im: #{
        value: 0.0, min: -3.0, max: 3.0, free: true,
        caption: "B im",
        hint: "Imaginary part of b.",
    },
    w1: #{
        value: 1.0, min: 0.0, max: 1.0,
        caption: "Branch 1",
        hint: "Selection weight of inverse branch 1 (Cardano root ω⁰). Biasing or zeroing branch weights selects sub-IFS attractors — genuinely different sub-self-similar subsets of the Julia set.",
    },
    w2: #{
        value: 1.0, min: 0.0, max: 1.0,
        caption: "Branch 2",
        hint: "Selection weight of inverse branch 2 (Cardano root ω¹).",
    },
    w3: #{
        value: 1.0, min: 0.0, max: 1.0,
        caption: "Branch 3",
        hint: "Selection weight of inverse branch 3 (Cardano root ω²).",
    },
    dc_mode: #{
        value: 0, choices: ["Off", "Branch", "Branch Blend", "Log Deriv"],
        caption: "Color Mode",
        hint: "Direct-color source (needs the transform's Direct Color > 0). Branch: which inverse sheet was taken this call. Branch Blend: persistent register pulled toward each branch's palette slot — colors the Julia set by IFS itinerary (symbolic address), segmenting its self-similar pieces. Log Deriv: log|f′(z)| = log|3z²+a| at the new point — the local contraction rate, a smooth Lyapunov-style gradient.",
    },
    dc_scale: #{
        value: 1.0, min: 0.1, max: 8.0,
        caption: "Color Scale",
        hint: "Palette-index multiplier for the color modes.",
    },
    color_speed: #{
        value: 0.3, min: 0.01, max: 1.0,
        caption: "Color Speed",
        hint: "Blend rate of the persistent register in Branch Blend: low = deep itinerary history (coarse self-similar pieces), high = recent branches only (fine detail).",
    },
};

const INFO = #{ title: "Cubic Julia", name: "rhai_cubic_julia", category: "advanced_2d", z: "preserve_z" };

fn init(p) {
    #{ creg: 0.0 }
}

fn cmul(a, b) {
    complex(a.re * b.re - a.im * b.im, a.re * b.im + a.im * b.re)
}

fn cmag2(a) {
    a.re * a.re + a.im * a.im
}

fn cconj(a) {
    complex(a.re, -a.im)
}

fn cdiv(a, b) {
    let denom = cmag2(b);
    let safe_denom = (if denom < 1.0e-30 { 1.0e-30 } else { denom });
    cmul(a, cconj(b)) / safe_denom
}

fn csqrt(a) {
    let mag = sqrt(cmag2(a));
    let real_part = sqrt(max(0.0, 0.5 * (mag + a.re)));
    let imag_mag = sqrt(max(0.0, 0.5 * (mag - a.re)));
    let imag_part = (if a.im >= 0.0 { imag_mag } else { -imag_mag });
    complex(real_part, imag_part)
}

fn cjulia_cbrt(z) {
    let r = z.abs();
    if (r < 1.0e-30) {
        return complex(0.0, 0.0);
    }
    let th = atan2(z.im, z.re) / 3.0;
    pow(r, 0.3333333333) * complex(cos(th), sin(th))
}

fn cjulia_inverse(z, a, b, k) {
    let q = b - z;
    if (a.norm_sqr() < 1.0e-24 && q.norm_sqr() < 1.0e-24) {
        return complex(0.0, 0.0);
    }
    let D = csqrt(0.25 * cmul(q, q) + cmul(cmul(a, a), a) / 27.0);
    let w = cjulia_cbrt(-0.5 * q + D);
    if (w.norm_sqr() < 1.0e-16) {
        w = cjulia_cbrt(-0.5 * q - D);
    }
    let om = complex(1.0, 0.0);
    if (k == u32(1)) {
        om = complex(-0.5, 0.86602540);
    } else if (k == u32(2)) {
        om = complex(-0.5, -0.86602540);
    }
    let wk = cmul(w, om);
    if (wk.norm_sqr() < 1.0e-24) {
        return complex(0.0, 0.0);
    }
    wk - cdiv(a, 3.0 * wk)
}

fn cjulia_pick(r, w1, w2, w3) {
    let tot = max(1.0e-9, w1 + w2 + w3);
    let x = r * tot;
    if (x < w1) {
        return u32(0);
    }
    if (x < w1 + w2) {
        return u32(1);
    }
    u32(2)
}

fn variation(z, p, ctx) {
    let color = ctx.color;
    let mode = u32(to_float(p.mode));
    let a = complex(p.a_re, p.a_im);
    let b = complex(p.b_re, p.b_im);
    let w1 = p.w1;
    let w2 = p.w2;
    let w3 = p.w3;
    let dc_mode = u32(to_float(p.dc_mode));
    let dc_scale = p.dc_scale;
    let cspeed = p.color_speed;
    let out = complex(0.0, 0.0);
    let k = u32(0);
    if (mode == u32(1)) {
        out = cmul(cmul(z, z), z) + cmul(a, z) + b;
    } else {
        k = cjulia_pick(rand(), w1, w2, w3);
        out = cjulia_inverse(z, a, b, k);
    }
    if (dc_mode == u32(1)) {
        color = fract((to_float(k) + 0.5) / 3.0 * dc_scale);
    } else if (dc_mode == u32(2)) {
        let creg = this.creg;
        creg = mix(creg, (to_float(k) + 0.5) / 3.0, cspeed);
        this.creg = creg;
        color = fract(creg * dc_scale);
    } else if (dc_mode == u32(3)) {
        let d = 3.0 * cmul(out, out) + a;
        color = fract(0.5 + 0.25 * dc_scale * ln(max(1.0e-20, d.abs())));
    }
    #{ z: out, color: color }
}

fn variation3(v, p, ctx) {
    let color = ctx.color;
    let mode = u32(to_float(p.mode));
    let a = complex(p.a_re, p.a_im);
    let b = complex(p.b_re, p.b_im);
    let w1 = p.w1;
    let w2 = p.w2;
    let w3 = p.w3;
    let dc_mode = u32(to_float(p.dc_mode));
    let dc_scale = p.dc_scale;
    let cspeed = p.color_speed;
    let out = complex(0.0, 0.0);
    let k = u32(0);
    if (mode == u32(1)) {
        out = cmul(cmul(complex(v.x, v.y), complex(v.x, v.y)), complex(v.x, v.y)) + cmul(a, complex(v.x, v.y)) + b;
    } else {
        k = cjulia_pick(rand(), w1, w2, w3);
        out = cjulia_inverse(complex(v.x, v.y), a, b, k);
    }
    if (dc_mode == u32(1)) {
        color = fract((to_float(k) + 0.5) / 3.0 * dc_scale);
    } else if (dc_mode == u32(2)) {
        let creg = this.creg;
        creg = mix(creg, (to_float(k) + 0.5) / 3.0, cspeed);
        this.creg = creg;
        color = fract(creg * dc_scale);
    } else if (dc_mode == u32(3)) {
        let d = 3.0 * cmul(out, out) + a;
        color = fract(0.5 + 0.25 * dc_scale * ln(max(1.0e-20, d.abs())));
    }
    #{ z: vec3(out.re, out.im, v.z), color: color }
}
