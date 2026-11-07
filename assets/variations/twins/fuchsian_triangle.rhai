/// Bagula's ⟨2,3,12⟩ Kleinian triangle-group limit set
/// (McMullen/Nylander).
///
/// # Authors
/// - Roger Bagula
/// - Fractals for All
/// - Claude Fable 5

// Fuchsian Triangle, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    avoid_reversal: #{
        value: true,
        caption: "Avoid Reversal",
        hint: "Skip a generator's inverse immediately after applying it (no g·g⁻¹ cancellation), so the orbit drifts through the limit set instead of dithering on a small subset (Nylander's backtrack-avoid). Off = all six generators equally likely every call.",
    },
    tri_trace: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "Tri Trace",
        hint: "Trace of the first generator s1 = [[t,-1],[1,0]]. |t| > 2 is hyperbolic (Bagula's 3 — spreads the elliptic circles fractally), t = 2 parabolic, t = 2·cos(π/n) elliptic of order n.",
    },
    tri_q: #{
        value: 7, min: 1, max: 24,
        caption: "Tri Order Q",
        hint: "Order of the second generator s2 = [[e^{2πi/q}, shear],[0, e^{-2πi/q}]] — an elliptic rotation of order q (Bagula's 3). Its orbits paint the mid-scale circles.",
    },
    tri_r: #{
        value: 8, min: 1, max: 24,
        caption: "Tri Order R",
        hint: "Order of the third generator s3 — the elliptic rotation by 2π/r fixing ±1 (Bagula's 12, the ⟨2,3,12⟩ 30° angle). Its orbits paint the fine circle rings.",
    },
    tri_shear: #{
        value: -4.0, min: -4.0, max: 4.0, free: true,
        caption: "Tri Shear",
        hint: "The off-diagonal shear in s2 (Bagula's 3). Pushes s2's two fixed points apart — 0 makes s2 a pure diagonal rotation about 0/∞.",
    },
    dc_mode: #{
        value: 0, choices: ["Off", "Generator"],
        caption: "Color Mode",
        hint: "Direct-color source (needs the transform's Direct Color > 0). Generator: each of the six group elements has its own palette position, blended through a persistent color register at Color Speed.",
    },
    dc_scale: #{
        value: 1.0, min: 0.1, max: 8.0,
        caption: "Color Scale",
        hint: "Palette-index multiplier for the Generator color (wrapped with fract).",
    },
    color_speed: #{
        value: 0.5, min: 0.0, max: 1.0,
        caption: "Color Speed",
        hint: "How hard each generator pulls the color register toward its own palette position. Low = long blends of orbit history, 1 = hard per-generator assignment.",
    },
    space: #{
        value: 0, choices: ["Euclidean", "Hyperbolic H3"],
        caption: "Space",
        hint: "3D render mode only. Euclidean: the ordinary complex Möbius map in the xy plane (z passes through) — the flat swiss-cheese packing. Hyperbolic H3: the Poincaré extension — SL(2,C) is exactly the isometry group of hyperbolic 3-space, so each generator acts on upper-half-space (the point as a quaternion x+yi+t·j, t = height) via (Aq+B)(Cq+D)⁻¹, filling the packing out to a 3D sphere-packing. The z-slice at t→0 is the 2D picture.",
    },
    symmetry: #{
        value: 0, choices: ["None", "2-Fold Point", "2-Fold Mirror", "4-Fold"],
        caption: "Symmetry",
        hint: "4-Fold applies a random one of {z, −z, conj z, −conj z} to the output each call — 2-Fold Point = {z,−z} (180° rotation), 2-Fold Mirror = {z,conj z} (reflection), 4-Fold = both. Symmetrizes the orbit into an evenly 4-fold packing.",
    },
};

const INFO = #{ title: "Fuchsian Triangle", name: "rhai_fuchsian_triangle", category: "advanced_2d" };

