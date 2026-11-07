/// Hyperbolic (p,q,r) rotation groups as a Möbius chaos game with
/// tessellation seeding.
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5

// Von Dyck, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    p: #{
        value: 2, min: 2, max: 24,
        caption: "P",
        hint: "Order of the rotation about vertex A (angle π/p there). (p,q,r) is hyperbolic iff 1/p + 1/q + 1/r < 1; the default (2,3,7) is the minimal Hurwitz triangle.",
    },
    q: #{
        value: 3, min: 2, max: 24,
        caption: "Q",
        hint: "Order of the rotation about vertex B (angle π/q).",
    },
    r: #{
        value: 7, min: 2, max: 24,
        caption: "R",
        hint: "Order of the rotation about vertex C (angle π/r).",
    },
    mirror: #{
        value: false,
        caption: "Mirror",
        hint: "Add the reflection in the A–B edge (an anti-Möbius generator z ↦ z̄) — upgrades the rotation group D(p,q,r) to the FULL triangle group Δ*(p,q,r), doubling the stamped copies (each triangle plus its mirror image).",
    },
    size: #{
        value: 1.0, min: 0.1, max: 4.0,
        caption: "Size",
        hint: "Radius of the Poincaré disk in world units.",
    },
    steps: #{
        value: 2, min: 1, max: 8,
        caption: "Steps",
        hint: "Group elements applied per call (backtrack-avoiding random walk).",
    },
    seed: #{
        value: 2, choices: ["Input", "Vertices", "Edges", "Faces"],
        caption: "Seed",
        hint: "What the walk stamps through the tiling: the incoming flame measure, the triangle's vertex orbit, its edge skeleton (geodesic segments), or face fragments — honeycomb's seed machinery on Möbius orbits.",
    },
    thickness: #{
        value: 0.0, min: 0.0, max: 2.0,
        caption: "Thickness",
        hint: "Seed modes: geodesic tangent-space offset by exact hyperbolic distance — balls at vertices, tubes along edges, slabs on faces, all with uniform hyperbolic radius.",
    },
    dc_mode: #{
        value: 0, choices: ["Off", "Generator", "Steps"],
        caption: "Color Mode",
        hint: "Direct-color source (needs the transform's Direct Color > 0). Generator: each group generator has its own palette position, blended through a persistent register at Color Speed. Steps: palette cycles with the walk depth since the last reseed (wraps, so deep levels stay distinct).",
    },
    dc_scale: #{
        value: 1.0, min: 0.1, max: 8.0,
        caption: "Color Scale",
        hint: "Palette-index multiplier for the color modes.",
    },
    color_speed: #{
        value: 0.5, min: 0.0, max: 1.0,
        caption: "Color Speed",
        hint: "Generator mode: pull strength toward each generator's palette position. Steps mode: palette advance per reflection (cyclic — wraps instead of saturating).",
    },
    space: #{
        value: 0, choices: ["Euclidean", "Hyperbolic H3"],
        caption: "Space",
        hint: "3D render mode only. Euclidean: the disk tiling in the xy plane (z passes through). Hyperbolic H3: the Poincaré extension — the 2D group acts on upper half-space (height = z) preserving the hemisphere dome over the unit circle, and seeds are planted ON the dome: the tiling drapes over the curved dome surface.",
    },
    qc_deform: #{
        value: false,
        caption: "QC Deform",
        hint: "Conjugate every generator by Bagula's triquasiconformal C = dk(δ)·s0·qf(θ+iη) — the even→uneven warp applied to a tiling. Triangle groups are rigid (their deformation space is a point), so this global Möbius warp is the only deformation they admit.",
    },
    conj_angle: #{
        value: 45.0, min: -360.0, max: 360.0, type: "angle",
        caption: "Angle",
        hint: "Elliptic rotation θ in the conjugator (QC Deform on).",
    },
    conj_hyper: #{
        value: 0.0, min: -360.0, max: 360.0, type: "angle",
        caption: "Hyper Angle",
        hint: "Hyperbolic rotation η in the conjugator (QC Deform on).",
    },
    qc_strength: #{
        value: 1.0, min: 0.1, max: 2.0,
        caption: "QC Strength",
        hint: "Quasiconformal δ in dk = [[1+iδ,1],[1,1−iδ]] (QC Deform on).",
    },
};

const INFO = #{ title: "Von Dyck", name: "rhai_von_dyck", category: "advanced_2d" };

fn init(p) {
    #{ st0: 0.0, creg: 0.0, depth: 0.0 }
}

