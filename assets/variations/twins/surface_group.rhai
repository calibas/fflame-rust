/// Genus-2 surface group: the octagon tiling with a quasi-Fuchsian bend
/// into SL(2,ℂ).
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5

// Surface Group, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    bend_re: #{
        value: 0.0, min: -1.0, max: 1.0, free: true,
        caption: "Bend re",
        hint: "Real part added to the pairing translation length c = arccosh(cot π/8). 0 is the exact genus-2 octagon group; nonzero changes the octagon's side length (the corners no longer close up exactly — cone-angle shear in the tiling).",
    },
    bend_im: #{
        value: 0.0, min: -1.0, max: 1.0, free: true,
        caption: "Bend im",
        hint: "Imaginary part of the pairing translation — the quasi-Fuchsian-style bend into SL(2,ℂ). 0 keeps the group Fuchsian (limit set a round circle); small values fragment the boundary into fractal quasi-circle dust while the interior tiling shears. Large values leave discreteness entirely — wilder, denser orbits.",
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
        value: 0, choices: ["Input", "Vertices", "Edges", "Faces"],
        caption: "Seed",
        hint: "What the walk stamps through the tiling: the incoming flame measure, the octagon's vertex orbit (8 corners, one orbit), its edge skeleton, or face-fan fragments.",
    },
    thickness: #{
        value: 0.0, min: 0.0, max: 2.0,
        caption: "Thickness",
        hint: "Seed modes: geodesic tangent-space offset by exact hyperbolic distance — balls, tubes, slabs of uniform hyperbolic radius.",
    },
    dc_mode: #{
        value: 0, choices: ["Off", "Generator", "Steps"],
        caption: "Color Mode",
        hint: "Direct-color source (needs the transform's Direct Color > 0). Generator: each of the 8 side-pairings has its own palette position, blended through a persistent register at Color Speed. Steps: palette cycles with the walk depth since the last reseed (wraps, so deep levels stay distinct).",
    },
    dc_scale: #{
        value: 1.0, min: 0.1, max: 8.0,
        caption: "Color Scale",
        hint: "Palette-index multiplier for the color modes.",
    },
    color_speed: #{
        value: 0.5, min: 0.0, max: 1.0,
        caption: "Color Speed",
        hint: "Generator mode: pull strength toward each pairing's palette position. Steps mode: palette advance per reflection (cyclic — wraps instead of saturating).",
    },
    space: #{
        value: 0, choices: ["Euclidean", "Hyperbolic H3"],
        caption: "Space",
        hint: "3D render mode only. Euclidean: the disk tiling in the xy plane (z passes through). Hyperbolic H3: the Poincaré extension — seeds plant on the hemisphere dome over the unit circle, draping the octagon tiling over a curved dome; bent (complex) groups act genuinely 3-dimensionally.",
    },
};

const INFO = #{ title: "Surface Group", name: "rhai_surface_group", category: "advanced_2d" };

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

fn su_apply_plain(m, z) {
    su_cdiv(su_cmul(m[0], z) + m[1], su_cmul(m[2], z) + m[3])
}

fn sg_mdot(a, b) {
    a.x * b.x + a.y * b.y - a.z * b.z
}

fn sg_lift(u) {
    let den = max(1.0e-6, 1.0 - u.norm_sqr());
    let t1 = 2.0 * u / den;
    vec3(t1.re, t1.im, (1.0 + u.norm_sqr()) / den)
}

fn sg_proj(h) {
    complex(h.x, h.y) / (1.0 + max(1.0, h.z))
}

fn sg_hnorm(v) {
    v / sqrt(max(1.0e-6, -sg_mdot(v, v)))
}

fn sg_vertex(k) {
    let th = (2.0 * to_float(k) + 1.0) * 0.39269908;
    0.8408964 * complex(cos(th), sin(th))
}

