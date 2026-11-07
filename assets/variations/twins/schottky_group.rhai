/// Classical Schottky circle-pairing chaos game (Indra's Pearls Ch. 4).
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5

// Schottky Group, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    c1_x: #{
        value: -1.2, min: -4.0, max: 4.0, free: true,
        caption: "C1 x",
        hint: "Center x of circle C1 — generator a maps the exterior of C1 into the interior of C2. Keep the four circles disjoint for a free (dust) group; push them toward tangency for connected gasket-like limit sets.",
    },
    c1_y: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "C1 y",
        hint: "Center y of circle C1.",
    },
    r1: #{
        value: 0.8, min: 0.05, max: 3.0,
        caption: "R1",
        hint: "Radius of circle C1.",
    },
    c2_x: #{
        value: 1.2, min: -4.0, max: 4.0, free: true,
        caption: "C2 x",
        hint: "Center x of circle C2 (the image circle of generator a).",
    },
    c2_y: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "C2 y",
        hint: "Center y of circle C2.",
    },
    r2: #{
        value: 0.8, min: 0.05, max: 3.0,
        caption: "R2",
        hint: "Radius of circle C2.",
    },
    c3_x: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "C3 x",
        hint: "Center x of circle C3 — generator b maps the exterior of C3 into the interior of C4.",
    },
    c3_y: #{
        value: -1.2, min: -4.0, max: 4.0, free: true,
        caption: "C3 y",
        hint: "Center y of circle C3.",
    },
    r3: #{
        value: 0.8, min: 0.05, max: 3.0,
        caption: "R3",
        hint: "Radius of circle C3.",
    },
    c4_x: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "C4 x",
        hint: "Center x of circle C4 (the image circle of generator b).",
    },
    c4_y: #{
        value: 1.2, min: -4.0, max: 4.0, free: true,
        caption: "C4 y",
        hint: "Center y of circle C4.",
    },
    r4: #{
        value: 0.8, min: 0.05, max: 3.0,
        caption: "R4",
        hint: "Radius of circle C4.",
    },
    avoid_reversal: #{
        value: true,
        caption: "Avoid Reversal",
        hint: "Skip a generator's inverse immediately after applying it (no g·g⁻¹ cancellation) — the Indra's Pearls chaos-game rule.",
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
        hint: "3D render mode only. Euclidean: the Möbius orbit in the xy plane (z passes through). Hyperbolic H3: the Poincaré extension — the pairing circles become hemispheres acting on upper half-space. The t→0 slice is the 2D picture.",
    },
    symmetry: #{
        value: 0, choices: ["None", "2-Fold Point", "2-Fold Mirror", "4-Fold"],
        caption: "Symmetry",
        hint: "Random output symmetrization each call: 2-Fold Point = {z,−z}, 2-Fold Mirror = {z,conj z}, 4-Fold = both.",
    },
    twist_a: #{
        value: 180.0, min: -360.0, max: 360.0, type: "angle",
        caption: "Twist A",
        hint: "Rotation of generator a about C1 before pairing — the free 'marking' of a Schottky pairing (any rotation composed with a circle pairing still pairs the circles). It controls how the group glues at tangency: at the default square arrangement pushed to kissing (r ≈ 0.849), Twist A = 180 / Twist B = 0 gives tr[a,b] = −2 exactly — the parabolic gluing that condenses the dust into the connected θ-Schottky quasi-circle.",
    },
    twist_b: #{
        value: 0.0, min: -360.0, max: 360.0, type: "angle",
        caption: "Twist B",
        hint: "Rotation of generator b about C3 before pairing (see Twist A).",
    },
};

const INFO = #{ title: "Schottky Group", name: "rhai_schottky_group", category: "advanced_2d" };

fn csub(a, b) {
    a - b
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

fn prepare(p) {
    let out = array(52, 0.0);
    out[0] = p.c1_x;
    out[1] = p.c1_y;
    out[2] = p.r1;
    out[3] = p.c2_x;
    out[4] = p.c2_y;
    out[5] = p.r2;
    out[6] = p.c3_x;
    out[7] = p.c3_y;
    out[8] = p.r3;
    out[9] = p.c4_x;
    out[10] = p.c4_y;
    out[11] = p.r4;
    out[12] = to_float(p.avoid_reversal);
    out[13] = to_float(p.dc_mode);
    out[14] = p.dc_scale;
    out[15] = p.color_speed;
    out[16] = to_float(p.space);
    out[17] = to_float(p.symmetry);
    out[18] = p.twist_a;
    out[19] = p.twist_b;
    for pair in 0..2 {
        let o = pair * u32(6);
        let c1 = complex(out[to_int(o)], out[to_int(o + u32(1))]);
        let r1 = max(0.05, out[to_int(o + u32(2))]);
        let c2 = complex(out[to_int(o + u32(3))], out[to_int(o + u32(4))]);
        let r2 = max(0.05, out[to_int(o + u32(5))]);
        let ma = c2;
        let mb = complex(r1 * r2, 0.0) - cmul(c1, c2);
        let mc = complex(1.0, 0.0);
        let md = -c1;
        let phi = out[to_int(u32(18) + pair)] * 0.01745329252;
        let e = complex(cos(phi), sin(phi));
        let rb = cmul(c1, complex(1.0, 0.0) - e);
        let ta = cmul(ma, e);
        let tb = cmul(ma, rb) + mb;
        let tc = cmul(mc, e);
        let td = cmul(mc, rb) + md;
        let sd = csqrt(csub(cmul(ta, td), cmul(tb, tc)));
        let na = cdiv(ta, sd);
        let nb = cdiv(tb, sd);
        let nc = cdiv(tc, sd);
        let nd = cdiv(td, sd);
        let ob = pair * u32(8);
        let oi = u32(16) + pair * u32(8);
        out[20 + to_int(ob)] = na.re;
        out[20 + to_int(ob + u32(1))] = na.im;
        out[20 + to_int(ob + u32(2))] = nb.re;
        out[20 + to_int(ob + u32(3))] = nb.im;
        out[20 + to_int(ob + u32(4))] = nc.re;
        out[20 + to_int(ob + u32(5))] = nc.im;
        out[20 + to_int(ob + u32(6))] = nd.re;
        out[20 + to_int(ob + u32(7))] = nd.im;
        out[20 + to_int(oi)] = nd.re;
        out[20 + to_int(oi + u32(1))] = nd.im;
        out[20 + to_int(oi + u32(2))] = -nb.re;
        out[20 + to_int(oi + u32(3))] = -nb.im;
        out[20 + to_int(oi + u32(4))] = -nc.re;
        out[20 + to_int(oi + u32(5))] = -nc.im;
        out[20 + to_int(oi + u32(6))] = na.re;
        out[20 + to_int(oi + u32(7))] = na.im;
    }
    #{ slots: out }
}

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

fn schottky_read(p, k) {
    let bo = u32(20) + k * u32(8);
    [complex(p.slots[to_int(bo)], p.slots[to_int(bo + u32(1))]), complex(p.slots[to_int(bo + u32(2))], p.slots[to_int(bo + u32(3))]), complex(p.slots[to_int(bo + u32(4))], p.slots[to_int(bo + u32(5))]), complex(p.slots[to_int(bo + u32(6))], p.slots[to_int(bo + u32(7))])]
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
    let avoid = to_float(p.avoid_reversal) > 0.5;
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
    let out = su_apply_plain(schottky_read(p, k), z);
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
    let g = schottky_read(p, k);
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