fn cmul(a, b) {
    complex(a.re * b.re - a.im * b.im, a.re * b.im + a.im * b.re)
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

fn vd_mdot(a, b) {
    a.x * b.x + a.y * b.y - a.z * b.z
}

fn vd_lift(u) {
    let den = max(1.0e-6, 1.0 - u.norm_sqr());
    let t1 = 2.0 * u / den;
    vec3(t1.re, t1.im, (1.0 + u.norm_sqr()) / den)
}

fn vd_proj(h) {
    complex(h.x, h.y) / (1.0 + max(1.0, h.z))
}

fn vd_hnorm(v) {
    v / sqrt(max(1.0e-6, -vd_mdot(v, v)))
}

fn vd_rot(w, lam) {
    let lami = complex(lam.re, -lam.im);
    let w2 = w.norm_sqr();
    let den = max(1.0e-6, 1.0 - w2);
    [(lam - w2 * lami) / den, cmul(w, lami - lam) / den, cmul(complex(w.re, -w.im), lam - lami) / den, (lami - w2 * lam) / den]
}

fn vd_gen(k, A, B, C, sp, sq, sr) {
    let base = k % u32(3);
    let sgn = (if k >= u32(3) { -1.0 } else { 1.0 });
    let w = A;
    let n = sp;
    if (base == u32(1)) {
        w = B;
        n = sq;
    } else if (base == u32(2)) {
        w = C;
        n = sr;
    }
    let half = sgn * 3.14159265359 / n;
    vd_rot(w, complex(cos(half), sin(half)))
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
    let sp = max(2.0, to_float(p.p));
    let sq = max(2.0, to_float(p.q));
    let sr = max(2.0, to_float(p.r));
    let mirror = to_float(p.mirror) > 0.5;
    let size = max(1.0e-6, p.size);
    let steps = to_int(to_float(p.steps));
    let seed_mode = u32(to_float(p.seed));
    let thickness = p.thickness;
    let dc_mode = u32(to_float(p.dc_mode));
    let dc_scale = p.dc_scale;
    let color_speed = p.color_speed;
    let deform = to_float(p.qc_deform) > 0.5;
    let theta = p.conj_angle * 0.01745329252;
    let eta = p.conj_hyper * 0.01745329252;
    let delta = p.qc_strength;
    let pi = 3.14159265359;
    let ap = pi / sp;
    let aq = pi / sq;
    let ar = pi / sr;
    let coshAB = (cos(ar) + cos(ap) * cos(aq)) / max(1.0e-6, sin(ap) * sin(aq));
    let coshAC = (cos(aq) + cos(ap) * cos(ar)) / max(1.0e-6, sin(ap) * sin(ar));
    let rB = tanh(0.5 * acosh(max(1.0, coshAB)));
    let rC = tanh(0.5 * acosh(max(1.0, coshAC)));
    let A = complex(0.0, 0.0);
    let B = complex(rB, 0.0);
    let C = rC * complex(cos(ap), sin(ap));
    let cnt = u32(6);
    if (mirror) {
        cnt = u32(7);
    }
    let cj = [complex(1.0, 0.0), complex(0.0, 0.0), complex(0.0, 0.0), complex(1.0, 0.0)];
    let cji = cj;
    if (deform) {
        cj = su_conjugator(theta, eta, delta);
        cji = su_matinv(cj);
    }
    let depth = this.depth;
    let a = complex(0.0, 0.0);
    if (seed_mode != u32(0) && rand() < 0.1) {
        let m = vec3(0.0, 0.0, 0.0);
        if (seed_mode == u32(1)) {
            let pick = min(u32(rand() * 3.0), u32(2));
            let w = A;
            if (pick == u32(1)) {
                w = B;
            } else if (pick == u32(2)) {
                w = C;
            }
            m = vd_lift(w);
        } else if (seed_mode == u32(2)) {
            let pick = min(u32(rand() * 3.0), u32(2));
            let u0 = A;
            let u1 = B;
            if (pick == u32(1)) {
                u0 = B;
                u1 = C;
            } else if (pick == u32(2)) {
                u0 = C;
                u1 = A;
            }
            let t = rand();
            m = vd_hnorm(mix(vd_lift(u0), vd_lift(u1), t));
        } else {
            let u = rand();
            let v = rand() * (1.0 - u);
            let hA = vd_lift(A);
            let hB = vd_lift(B);
            let hC = vd_lift(C);
            m = vd_hnorm(hA + u * (hB - hA) + v * (hC - hA));
        }
        if (thickness > 0.0) {
            let ph = rand() * 6.28318530718;
            let tt = thickness * sqrt(rand());
            let e1 = vec3(1.0, 0.0, 0.0);
            e1 = e1 + vd_mdot(e1, m) * m;
            e1 = e1 / sqrt(max(1.0e-6, vd_mdot(e1, e1)));
            let e2 = vec3(0.0, 1.0, 0.0);
            e2 = e2 + vd_mdot(e2, m) * m;
            e2 = e2 - vd_mdot(e2, e1) * e1;
            e2 = e2 / sqrt(max(1.0e-6, vd_mdot(e2, e2)));
            let u = cos(ph) * e1 + sin(ph) * e2;
            m = cosh(tt) * m + sinh(tt) * u;
        }
        a = vd_proj(m);
        depth = 0.0;
    } else {
        a = z / size;
        let r2 = a.norm_sqr();
        if (r2 >= 1.0) {
            a = a / (r2 + 1.0e-9);
        }
    }
    let prev = u32(this.st0);
    let creg = this.creg;
    for i in 0..steps {
        let k = min(u32(rand() * to_float(cnt)), cnt - u32(1));
        let invp = prev;
        if (prev < u32(6)) {
            invp = (prev + u32(3)) % u32(6);
        }
        if (prev < cnt && k == invp) {
            k = (k + u32(1)) % cnt;
        }
        if (k == u32(6)) {
            if (deform) {
                let t = su_apply_plain(cji, a);
                t = complex(t.re, -t.im);
                a = su_apply_plain(cj, t);
            } else {
                a = complex(a.re, -a.im);
            }
        } else {
            let g = vd_gen(k, A, B, C, sp, sq, sr);
            if (deform) {
                a = su_apply_m(g, a, cj, cji);
            } else {
                a = su_apply_plain(g, a);
            }
        }
        prev = k;
        if (dc_mode == u32(1)) {
            creg = mix(creg, fract((to_float(k) + 0.5) / to_float(cnt) * dc_scale), color_speed);
        }
    }
    this.st0 = to_float(prev);
    depth = depth + to_float(steps);
    this.depth = depth;
    if (dc_mode == u32(1)) {
        this.creg = creg;
        color = creg;
    } else if (dc_mode == u32(2)) {
        if (seed_mode != u32(0)) {
            let g = max(1.0, depth / max(1.0, to_float(steps)));
            let q = 0.9;
            let t = 1.0 - pow(q, g - 1.0) * (1.0 + q) * 0.5;
            color = fract(t * dc_scale);
        } else {
            color = fract(depth * color_speed * 0.1 * dc_scale);
        }
    }
    #{ z: a * size, color: color }
}

fn variation3(pt, p, ctx) {
    let color = ctx.color;
    let sp = max(2.0, to_float(p.p));
    let sq = max(2.0, to_float(p.q));
    let sr = max(2.0, to_float(p.r));
    let mirror = to_float(p.mirror) > 0.5;
    let size = max(1.0e-6, p.size);
    let steps = to_int(to_float(p.steps));
    let seed_mode = u32(to_float(p.seed));
    let thickness = p.thickness;
    let dc_mode = u32(to_float(p.dc_mode));
    let dc_scale = p.dc_scale;
    let color_speed = p.color_speed;
    let space = u32(to_float(p.space));
    let deform = to_float(p.qc_deform) > 0.5;
    let theta = p.conj_angle * 0.01745329252;
    let eta = p.conj_hyper * 0.01745329252;
    let delta = p.qc_strength;
    let pi = 3.14159265359;
    let ap = pi / sp;
    let aq = pi / sq;
    let ar = pi / sr;
    let coshAB = (cos(ar) + cos(ap) * cos(aq)) / max(1.0e-6, sin(ap) * sin(aq));
    let coshAC = (cos(aq) + cos(ap) * cos(ar)) / max(1.0e-6, sin(ap) * sin(ar));
    let rB = tanh(0.5 * acosh(max(1.0, coshAB)));
    let rC = tanh(0.5 * acosh(max(1.0, coshAC)));
    let A = complex(0.0, 0.0);
    let B = complex(rB, 0.0);
    let C = rC * complex(cos(ap), sin(ap));
    let cnt = u32(6);
    if (mirror) {
        cnt = u32(7);
    }
    let cj = [complex(1.0, 0.0), complex(0.0, 0.0), complex(0.0, 0.0), complex(1.0, 0.0)];
    let cji = cj;
    if (deform) {
        cj = su_conjugator(theta, eta, delta);
        cji = su_matinv(cj);
    }
    let depth = this.depth;
    let t1 = complex(pt.x, pt.y);
    let a3 = vec3(t1.re, t1.im, pt.z) / size;
    if (seed_mode != u32(0) && rand() < 0.1) {
        let m = vec3(0.0, 0.0, 0.0);
        if (seed_mode == u32(1)) {
            let pick = min(u32(rand() * 3.0), u32(2));
            let w = A;
            if (pick == u32(1)) {
                w = B;
            } else if (pick == u32(2)) {
                w = C;
            }
            m = vd_lift(w);
        } else if (seed_mode == u32(2)) {
            let pick = min(u32(rand() * 3.0), u32(2));
            let u0 = A;
            let u1 = B;
            if (pick == u32(1)) {
                u0 = B;
                u1 = C;
            } else if (pick == u32(2)) {
                u0 = C;
                u1 = A;
            }
            let t = rand();
            m = vd_hnorm(mix(vd_lift(u0), vd_lift(u1), t));
        } else {
            let u = rand();
            let v = rand() * (1.0 - u);
            let hA = vd_lift(A);
            let hB = vd_lift(B);
            let hC = vd_lift(C);
            m = vd_hnorm(hA + u * (hB - hA) + v * (hC - hA));
        }
        if (thickness > 0.0) {
            let ph = rand() * 6.28318530718;
            let tt = thickness * sqrt(rand());
            let e1 = vec3(1.0, 0.0, 0.0);
            e1 = e1 + vd_mdot(e1, m) * m;
            e1 = e1 / sqrt(max(1.0e-6, vd_mdot(e1, e1)));
            let e2 = vec3(0.0, 1.0, 0.0);
            e2 = e2 + vd_mdot(e2, m) * m;
            e2 = e2 - vd_mdot(e2, e1) * e1;
            e2 = e2 / sqrt(max(1.0e-6, vd_mdot(e2, e2)));
            let u = cos(ph) * e1 + sin(ph) * e2;
            m = cosh(tt) * m + sinh(tt) * u;
        }
        if (space == u32(1)) {
            let t2 = complex(m.x, m.y) / m.z;
            a3 = vec3(t2.re, t2.im, 1.0 / m.z);
        } else {
            let t3 = vd_proj(m);
            a3 = vec3(t3.re, t3.im, a3.z);
        }
        depth = 0.0;
    } else {
        let r2 = complex(a3.x, a3.y).norm_sqr();
        if (r2 >= 1.0 && space == u32(0)) {
            let t4 = complex(a3.x, a3.y) / (r2 + 1.0e-9);
            a3 = vec3(t4.re, t4.im, a3.z);
        }
    }
    let prev = u32(this.st0);
    let creg = this.creg;
    for i in 0..steps {
        let k = min(u32(rand() * to_float(cnt)), cnt - u32(1));
        let invp = prev;
        if (prev < u32(6)) {
            invp = (prev + u32(3)) % u32(6);
        }
        if (prev < cnt && k == invp) {
            k = (k + u32(1)) % cnt;
        }
        if (k == u32(6)) {
            if (space == u32(1)) {
                if (deform) {
                    let t = su_apply_plain3(cji, a3);
                    t = vec3(t.x, -t.y, t.z);
                    a3 = su_apply_plain3(cj, t);
                } else {
                    a3 = vec3(a3.x, -a3.y, a3.z);
                }
            } else if (deform) {
                let t = su_apply_plain(cji, complex(a3.x, a3.y));
                t = complex(t.re, -t.im);
                let t5 = su_apply_plain(cj, t);
                a3 = vec3(t5.re, t5.im, a3.z);
            } else {
                a3 = vec3(a3.x, -a3.y, a3.z);
            }
        } else {
            let g = vd_gen(k, A, B, C, sp, sq, sr);
            if (space == u32(1)) {
                if (deform) {
                    a3 = su_apply_m3(g, a3, cj, cji);
                } else {
                    a3 = su_apply_plain3(g, a3);
                }
            } else if (deform) {
                let t6 = su_apply_m(g, complex(a3.x, a3.y), cj, cji);
                a3 = vec3(t6.re, t6.im, a3.z);
            } else {
                let t7 = su_apply_plain(g, complex(a3.x, a3.y));
                a3 = vec3(t7.re, t7.im, a3.z);
            }
        }
        prev = k;
        if (dc_mode == u32(1)) {
            creg = mix(creg, fract((to_float(k) + 0.5) / to_float(cnt) * dc_scale), color_speed);
        }
    }
    this.st0 = to_float(prev);
    depth = depth + to_float(steps);
    this.depth = depth;
    if (dc_mode == u32(1)) {
        this.creg = creg;
        color = creg;
    } else if (dc_mode == u32(2)) {
        if (seed_mode != u32(0)) {
            let g = max(1.0, depth / max(1.0, to_float(steps)));
            let q = 0.9;
            let t = 1.0 - pow(q, g - 1.0) * (1.0 + q) * 0.5;
            color = fract(t * dc_scale);
        } else {
            color = fract(depth * color_speed * 0.1 * dc_scale);
        }
    }
    #{ z: a3 * size, color: color }
}