fn sg_gen(k, bend) {
    let base = k % u32(4);
    let sgn = (if k >= u32(4) { -1.0 } else { 1.0 });
    let c = sgn * (complex(1.5285709, 0.0) + bend);
    let ch = complex(cosh(c.re) * cos(c.im), sinh(c.re) * sin(c.im));
    let sh = complex(sinh(c.re) * cos(c.im), cosh(c.re) * sin(c.im));
    let th = to_float(base) * 0.78539816;
    let e = complex(cos(th), sin(th));
    let ei = complex(cos(th), -sin(th));
    [ch, cmul(e, sh), cmul(ei, sh), ch]
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
    let bend = complex(p.bend_re, p.bend_im);
    let size = max(1.0e-6, p.size);
    let steps = to_int(to_float(p.steps));
    let seed_mode = u32(to_float(p.seed));
    let thickness = p.thickness;
    let dc_mode = u32(to_float(p.dc_mode));
    let dc_scale = p.dc_scale;
    let color_speed = p.color_speed;
    let depth = this.depth;
    let a = complex(0.0, 0.0);
    if (seed_mode != u32(0) && rand() < 0.1) {
        let m = vec3(0.0, 0.0, 0.0);
        if (seed_mode == u32(1)) {
            m = sg_lift(sg_vertex(min(u32(rand() * 8.0), u32(7))));
        } else if (seed_mode == u32(2)) {
            let k = min(u32(rand() * 8.0), u32(7));
            let t = rand();
            m = sg_hnorm(mix(sg_lift(sg_vertex(k)), sg_lift(sg_vertex((k + u32(1)) % u32(8))), t));
        } else {
            let k = min(u32(rand() * 8.0), u32(7));
            let u = rand();
            let v = rand() * (1.0 - u);
            let h0 = vec3(0.0, 0.0, 1.0);
            let h1 = sg_lift(sg_vertex(k));
            let h2 = sg_lift(sg_vertex((k + u32(1)) % u32(8)));
            m = sg_hnorm(h0 + u * (h1 - h0) + v * (h2 - h0));
        }
        if (thickness > 0.0) {
            let ph = rand() * 6.28318530718;
            let tt = thickness * sqrt(rand());
            let e1 = vec3(1.0, 0.0, 0.0);
            e1 = e1 + sg_mdot(e1, m) * m;
            e1 = e1 / sqrt(max(1.0e-6, sg_mdot(e1, e1)));
            let e2 = vec3(0.0, 1.0, 0.0);
            e2 = e2 + sg_mdot(e2, m) * m;
            e2 = e2 - sg_mdot(e2, e1) * e1;
            e2 = e2 / sqrt(max(1.0e-6, sg_mdot(e2, e2)));
            let u = cos(ph) * e1 + sin(ph) * e2;
            m = cosh(tt) * m + sinh(tt) * u;
        }
        a = sg_proj(m);
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
        let k = min(u32(rand() * 8.0), u32(7));
        if (prev < u32(8) && k == (prev + u32(4)) % u32(8)) {
            k = (k + u32(1)) % u32(8);
        }
        a = su_apply_plain(sg_gen(k, bend), a);
        prev = k;
        if (dc_mode == u32(1)) {
            creg = mix(creg, fract((to_float(k) + 0.5) / 8.0 * dc_scale), color_speed);
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
    let bend = complex(p.bend_re, p.bend_im);
    let size = max(1.0e-6, p.size);
    let steps = to_int(to_float(p.steps));
    let seed_mode = u32(to_float(p.seed));
    let thickness = p.thickness;
    let dc_mode = u32(to_float(p.dc_mode));
    let dc_scale = p.dc_scale;
    let color_speed = p.color_speed;
    let space = u32(to_float(p.space));
    let depth = this.depth;
    let t1 = complex(pt.x, pt.y);
    let a3 = vec3(t1.re, t1.im, pt.z) / size;
    if (seed_mode != u32(0) && rand() < 0.1) {
        let m = vec3(0.0, 0.0, 0.0);
        if (seed_mode == u32(1)) {
            m = sg_lift(sg_vertex(min(u32(rand() * 8.0), u32(7))));
        } else if (seed_mode == u32(2)) {
            let k = min(u32(rand() * 8.0), u32(7));
            let t = rand();
            m = sg_hnorm(mix(sg_lift(sg_vertex(k)), sg_lift(sg_vertex((k + u32(1)) % u32(8))), t));
        } else {
            let k = min(u32(rand() * 8.0), u32(7));
            let u = rand();
            let v = rand() * (1.0 - u);
            let h0 = vec3(0.0, 0.0, 1.0);
            let h1 = sg_lift(sg_vertex(k));
            let h2 = sg_lift(sg_vertex((k + u32(1)) % u32(8)));
            m = sg_hnorm(h0 + u * (h1 - h0) + v * (h2 - h0));
        }
        if (thickness > 0.0) {
            let ph = rand() * 6.28318530718;
            let tt = thickness * sqrt(rand());
            let e1 = vec3(1.0, 0.0, 0.0);
            e1 = e1 + sg_mdot(e1, m) * m;
            e1 = e1 / sqrt(max(1.0e-6, sg_mdot(e1, e1)));
            let e2 = vec3(0.0, 1.0, 0.0);
            e2 = e2 + sg_mdot(e2, m) * m;
            e2 = e2 - sg_mdot(e2, e1) * e1;
            e2 = e2 / sqrt(max(1.0e-6, sg_mdot(e2, e2)));
            let u = cos(ph) * e1 + sin(ph) * e2;
            m = cosh(tt) * m + sinh(tt) * u;
        }
        if (space == u32(1)) {
            let t2 = complex(m.x, m.y) / m.z;
            a3 = vec3(t2.re, t2.im, 1.0 / m.z);
        } else {
            let t3 = sg_proj(m);
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
        let k = min(u32(rand() * 8.0), u32(7));
        if (prev < u32(8) && k == (prev + u32(4)) % u32(8)) {
            k = (k + u32(1)) % u32(8);
        }
        let g = sg_gen(k, bend);
        if (space == u32(1)) {
            a3 = su_apply_plain3(g, a3);
        } else {
            let t5 = su_apply_plain(g, complex(a3.x, a3.y));
            a3 = vec3(t5.re, t5.im, a3.z);
        }
        prev = k;
        if (dc_mode == u32(1)) {
            creg = mix(creg, fract((to_float(k) + 0.5) / 8.0 * dc_scale), color_speed);
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
