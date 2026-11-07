/// Hyperbolic honeycomb reflection walk (Schläfli {p,q,r}; 2D mode is
/// the general (p,q,r) triangle group).
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5

// Hyperbolic Honeycomb, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    p: #{
        value: 5, min: 2, max: 12,
        caption: "P",
        hint: "First Schläfli number: the cell's face polygon ({5,3,4} = dodecahedral cells). Compact hyperbolic honeycombs: {5,3,4}, {4,3,5}, {3,5,3}, {5,3,5}. Other combinations still run — as spherical/Euclidean/non-discrete kaleidoscopes.",
    },
    q: #{
        value: 3, min: 2, max: 12,
        caption: "Q",
        hint: "Second Schläfli number: faces around each cell vertex ({p,q} is the cell polyhedron). In 2D render mode (p,q,r) is the general Fuchsian triangle group.",
    },
    r: #{
        value: 4, min: 2, max: 12,
        caption: "R",
        hint: "Third Schläfli number: cells around each edge (3D). In 2D render mode this is the THIRD triangle angle: the group becomes the general (p,q,r) triangle group (angles pi/p, pi/q, pi/r) — r = 2 gives the standard {p,q} tiling, r > 2 the full Fuchsian triangle groups like (2,3,12).",
    },
    size: #{
        value: 1.0, min: 0.1, max: 4.0,
        caption: "Size",
        hint: "Radius of the ball model in world units.",
    },
    steps: #{
        value: 2, min: 1, max: 8,
        caption: "Steps",
        hint: "Random mirror reflections per call. More steps walk deeper into the group per iteration (finer boundary detail for the same budget); even counts favor orientation-preserving words.",
    },
    projection: #{
        value: 0, choices: ["Poincaré", "Beltrami–Klein", "Half-Space"],
        caption: "Projection",
        hint: "Model of hyperbolic space for the plotted (and fed-forward) point. Poincaré: conformal ball — cells shrink toward the boundary sphere. Beltrami–Klein: projective ball — straight geodesics, flat cell walls. Half-Space: the honeycomb fills the region above an infinite floor plane, cells shrinking toward the floor and growing overhead (the classic 'honeycomb over a plane' renders; upper half-PLANE in 2D).",
    },
    dc_mode: #{
        value: 0, choices: ["Off", "Mirror", "Depth", "Steps"],
        caption: "Color Mode",
        hint: "Direct-color source (needs the transform's Direct Color > 0). Mirror: each of the 4 orthoscheme mirrors acts like a transform with its own palette position, blended through a persistent color register at Color Speed. Depth: color by the output z (2D: disc radius). Steps: palette position sweeps start-to-end across the walk depth since the last reseed — the first shells get the first palette colors, the deepest get the last. Color Speed 1 spans the full palette over the mean walk length; Color Scale adds wrap cycles. (Seed modes; in Input mode depth never resets, so everything saturates at the final color.)",
    },
    dc_scale: #{
        value: 1.0, min: 0.1, max: 8.0,
        caption: "Color Scale",
        hint: "Palette-index multiplier for the Mirror colors (wrapped with fract).",
    },
    color_speed: #{
        value: 0.5, min: 0.0, max: 1.0,
        caption: "Color Speed",
        hint: "Mirror mode: how hard each reflection pulls the color register toward its mirror's palette position (low = long blends, 1 = hard assignment). Steps mode: how much of the palette the walk traverses — 1 sweeps the whole palette from first shell to the mean walk depth.",
    },
    seed: #{
        value: 2, choices: ["Input", "Vertices", "Edges", "Faces"],
        caption: "Seed",
        hint: "What the group walk stamps through the honeycomb. Input: the incoming flame measure (compose freely — any Wythoff decoration). Vertices: the honeycomb vertex — an array of dots. Edges: random points on the vertex-to-edge-center geodesic — the full ball-and-stick edge skeleton, the classic look of published honeycomb renders. Faces: random points on the orthoscheme's face triangle — the cell walls.",
    },
    thickness: #{
        value: 0.01, min: 0.0, max: 2.0,
        caption: "Thickness",
        hint: "Seed modes only: perturbs the seed before it lands on the hyperboloid, turning vertices into balls, edges into tubes, faces into slabs. The walk is isometric, so the tubes stay crisp and shrink toward the boundary exactly like the cells do — hyperbolically true thickness, unlike a plot-space jitter (which would also accumulate through the fed-forward walk).",
    },
    variant: #{
        value: 0, choices: ["Normal", "Rectified", "Truncated", "Bitruncated", "Cantellated", "Cantitruncated", "Runcitruncated", "Runcicantellated", "Omnitruncated"],
        caption: "Variant",
        hint: "Wythoff variant of the honeycomb (seed modes only): which Coxeter-diagram nodes are ringed. The generator point sits ON every unringed mirror and equidistant from every ringed one; Vertices/Edges/Faces then stamp the chosen variant's elements. Normal = the plain {p,q,r}; Omnitruncated rings all four nodes. Input mode ignores this — there the flame itself is the generator.",
    },
};

