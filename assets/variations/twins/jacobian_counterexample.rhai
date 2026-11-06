/// Inverse-branch IFS of the 2026 Jacobian-conjecture counterexample
/// map (closed-form 3-sheet inverse).
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5

// Jacobian Counterexample, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    mode: #{
        value: 0, choices: ["Inverse IFS", "Forward"],
        caption: "Mode",
        hint: "Inverse IFS: chaos game over the three closed-form inverse branches (random Cardano root per call) — converges onto the fractal attractor, each branch contracting area by 1/|2c²|. Forward: the degree-4/6 endomorphism itself — expanding, so blend it with contracting affines like any polynomial variation.",
    },
    scale: #{
        value: 1.0, min: 0.05, max: 4.0,
        caption: "Scale",
        hint: "Similarity conjugation: world point = (math point − center) × scale.",
    },
    center_x: #{
        value: -1.0, min: -4.0, max: 4.0, free: true,
        caption: "Center re",
        hint: "Real part of the view center in math coordinates, subtracted before plotting. The attractor rim is pinned at a = −1 (a structural rigidity of the map), so the default centers it on the origin.",
    },
    center_y: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "Center im",
        hint: "Imaginary part of the view center.",
    },
    steps: #{
        value: 1, min: 1, max: 8,
        caption: "Steps",
        hint: "Inverse applications per call (fresh random branch each). Higher values pull stray trajectories onto the attractor harder — sharpens the halo the 2D hidden-fiber desync paints — at proportional GPU cost. Forward mode: leave at 1 (degree 6 per step compounds fast).",
    },
    w1: #{
        value: 1.0, min: 0.0, max: 1.0,
        caption: "Branch 1",
        hint: "Selection weight of inverse branch 1 (Cardano root ω⁰). Biasing or zeroing branch weights selects sub-semigroups of the inverse monoid — genuinely different sub-self-similar limit sets. The strongest shape knob this map has (the map itself has no free constants).",
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
    julia_a_re: #{
        value: 0.0, min: -8.0, max: 8.0, free: true,
        caption: "Julia A re",
        hint: "Julia offset on the a-equation: the chaos game inverts G + j instead of G (the z²+c pattern — the fiber cubic's target just shifts, so the closed form survives). The rim stays anchored at a = −1 (structural rigidity of the map) but its internal texture deforms.",
    },
    julia_a_im: #{
        value: 0.0, min: -8.0, max: 8.0, free: true,
        caption: "Julia A im",
        hint: "Imaginary part of the a-offset.",
    },
    julia_b_re: #{
        value: 0.0, min: -8.0, max: 8.0, free: true,
        caption: "Julia B re",
        hint: "Julia offset on the b-equation — the stronger of the two: large values transform the thin rim into thick gear-toothed annuli with holes punched through (try −8), and move the fiber attractor visibly in 3D z/w and Fiber Phase coloring.",
    },
    julia_b_im: #{
        value: 0.0, min: -8.0, max: 8.0, free: true,
        caption: "Julia B im",
        hint: "Imaginary part of the b-offset.",
    },
    dc_mode: #{
        value: 0, choices: ["Off", "Log C", "Branch", "Branch Blend", "Lyapunov", "Fiber Phase"],
        caption: "Color Mode",
        hint: "Direct-color source (needs the transform's Direct Color > 0). Log C: instantaneous log|c| (the Birkhoff cocycle of the skew coordinate; local contraction rate det DG = 2c²). Branch: which branch was taken this call. Branch Blend: persistent register pulled toward each branch's palette slot — colors the attractor by IFS itinerary (symbolic address), revealing the self-similar pieces. Lyapunov: running average of log|c| — smooth contraction-rate gradient along the attractor. Fiber Phase: arg(b), the hidden fiber dimension the a-plane projection discards.",
    },
    dc_scale: #{
        value: 1.0, min: 0.1, max: 8.0,
        caption: "Color Scale",
        hint: "Palette-index multiplier for the color modes.",
    },
    color_speed: #{
        value: 0.3, min: 0.01, max: 1.0,
        caption: "Color Speed",
        hint: "Blend rate of the persistent register in Branch Blend / Lyapunov: low = deep itinerary history (coarse self-similar pieces), high = recent branches only (fine detail).",
    },
};

const INFO = #{ title: "Jacobian Counterexample", name: "rhai_jacobian_counterexample", category: "advanced_2d", slow: ["steps"] };

fn init(p) {
    let r1 = rand();
    let r2 = rand();
    #{ st0: (r1 - 0.5) * 0.6, st1: (r2 - 0.5) * 0.6, creg: 0.5 }
}

fn cmul(a, b) {
    complex(a.re * b.re - a.im * b.im, a.re * b.im + a.im * b.re)
}

fn cconj(a) {
    complex(a.re, -a.im)
}