fn prepare(p) {
    let out = array(58, 0.0);
    out[0] = to_float(p.avoid_reversal);
    out[1] = p.tri_trace;
    out[2] = to_float(p.tri_q);
    out[3] = to_float(p.tri_r);
    out[4] = p.tri_shear;
    out[5] = to_float(p.dc_mode);
    out[6] = p.dc_scale;
    out[7] = p.color_speed;
    out[8] = to_float(p.space);
    out[9] = to_float(p.symmetry);
    let pi = 3.14159265359;
    let t1 = p.tri_trace;
    let aq = 2.0 * pi / max(1.0, to_float(p.tri_q));
    let ar = 2.0 * pi / max(1.0, to_float(p.tri_r));
    let sh = p.tri_shear;
    let ga = array(3, complex(0.0, 0.0));
    let gb = array(3, complex(0.0, 0.0));
    let gc = array(3, complex(0.0, 0.0));
    let gd = array(3, complex(0.0, 0.0));
    ga[0] = complex(t1, 0.0);
    gb[0] = complex(-1.0, 0.0);
    gc[0] = complex(1.0, 0.0);
    gd[0] = complex(0.0, 0.0);
    ga[1] = complex(cos(aq), sin(aq));
    gb[1] = complex(sh, 0.0);
    gc[1] = complex(0.0, 0.0);
    gd[1] = complex(cos(aq), -sin(aq));
    ga[2] = complex(cos(ar), 0.0);
    gb[2] = complex(0.0, sin(ar));
    gc[2] = complex(0.0, sin(ar));
    gd[2] = complex(cos(ar), 0.0);
    for i_n in 0..3 {
        let i = u32(i_n);
        let na = ga[i];
        let nb = gb[i];
        let nc = gc[i];
        let nd = gd[i];
        let o = i * u32(8);
        let oi = u32(24) + i * u32(8);
        out[10 + to_int(o)] = na.re;
        out[10 + to_int(o + u32(1))] = na.im;
        out[10 + to_int(o + u32(2))] = nb.re;
        out[10 + to_int(o + u32(3))] = nb.im;
        out[10 + to_int(o + u32(4))] = nc.re;
        out[10 + to_int(o + u32(5))] = nc.im;
        out[10 + to_int(o + u32(6))] = nd.re;
        out[10 + to_int(o + u32(7))] = nd.im;
        out[10 + to_int(oi)] = nd.re;
        out[10 + to_int(oi + u32(1))] = nd.im;
        out[10 + to_int(oi + u32(2))] = -nb.re;
        out[10 + to_int(oi + u32(3))] = -nb.im;
        out[10 + to_int(oi + u32(4))] = -nc.re;
        out[10 + to_int(oi + u32(5))] = -nc.im;
        out[10 + to_int(oi + u32(6))] = na.re;
        out[10 + to_int(oi + u32(7))] = na.im;
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

fn fuchsian_triangle_read(p, k) {
    let bo = u32(10) + k * u32(8);
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
    let cnt = u32(6);
    let half = u32(3);
    let k = min(u32(rand() * to_float(cnt)), cnt - u32(1));
    if (avoid) {
        let prev = u32(this.st0);
        if (prev < cnt && k == (prev + half) % cnt) {
            k = (k + u32(1)) % cnt;
        }
    }
    this.st0 = to_float(k);
    if (dc_mode == u32(1)) {
        let creg = this.creg;
        creg = mix(creg, fract((to_float(k) + 0.5) / to_float(cnt) * dc_scale), color_speed);
        this.creg = creg;
        color = creg;
    }
    let out = su_apply_plain(fuchsian_triangle_read(p, k), z);
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
    let cnt = u32(6);
    let half = u32(3);
    let k = min(u32(rand() * to_float(cnt)), cnt - u32(1));
    if (avoid) {
        let prev = u32(this.st0);
        if (prev < cnt && k == (prev + half) % cnt) {
            k = (k + u32(1)) % cnt;
        }
    }
    this.st0 = to_float(k);
    if (dc_mode == u32(1)) {
        let creg = this.creg;
        creg = mix(creg, fract((to_float(k) + 0.5) / to_float(cnt) * dc_scale), color_speed);
        this.creg = creg;
        color = creg;
    }
    let space = u32(to_float(p.space));
    let ba = fuchsian_triangle_read(p, k);
    let out = vec3(0.0, 0.0, 0.0);
    if (space == u32(1)) {
        out = su_apply_plain3(ba, v);
    } else {
        let t1 = su_apply_plain(ba, complex(v.x, v.y));
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