const INFO = #{ title: "Hyperbolic Honeycomb", name: "rhai_honeycomb", category: "advanced_2d" };

fn init(p) {
    #{ creg: 0.0, depth: 0.0 }
}

fn honeycomb_mdot2(a, b) {
    a.x * b.x + a.y * b.y - a.z * b.z
}

fn honeycomb_mdot(a, b) {
    a.x * b.x + a.y * b.y + a.z * b.z - a.w * b.w
}

fn count_one_bits(x) {
    let v = x;
    let n = u32(0);
    while v != u32(0) {
        v = v & (v - u32(1));
        n = n + u32(1);
    }
    n
}

fn variation(pt2, p, ctx) {
    let color = ctx.color;
    let sp = to_float(p.p);
    let sq = to_float(p.q);
    let sr = to_float(p.r);
    let size = max(1.0e-6, p.size);
    let steps = to_int(to_float(p.steps));
    let projection = u32(to_float(p.projection));
    let dc_mode = u32(to_float(p.dc_mode));
    let dc_scale = p.dc_scale;
    let color_speed = p.color_speed;
    let variant = u32(to_float(p.variant));
    let pi = 3.14159265359;
    let cp = cos(pi / max(2.0, sp));
    let cq = cos(pi / max(2.0, sq));
    let cr = cos(pi / max(2.0, sr));
    let s1 = sqrt(max(1.0e-6, 1.0 - cp * cp));
    let n2x = -cr;
    let n2y = (-cq - cp * cr) / s1;
    let w2 = sqrt(max(0.0, n2x * n2x + n2y * n2y - 1.0));
    let mirrors = [1.0, 0.0, 0.0, -cp, s1, 0.0, n2x, n2y, w2];
    let seed_mode = u32(to_float(p.seed));
    let thickness = p.thickness;
    let depth = this.depth;
    let x = vec3(0.0, 0.0, 0.0);
    if (seed_mode != u32(0) && rand() < 0.1) {
        let mask = u32(1);
        let t1 = variant;
        if t1 == u32(1) {
            mask = u32(2);
        } else if t1 == u32(2) {
            mask = u32(3);
        } else if t1 == u32(3) {
            mask = u32(6);
        } else if t1 == u32(4) {
            mask = u32(5);
        } else if t1 == u32(5) || t1 == u32(8) {
            mask = u32(7);
        } else if t1 == u32(6) {
            mask = u32(3);
        } else if t1 == u32(7) {
            mask = u32(5);
        } else {
            mask = u32(1);
        }
        let rc0 = (if (mask & u32(1)) != u32(0) { -1.0 } else { 0.0 });
        let rc1 = (if (mask & u32(2)) != u32(0) { -1.0 } else { 0.0 });
        let rc2 = (if (mask & u32(4)) != u32(0) { -1.0 } else { 0.0 });
        let px = rc0;
        let py = (rc1 + cp * px) / s1;
        let w2s = max(1.0e-6, w2);
        let pt = (n2x * px + n2y * py - rc2) / w2s;
        let pm = vec3(px, py, pt);
        if (pm.z < 0.0) {
            pm = -pm;
        }
        let wp = pm / sqrt(max(1.0e-6, -honeycomb_mdot2(pm, pm)));
        let m = wp;
        if (seed_mode >= u32(2)) {
            let nring = to_float(count_one_bits(mask));
            let pick = to_int(min(rand() * nring, nring - 0.5));
            let mi = u32(0);
            for i_n in 0..3 {
                let i = u32(i_n);
                if ((mask & (u32(1) << i)) != u32(0)) {
                    if (pick == 0) {
                        mi = i;
                        break;
                    }
                    pick = pick - 1;
                }
            }
            let f1 = wp - honeycomb_mdot2(wp, vec3(mirrors[3 * mi + 0], mirrors[3 * mi + 1], mirrors[3 * mi + 2])) * vec3(mirrors[3 * mi + 0], mirrors[3 * mi + 1], mirrors[3 * mi + 2]);
            f1 = f1 / sqrt(max(1.0e-6, -honeycomb_mdot2(f1, f1)));
            if (seed_mode == u32(2)) {
                m = mix(wp, f1, rand());
            } else {
                let mj = (mi + u32(1) + min(u32(rand() * 2.0), u32(1))) % u32(3);
                for ttry_n in 0..2 {
                    let ttry = u32(ttry_n);
                    if (mj != mi && abs(honeycomb_mdot2(f1, vec3(mirrors[3 * mj + 0], mirrors[3 * mj + 1], mirrors[3 * mj + 2]))) > 1.0e-4) {
                        break;
                    }
                    mj = (mj + u32(1)) % u32(3);
                    if (mj == mi) {
                        mj = (mj + u32(1)) % u32(3);
                    }
                }
                let f2 = f1 - honeycomb_mdot2(f1, vec3(mirrors[3 * mj + 0], mirrors[3 * mj + 1], mirrors[3 * mj + 2])) * vec3(mirrors[3 * mj + 0], mirrors[3 * mj + 1], mirrors[3 * mj + 2]);
                f2 = f2 / sqrt(max(1.0e-6, -honeycomb_mdot2(f2, f2)));
                let u = rand();
                let v = rand() * (1.0 - u);
                m = wp + u * (f1 - wp) + v * (f2 - wp);
            }
        }
        x = m / sqrt(max(1.0e-6, -honeycomb_mdot2(m, m)));
        if (thickness > 0.0) {
            let ph = rand() * 6.28318530718;
            let tt = thickness * sqrt(rand());
            let e1 = vec3(1.0, 0.0, 0.0);
            e1 = e1 + honeycomb_mdot2(e1, x) * x;
            e1 = e1 / sqrt(max(1.0e-6, honeycomb_mdot2(e1, e1)));
            let e2 = vec3(0.0, 1.0, 0.0);
            e2 = e2 + honeycomb_mdot2(e2, x) * x;
            e2 = e2 - honeycomb_mdot2(e2, e1) * e1;
            e2 = e2 / sqrt(max(1.0e-6, honeycomb_mdot2(e2, e2)));
            let u = cos(ph) * e1 + sin(ph) * e2;
            x = cosh(tt) * x + sinh(tt) * u;
        }
        depth = 0.0;
    } else {
        let b = pt2 / size;
        let r2 = b.norm_sqr();
        if (r2 >= 1.0) {
            b = b / (r2 + 1.0e-9);
            r2 = b.norm_sqr();
        }
        if (projection == u32(1)) {
            let t = 1.0 / sqrt(max(1.0e-9, 1.0 - r2));
            let t2 = b * t;
            x = vec3(t2.re, t2.im, t);
        } else if (projection == u32(2)) {
            let hb = pt2 / size;
            let w = max(1.0e-6, abs(hb.im));
            let hx = hb.re / w;
            let s2 = hb.re * hb.re + w * w;
            let t = (1.0 + s2) / (2.0 * w);
            x = vec3(hx, t - 1.0 / w, t);
        } else {
            let d = max(1.0e-9, 1.0 - r2);
            let t3 = 2.0 * b / d;
            x = vec3(t3.re, t3.im, (1.0 + r2) / d);
        }
    }
    let creg = this.creg;
    let prev = u32(99);
    for i in 0..steps {
        let idx = min(u32(rand() * 3.0), u32(2));
        if (idx == prev) {
            idx = (idx + u32(1) + min(u32(rand() * 2.0), u32(1))) % u32(3);
        }
        prev = idx;
        let n = vec3(mirrors[3 * idx + 0], mirrors[3 * idx + 1], mirrors[3 * idx + 2]);
        x = x - 2.0 * honeycomb_mdot2(x, n) * n;
        if (dc_mode == u32(1)) {
            creg = mix(creg, fract((to_float(idx) + 0.5) / 3.0 * dc_scale), color_speed);
        }
    }
    if (dc_mode == u32(1)) {
        this.creg = creg;
        color = creg;
    }
    depth = depth + to_float(steps);
    this.depth = depth;
    let t = max(1.0, x.z);
    let out = complex(0.0, 0.0);
    if (projection == u32(1)) {
        out = (complex(x.x, x.y) / t) * size;
    } else if (projection == u32(2)) {
        let iw = max(1.0e-6, x.z - x.y);
        out = complex(x.x / iw, 1.0 / iw) * size;
    } else {
        out = (complex(x.x, x.y) / (1.0 + t)) * size;
    }
    if (dc_mode == u32(2)) {
        color = fract(out.abs() / size * dc_scale);
    } else if (dc_mode == u32(3)) {
        if (seed_mode != u32(0)) {
            let g = max(1.0, depth / max(1.0, to_float(steps)));
            let q = 0.9;
            let t = 1.0 - pow(q, g - 1.0) * (1.0 + q) * 0.5;
            color = fract(t * dc_scale);
        } else {
            color = fract(depth * color_speed * 0.1 * dc_scale);
        }
    }
    #{ z: out, color: color }
}