fn cmag2(a) {
    a.re * a.re + a.im * a.im
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

fn jcubic_cbrt(z) {
    let r = z.abs();
    if (r < 1.0e-30) {
        return complex(0.0, 0.0);
    }
    let th = atan2(z.im, z.re) / 3.0;
    let rc = pow(r, 0.3333333333);
    rc * complex(cos(th), sin(th))
}

fn jcubic_v(a, b) {
    let one = complex(1.0, 0.0);
    let ap1 = a + one;
    cmul(cmul(ap1, ap1), b) + cmul(cmul(a, a), 3.0 * a + complex(4.0, 0.0))
}

fn jcubic_e2res(a, b, B) {
    let c = complex(2.0, 0.0) - 3.0 * a - b;
    let e = cmul(cmul(c, c), cmul(a + complex(1.0, 0.0), jcubic_v(a, b))) - B;
    e.norm_sqr()
}

fn jcubic_pick(r, w1, w2, w3) {
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

fn jcubic_map(ab, mode, k, jul) {
    let one = complex(1.0, 0.0);
    if (mode == u32(1)) {
        let c = complex(2.0, 0.0) - 3.0 * complex(ab.x, ab.y) - complex(ab.z, ab.w);
        let V = jcubic_v(complex(ab.x, ab.y), complex(ab.z, ab.w));
        let a2 = cmul(c, complex(ab.x, ab.y) + 3.0 * V) + complex(jul.x, jul.y);
        let b2 = cmul(cmul(c, c), cmul(complex(ab.x, ab.y) + one, V)) + complex(jul.z, jul.w);
        return vec4(a2.re, a2.im, b2.re, b2.im);
    }
    let A = complex(ab.x, ab.y) - complex(jul.x, jul.y);
    let B = complex(ab.z, ab.w) - complex(jul.z, jul.w);
    let A2 = cmul(A, A);
    let A3 = cmul(A2, A);
    let K = A3 - A2 - 18.0 * cmul(A, B) + 27.0 * cmul(B, B) + 16.0 * B;
    let M = A2 - 12.0 * B;
    let u = complex(0.0, 0.0);
    if (K.norm_sqr() < 1.0e-24) {
        if (M.norm_sqr() < 1.0e-24) {
            u = complex(0.0, 0.0);
        } else {
            u = cdiv(4.0 * B, M);
        }
    } else {
        let p = cdiv(M, K);
        let q = cdiv(-4.0 * B, K);
        let D = csqrt(0.25 * cmul(q, q) + cmul(cmul(p, p), p) / 27.0);
        let w = jcubic_cbrt(-0.5 * q + D);
        if (w.norm_sqr() < 1.0e-16) {
            w = jcubic_cbrt(-0.5 * q - D);
        }
        let om = complex(1.0, 0.0);
        if (k == u32(1)) {
            om = complex(-0.5, 0.86602540);
        } else if (k == u32(2)) {
            om = complex(-0.5, -0.86602540);
        }
        let wk = cmul(w, om);
        if (wk.norm_sqr() < 1.0e-24) {
            u = complex(0.0, 0.0);
        } else {
            u = wk - cdiv(p, 3.0 * wk);
        }
    }
    let a = u - one;
    let aa = cmul(a, a);
    let s = a + 12.0 * aa + 9.0 * cmul(aa, a);
    let t = 3.0 * cmul(a + one, a + one);
    let tm = complex(2.0, 0.0) - 3.0 * a;
    let q1c = cmul(tm, t) - s;
    let q0c = cmul(tm, s) - A;
    let b = complex(0.0, 0.0);
    if (t.norm_sqr() < 1.0e-24) {
        b = cdiv(-q0c, q1c);
    } else {
        let disc = csqrt(cmul(q1c, q1c) + 4.0 * cmul(t, q0c));
        let b1 = cdiv(-q1c + disc, -2.0 * t);
        let b2 = cdiv(-q1c - disc, -2.0 * t);
        b = (if jcubic_e2res(a, b1, B) < jcubic_e2res(a, b2, B) { b1 } else { b2 });
    }
    vec4(a.re, a.im, b.re, b.im)
}

fn variation(q, p, ctx) {
    let color = ctx.color;
    let mode = u32(to_float(p.mode));
    let scale = max(1.0e-6, p.scale);
    let ctr = complex(p.center_x, p.center_y);
    let steps = to_int(to_float(p.steps));
    let w1 = p.w1;
    let w2 = p.w2;
    let w3 = p.w3;
    let jul = vec4(p.julia_a_re, p.julia_a_im, p.julia_b_re, p.julia_b_im);
    let dc_mode = u32(to_float(p.dc_mode));
    let dc_scale = p.dc_scale;
    let cspeed = p.color_speed;
    let t1 = q / scale + ctr;
    let t2 = complex(this.st0, this.st1);
    let ab = vec4(t1.re, t1.im, t2.re, t2.im);
    let creg = this.creg;
    let k = u32(0);
    for i in 0..steps {
        k = jcubic_pick(rand(), w1, w2, w3);
        ab = jcubic_map(ab, mode, k, jul);
        if (!((ab.x * ab.x + ab.y * ab.y + ab.z * ab.z + ab.w * ab.w) < 1.0e12)) {
            let t3 = complex(ab.x, ab.y) * 0.0;
            ab = vec4(t3.re, t3.im, (rand() - 0.5) * 0.6, (rand() - 0.5) * 0.6);
        }
        if (dc_mode == u32(3)) {
            creg = mix(creg, (to_float(k) + 0.5) / 3.0, cspeed);
        } else if (dc_mode == u32(4)) {
            let cc = complex(2.0, 0.0) - 3.0 * complex(ab.x, ab.y) - complex(ab.z, ab.w);
            creg = mix(creg, clamp(ln(max(1.0e-20, cc.abs())), -4.0, 4.0), cspeed);
        }
    }
    this.st0 = ab.z;
    this.st1 = ab.w;
    if (dc_mode == u32(1)) {
        let c = complex(2.0, 0.0) - 3.0 * complex(ab.x, ab.y) - complex(ab.z, ab.w);
        color = fract(0.5 + 0.25 * dc_scale * ln(max(1.0e-20, c.abs())));
    } else if (dc_mode == u32(2)) {
        color = fract((to_float(k) + 0.5) / 3.0 * dc_scale);
    } else if (dc_mode == u32(3)) {
        this.creg = creg;
        color = fract(creg * dc_scale);
    } else if (dc_mode == u32(4)) {
        this.creg = creg;
        color = fract(0.5 + 0.25 * dc_scale * creg);
    } else if (dc_mode == u32(5)) {
        color = fract((atan2(ab.w, ab.z) * 0.15915494 + 0.5) * dc_scale);
    }
    #{ z: (complex(ab.x, ab.y) - ctr) * scale, color: color }
}

fn variation3(v, p, ctx) {
    let color = ctx.color;
    let w_out = 0.0;
    let mode = u32(to_float(p.mode));
    let scale = max(1.0e-6, p.scale);
    let ctr = complex(p.center_x, p.center_y);
    let steps = to_int(to_float(p.steps));
    let w1 = p.w1;
    let w2 = p.w2;
    let w3 = p.w3;
    let jul = vec4(p.julia_a_re, p.julia_a_im, p.julia_b_re, p.julia_b_im);
    let dc_mode = u32(to_float(p.dc_mode));
    let dc_scale = p.dc_scale;
    let cspeed = p.color_speed;
    let t1 = complex(v.x, v.y) / scale + ctr;
    let t2 = complex(v.z, ctx.w) / scale;
    let ab = vec4(t1.re, t1.im, t2.re, t2.im);
    let creg = this.creg;
    let k = u32(0);
    for i in 0..steps {
        k = jcubic_pick(rand(), w1, w2, w3);
        ab = jcubic_map(ab, mode, k, jul);
        if (!((ab.x * ab.x + ab.y * ab.y + ab.z * ab.z + ab.w * ab.w) < 1.0e12)) {
            let t3 = complex(ab.x, ab.y) * 0.0;
            ab = vec4(t3.re, t3.im, (rand() - 0.5) * 0.6, (rand() - 0.5) * 0.6);
        }
        if (dc_mode == u32(3)) {
            creg = mix(creg, (to_float(k) + 0.5) / 3.0, cspeed);
        } else if (dc_mode == u32(4)) {
            let cc = complex(2.0, 0.0) - 3.0 * complex(ab.x, ab.y) - complex(ab.z, ab.w);
            creg = mix(creg, clamp(ln(max(1.0e-20, cc.abs())), -4.0, 4.0), cspeed);
        }
    }
    if (dc_mode == u32(1)) {
        let c = complex(2.0, 0.0) - 3.0 * complex(ab.x, ab.y) - complex(ab.z, ab.w);
        color = fract(0.5 + 0.25 * dc_scale * ln(max(1.0e-20, c.abs())));
    } else if (dc_mode == u32(2)) {
        color = fract((to_float(k) + 0.5) / 3.0 * dc_scale);
    } else if (dc_mode == u32(3)) {
        this.creg = creg;
        color = fract(creg * dc_scale);
    } else if (dc_mode == u32(4)) {
        this.creg = creg;
        color = fract(0.5 + 0.25 * dc_scale * creg);
    } else if (dc_mode == u32(5)) {
        color = fract((atan2(ab.w, ab.z) * 0.15915494 + 0.5) * dc_scale);
    }
    w_out = ab.w * scale;
    let t4 = (complex(ab.x, ab.y) - ctr) * scale;
    #{ z: vec3(t4.re, t4.im, ab.z * scale), color: color, w: w_out }
}