fn variation3(pt, p, ctx) {
    let color = ctx.color;
    let sp = to_float(p.p);
    let sq = to_float(p.q);
    let sr = to_float(p.r);
    let size = max(1.0e-6, p.size);
    let steps = to_int(to_float(p.steps));
    let projection = u32(to_float(p.projection));
    let dc_mode = u32(to_float(p.dc_mode));
    let dc_scale = p.dc_scale;
    let color_speed = p.color_speed;
    let pi = 3.14159265359;
    let cp = cos(pi / max(2.0, sp));
    let cq = cos(pi / max(2.0, sq));
    let cr = cos(pi / max(2.0, sr));
    let s1 = sqrt(max(1.0e-6, 1.0 - cp * cp));
    let y2 = -cq / s1;
    let a = 1.0 - y2 * y2;
    let z2 = 0.0;
    let w2 = 0.0;
    let z3 = 0.0;
    let w3 = 0.0;
    if (a > 1.0e-4) {
        z2 = sqrt(a);
        z3 = -cr / z2;
        w3 = sqrt(max(0.0, z3 * z3 - 1.0));
    } else if (a < -1.0e-4) {
        w2 = sqrt(-a);
        w3 = cr / w2;
        z3 = sqrt(1.0 + w3 * w3);
    } else {
        z2 = 1.0;
        w2 = 1.0;
        let crs = max(1.0e-3, cr);
        z3 = -0.5 * (crs + 1.0 / crs);
        w3 = 0.5 * (crs - 1.0 / crs);
    }
    let mirrors = [1.0, 0.0, 0.0, 0.0, -cp, s1, 0.0, 0.0, 0.0, y2, z2, w2, 0.0, 0.0, z3, w3];
    let seed_mode = u32(to_float(p.seed));
    let thickness = p.thickness;
    let variant = u32(to_float(p.variant));
    let depth = this.depth;
    let x = vec4(0.0, 0.0, 0.0, 0.0);
    if (seed_mode != u32(0) && rand() < 0.1) {
        let mask = u32(1);
        let t1 = variant;
        if t1 == u32(1) {
            mask = u32(2);
        } else if t1 == u32(2) {
            mask = u32(3);
        } else if t1 == u32(3) {
            mask = u32(6);
        } else if t1 == u32(4) {
            mask = u32(5);
        } else if t1 == u32(5) {
            mask = u32(7);
        } else if t1 == u32(6) {
            mask = u32(11);
        } else if t1 == u32(7) {
            mask = u32(13);
        } else if t1 == u32(8) {
            mask = u32(15);
        } else {
            mask = u32(1);
        }
        let rc0 = (if (mask & u32(1)) != u32(0) { -1.0 } else { 0.0 });
        let rc1 = (if (mask & u32(2)) != u32(0) { -1.0 } else { 0.0 });
        let rc2 = (if (mask & u32(4)) != u32(0) { -1.0 } else { 0.0 });
        let rc3 = (if (mask & u32(8)) != u32(0) { -1.0 } else { 0.0 });
        let px = rc0;
        let py = (rc1 + cp * px) / s1;
        let b2 = rc2 - y2 * py;
        let dd = w2 * z3 - z2 * w3;
        if (abs(dd) < 1.0e-6) {
            dd = (if dd >= 0.0 { 1.0e-6 } else { -1.0e-6 });
        }
        let pz = (w2 * rc3 - w3 * b2) / dd;
        let pw = (z2 * rc3 - z3 * b2) / dd;
        let pm = vec4(px, py, pz, pw);
        if (pm.w < 0.0) {
            pm = -pm;
        }
        let wp = pm / sqrt(max(1.0e-6, -honeycomb_mdot(pm, pm)));
        let m = wp;
        if (seed_mode >= u32(2)) {
            let nring = to_float(count_one_bits(mask));
            let pick = to_int(min(rand() * nring, nring - 0.5));
            let mi = u32(0);
            for i_n in 0..4 {
                let i = u32(i_n);
                if ((mask & (u32(1) << i)) != u32(0)) {
                    if (pick == 0) {
                        mi = i;
                        break;
                    }
                    pick = pick - 1;
                }
            }
            let f1 = wp - honeycomb_mdot(wp, vec4(mirrors[4 * mi + 0], mirrors[4 * mi + 1], mirrors[4 * mi + 2], mirrors[4 * mi + 3])) * vec4(mirrors[4 * mi + 0], mirrors[4 * mi + 1], mirrors[4 * mi + 2], mirrors[4 * mi + 3]);
            f1 = f1 / sqrt(max(1.0e-6, -honeycomb_mdot(f1, f1)));
            if (seed_mode == u32(2)) {
                m = mix(wp, f1, rand());
            } else {
                let mj = (mi + u32(1) + min(u32(rand() * 3.0), u32(2))) % u32(4);
                for ttry_n in 0..3 {
                    let ttry = u32(ttry_n);
                    if (mj != mi && abs(honeycomb_mdot(f1, vec4(mirrors[4 * mj + 0], mirrors[4 * mj + 1], mirrors[4 * mj + 2], mirrors[4 * mj + 3]))) > 1.0e-4) {
                        break;
                    }
                    mj = (mj + u32(1)) % u32(4);
                    if (mj == mi) {
                        mj = (mj + u32(1)) % u32(4);
                    }
                }
                let f2 = f1 - honeycomb_mdot(f1, vec4(mirrors[4 * mj + 0], mirrors[4 * mj + 1], mirrors[4 * mj + 2], mirrors[4 * mj + 3])) * vec4(mirrors[4 * mj + 0], mirrors[4 * mj + 1], mirrors[4 * mj + 2], mirrors[4 * mj + 3]);
                f2 = f2 / sqrt(max(1.0e-6, -honeycomb_mdot(f2, f2)));
                let u = rand();
                let v = rand() * (1.0 - u);
                m = wp + u * (f1 - wp) + v * (f2 - wp);
            }
        }
        x = m / sqrt(max(1.0e-6, -honeycomb_mdot(m, m)));
        if (thickness > 0.0) {
            let e1 = vec4(1.0, 0.0, 0.0, 0.0);
            e1 = e1 + honeycomb_mdot(e1, x) * x;
            e1 = e1 / sqrt(max(1.0e-6, honeycomb_mdot(e1, e1)));
            let e2 = vec4(0.0, 1.0, 0.0, 0.0);
            e2 = e2 + honeycomb_mdot(e2, x) * x;
            e2 = e2 - honeycomb_mdot(e2, e1) * e1;
            e2 = e2 / sqrt(max(1.0e-6, honeycomb_mdot(e2, e2)));
            let e3 = vec4(0.0, 0.0, 1.0, 0.0);
            e3 = e3 + honeycomb_mdot(e3, x) * x;
            e3 = e3 - honeycomb_mdot(e3, e1) * e1;
            e3 = e3 - honeycomb_mdot(e3, e2) * e2;
            e3 = e3 / sqrt(max(1.0e-6, honeycomb_mdot(e3, e3)));
            let zz = rand() * 2.0 - 1.0;
            let ph = rand() * 6.28318530718;
            let rxy = sqrt(max(0.0, 1.0 - zz * zz));
            let u = (rxy * cos(ph)) * e1 + (rxy * sin(ph)) * e2 + zz * e3;
            x = cosh(thickness) * x + sinh(thickness) * u;
        }
        depth = 0.0;
    } else {
        let b = pt / size;
        let r2 = dot(b, b);
        if (r2 >= 1.0) {
            b = b / (r2 + 1.0e-9);
            r2 = dot(b, b);
        }
        if (projection == u32(1)) {
            let t = 1.0 / sqrt(max(1.0e-9, 1.0 - r2));
            let t2 = b * t;
            x = vec4(t2.x, t2.y, t2.z, t);
        } else if (projection == u32(2)) {
            let hb = pt / size;
            let w = max(1.0e-6, abs(hb.z));
            let hx = hb.x / w;
            let hy = hb.y / w;
            let s2 = hb.x * hb.x + hb.y * hb.y + w * w;
            let t = (1.0 + s2) / (2.0 * w);
            x = vec4(hx, hy, t - 1.0 / w, t);
        } else {
            let d = max(1.0e-9, 1.0 - r2);
            let t3 = 2.0 * b / d;
            x = vec4(t3.x, t3.y, t3.z, (1.0 + r2) / d);
        }
    }
    let creg = this.creg;
    let prev = u32(99);
    for i in 0..steps {
        let idx = min(u32(rand() * 4.0), u32(3));
        if (idx == prev) {
            idx = (idx + u32(1) + min(u32(rand() * 3.0), u32(2))) % u32(4);
        }
        prev = idx;
        let n = vec4(mirrors[4 * idx + 0], mirrors[4 * idx + 1], mirrors[4 * idx + 2], mirrors[4 * idx + 3]);
        x = x - 2.0 * honeycomb_mdot(x, n) * n;
        if (dc_mode == u32(1)) {
            creg = mix(creg, fract((to_float(idx) + 0.5) / 4.0 * dc_scale), color_speed);
        }
    }
    if (dc_mode == u32(1)) {
        this.creg = creg;
        color = creg;
    }
    depth = depth + to_float(steps);
    this.depth = depth;
    let t = max(1.0, x.w);
    let out = vec3(0.0, 0.0, 0.0);
    if (projection == u32(1)) {
        out = (vec3(x.x, x.y, x.z) / t) * size;
    } else if (projection == u32(2)) {
        let iw = max(1.0e-6, x.w - x.z);
        out = vec3(x.x / iw, x.y / iw, 1.0 / iw) * size;
    } else {
        out = (vec3(x.x, x.y, x.z) / (1.0 + t)) * size;
    }
    if (dc_mode == u32(2)) {
        color = 0.5 + 0.5 * tanh(2.0 * dc_scale * out.z / size);
    } else if (dc_mode == u32(3)) {
        if (seed_mode != u32(0)) {
            let g = max(1.0, depth / max(1.0, to_float(steps)));
            let q = 0.9;
            let t = 1.0 - pow(q, g - 1.0) * (1.0 + q) * 0.5;
            color = fract(t * dc_scale);
        } else {
            color = fract(depth * color_speed * 0.1 * dc_scale);
        }
    }
    #{ z: out, color: color }
}
