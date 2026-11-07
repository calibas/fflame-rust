/// Hyperbolic H⁴ honeycomb reflection walk (Schläfli {p,q,r,s}).
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5

// Hyperbolic Honeycomb 4D, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    p: #{
        value: 5, min: 2, max: 12,
        caption: "P",
        hint: "First Schläfli number. Compact hyperbolic H⁴ honeycombs: {5,3,3,4}, {5,3,3,5}, {4,3,3,5}, {3,3,3,5}, {5,3,3,3}; {4,3,3,4} is the Euclidean tesseractic honeycomb. Other combinations degrade to spherical/Euclidean/non-discrete kaleidoscopes.",
    },
    q: #{
        value: 3, min: 2, max: 12,
        caption: "Q",
        hint: "Second Schläfli number ({p,q,r} is the 4-polytope cell).",
    },
    r: #{
        value: 3, min: 2, max: 12,
        caption: "R",
        hint: "Third Schläfli number (4D). In 2D render mode this is the third triangle angle — (p,q,r) becomes the general Fuchsian triangle group; r = 2 gives the {p,q} tiling.",
    },
    s: #{
        value: 4, min: 2, max: 12,
        caption: "S",
        hint: "Fourth Schläfli number: cells around each 2-face.",
    },
    size: #{
        value: 1.0, min: 0.1, max: 4.0,
        caption: "Size",
        hint: "Radius of the 4-ball model in world units.",
    },
    steps: #{
        value: 2, min: 1, max: 8,
        caption: "Steps",
        hint: "Random mirror reflections per call (5 mirrors).",
    },
    projection: #{
        value: 0, choices: ["Poincaré", "Beltrami–Klein", "Half-Space"],
        caption: "Projection",
        hint: "Model of H⁴ for the plotted (and fed-forward) 4-ball point; the xyz shadow is plotted and the 4th ball coordinate rides the per-thread w register. Half-Space uses the 4th coordinate as the height above the floor hyperplane.",
    },
    dc_mode: #{
        value: 0, choices: ["Off", "Mirror", "Depth", "Steps"],
        caption: "Color Mode",
        hint: "Direct-color source. Mirror: each of the 5 orthoscheme mirrors has its own palette position, blended through the persistent color register at Color Speed. Depth: output z. Steps: palette cycles with the walk depth since the last reseed (wraps, so deep levels stay distinct).",
    },
    dc_scale: #{
        value: 1.0, min: 0.1, max: 8.0,
        caption: "Color Scale",
        hint: "Palette-index multiplier for the color modes.",
    },
    color_speed: #{
        value: 0.5, min: 0.0, max: 1.0,
        caption: "Color Speed",
        hint: "Mirror mode: pull strength toward each mirror's palette position. Steps mode: palette advance per reflection (cyclic — wraps instead of saturating).",
    },
    seed: #{
        value: 2, choices: ["Input", "Vertices", "Edges", "Faces"],
        caption: "Seed",
        hint: "What the walk stamps through the honeycomb: the incoming flame measure, the vertex orbit, the edge skeleton, or flag-fragment faces (see honeycomb — same semantics, one dimension up).",
    },
    thickness: #{
        value: 0.01, min: 0.0, max: 2.0,
        caption: "Thickness",
        hint: "Seed modes: geodesic tangent-space offset by exact hyperbolic distance — balls / tubes / slabs with uniform hyperbolic radius (see honeycomb).",
    },
    variant: #{
        value: 0, choices: ["Normal", "Rectified", "Truncated", "Bitruncated", "Cantellated", "Cantitruncated", "Runcitruncated", "Runcicantellated", "Omnitruncated"],
        caption: "Variant",
        hint: "Wythoff variant: ring mask over the first four Coxeter nodes (the fifth stays unringed). Same construction as honeycomb, one dimension up.",
    },
};

const INFO = #{ title: "Hyperbolic Honeycomb 4D", name: "rhai_honeycomb4d", category: "advanced_2d" };

fn init(p) {
    #{ creg: 0.0, depth: 0.0 }
}

fn honeycomb4d_mdot2(a, b) {
    a.x * b.x + a.y * b.y - a.z * b.z
}

fn honeycomb4d_mdot(a_s, a_t, b_s, b_t) {
    (a_s.x * b_s.x + a_s.y * b_s.y + a_s.z * b_s.z + a_s.w * b_s.w) - a_t * b_t
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

fn variation(w, p, ctx) {
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
    let depth = this.depth;
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
    let variant = u32(to_float(p.variant));
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
        let wp = pm / sqrt(max(1.0e-6, -honeycomb4d_mdot2(pm, pm)));
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
            let f1 = wp - honeycomb4d_mdot2(wp, vec3(mirrors[3 * mi + 0], mirrors[3 * mi + 1], mirrors[3 * mi + 2])) * vec3(mirrors[3 * mi + 0], mirrors[3 * mi + 1], mirrors[3 * mi + 2]);
            f1 = f1 / sqrt(max(1.0e-6, -honeycomb4d_mdot2(f1, f1)));
            if (seed_mode == u32(2)) {
                m = mix(wp, f1, rand());
            } else {
                let mj = (mi + u32(1) + min(u32(rand() * 2.0), u32(1))) % u32(3);
                for ttry_n in 0..2 {
                    let ttry = u32(ttry_n);
                    if (mj != mi && abs(honeycomb4d_mdot2(f1, vec3(mirrors[3 * mj + 0], mirrors[3 * mj + 1], mirrors[3 * mj + 2]))) > 1.0e-4) {
                        break;
                    }
                    mj = (mj + u32(1)) % u32(3);
                    if (mj == mi) {
                        mj = (mj + u32(1)) % u32(3);
                    }
                }
                let f2 = f1 - honeycomb4d_mdot2(f1, vec3(mirrors[3 * mj + 0], mirrors[3 * mj + 1], mirrors[3 * mj + 2])) * vec3(mirrors[3 * mj + 0], mirrors[3 * mj + 1], mirrors[3 * mj + 2]);
                f2 = f2 / sqrt(max(1.0e-6, -honeycomb4d_mdot2(f2, f2)));
                let u = rand();
                let v = rand() * (1.0 - u);
                m = wp + u * (f1 - wp) + v * (f2 - wp);
            }
        }
        x = m / sqrt(max(1.0e-6, -honeycomb4d_mdot2(m, m)));
        if (thickness > 0.0) {
            let ph = rand() * 6.28318530718;
            let tt = thickness * sqrt(rand());
            let e1 = vec3(1.0, 0.0, 0.0);
            e1 = e1 + honeycomb4d_mdot2(e1, x) * x;
            e1 = e1 / sqrt(max(1.0e-6, honeycomb4d_mdot2(e1, e1)));
            let e2 = vec3(0.0, 1.0, 0.0);
            e2 = e2 + honeycomb4d_mdot2(e2, x) * x;
            e2 = e2 - honeycomb4d_mdot2(e2, e1) * e1;
            e2 = e2 / sqrt(max(1.0e-6, honeycomb4d_mdot2(e2, e2)));
            let u = cos(ph) * e1 + sin(ph) * e2;
            x = cosh(tt) * x + sinh(tt) * u;
        }
        depth = 0.0;
    } else {
        let b = w / size;
        let r2 = b.norm_sqr();
        if (r2 >= 1.0) {
            b = b / (r2 + 1.0e-9);
            r2 = b.norm_sqr();
        }
        if (projection == u32(1)) {
            let t = 1.0 / sqrt(max(1.0e-9, 1.0 - r2));
            let t2 = b * t;
            x = vec3(t2.re, t2.im, t);
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
        x = x - 2.0 * honeycomb4d_mdot2(x, n) * n;
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

fn variation3(pv, p, ctx) {
    let color = ctx.color;
    let w_out = 0.0;
    let sp = to_float(p.p);
    let sq = to_float(p.q);
    let sr = to_float(p.r);
    let ss = to_float(p.s);
    let size = max(1.0e-6, p.size);
    let steps = to_int(to_float(p.steps));
    let projection = u32(to_float(p.projection));
    let dc_mode = u32(to_float(p.dc_mode));
    let dc_scale = p.dc_scale;
    let color_speed = p.color_speed;
    let seed_mode = u32(to_float(p.seed));
    let thickness = p.thickness;
    let variant = u32(to_float(p.variant));
    let depth = this.depth;
    let pi = 3.14159265359;
    let cp = cos(pi / max(2.0, sp));
    let cq = cos(pi / max(2.0, sq));
    let cr = cos(pi / max(2.0, sr));
    let cs = cos(pi / max(2.0, ss));
    let s1 = sqrt(max(1.0e-6, 1.0 - cp * cp));
    let y2 = -cq / s1;
    let z2 = sqrt(max(1.0e-6, 1.0 - y2 * y2));
    let z3 = -cr / z2;
    let u3 = sqrt(max(1.0e-6, 1.0 - z3 * z3));
    let u4 = -cs / u3;
    let t4 = sqrt(max(0.0, u4 * u4 - 1.0));
    let mirror_s = [1.0, 0.0, 0.0, 0.0, -cp, s1, 0.0, 0.0, 0.0, y2, z2, 0.0, 0.0, 0.0, z3, u3, 0.0, 0.0, 0.0, u4];
    let mirror_t = [0.0, 0.0, 0.0, 0.0, t4];
    let xs = vec4(0.0, 0.0, 0.0, 0.0);
    let xt = 0.0;
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
        let pz = (rc2 - y2 * py) / max(1.0e-6, z2);
        let pu = (rc3 - z3 * pz) / max(1.0e-6, u3);
        let pt = u4 * pu / max(1.0e-6, t4);
        let pms = vec4(px, py, pz, pu);
        let pmt = pt;
        if (pmt < 0.0) {
            pms = -pms;
            pmt = -pmt;
        }
        let nn = sqrt(max(1.0e-6, -honeycomb4d_mdot(pms, pmt, pms, pmt)));
        let wps = pms / nn;
        let wpt = pmt / nn;
        let ms = wps;
        let mt = wpt;
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
            let d1 = honeycomb4d_mdot(wps, wpt, vec4(mirror_s[4 * mi + 0], mirror_s[4 * mi + 1], mirror_s[4 * mi + 2], mirror_s[4 * mi + 3]), mirror_t[mi]);
            let f1s = wps - d1 * vec4(mirror_s[4 * mi + 0], mirror_s[4 * mi + 1], mirror_s[4 * mi + 2], mirror_s[4 * mi + 3]);
            let f1t = wpt - d1 * mirror_t[mi];
            nn = sqrt(max(1.0e-6, -honeycomb4d_mdot(f1s, f1t, f1s, f1t)));
            f1s = f1s / nn;
            f1t = f1t / nn;
            if (seed_mode == u32(2)) {
                let tt = rand();
                ms = mix(wps, f1s, tt);
                mt = mix(wpt, f1t, tt);
            } else {
                let mj = (mi + u32(1) + min(u32(rand() * 4.0), u32(3))) % u32(5);
                for ttry_n in 0..4 {
                    let ttry = u32(ttry_n);
                    if (mj != mi && abs(honeycomb4d_mdot(f1s, f1t, vec4(mirror_s[4 * mj + 0], mirror_s[4 * mj + 1], mirror_s[4 * mj + 2], mirror_s[4 * mj + 3]), mirror_t[mj])) > 1.0e-4) {
                        break;
                    }
                    mj = (mj + u32(1)) % u32(5);
                    if (mj == mi) {
                        mj = (mj + u32(1)) % u32(5);
                    }
                }
                let d2 = honeycomb4d_mdot(f1s, f1t, vec4(mirror_s[4 * mj + 0], mirror_s[4 * mj + 1], mirror_s[4 * mj + 2], mirror_s[4 * mj + 3]), mirror_t[mj]);
                let f2s = f1s - d2 * vec4(mirror_s[4 * mj + 0], mirror_s[4 * mj + 1], mirror_s[4 * mj + 2], mirror_s[4 * mj + 3]);
                let f2t = f1t - d2 * mirror_t[mj];
                nn = sqrt(max(1.0e-6, -honeycomb4d_mdot(f2s, f2t, f2s, f2t)));
                f2s = f2s / nn;
                f2t = f2t / nn;
                let u = rand();
                let v = rand() * (1.0 - u);
                ms = wps + u * (f1s - wps) + v * (f2s - wps);
                mt = wpt + u * (f1t - wpt) + v * (f2t - wpt);
            }
        }
        nn = sqrt(max(1.0e-6, -honeycomb4d_mdot(ms, mt, ms, mt)));
        xs = ms / nn;
        xt = mt / nn;
        if (thickness > 0.0) {
            let e1s = vec4(1.0, 0.0, 0.0, 0.0);
            let e1t = 0.0;
            let cd = honeycomb4d_mdot(e1s, e1t, xs, xt);
            e1s = e1s + cd * xs;
            e1t = e1t + cd * xt;
            nn = sqrt(max(1.0e-6, honeycomb4d_mdot(e1s, e1t, e1s, e1t)));
            e1s = e1s / nn;
            e1t = e1t / nn;
            let e2s = vec4(0.0, 1.0, 0.0, 0.0);
            let e2t = 0.0;
            cd = honeycomb4d_mdot(e2s, e2t, xs, xt);
            e2s = e2s + cd * xs;
            e2t = e2t + cd * xt;
            cd = honeycomb4d_mdot(e2s, e2t, e1s, e1t);
            e2s = e2s - cd * e1s;
            e2t = e2t - cd * e1t;
            nn = sqrt(max(1.0e-6, honeycomb4d_mdot(e2s, e2t, e2s, e2t)));
            e2s = e2s / nn;
            e2t = e2t / nn;
            let e3s = vec4(0.0, 0.0, 1.0, 0.0);
            let e3t = 0.0;
            cd = honeycomb4d_mdot(e3s, e3t, xs, xt);
            e3s = e3s + cd * xs;
            e3t = e3t + cd * xt;
            cd = honeycomb4d_mdot(e3s, e3t, e1s, e1t);
            e3s = e3s - cd * e1s;
            e3t = e3t - cd * e1t;
            cd = honeycomb4d_mdot(e3s, e3t, e2s, e2t);
            e3s = e3s - cd * e2s;
            e3t = e3t - cd * e2t;
            nn = sqrt(max(1.0e-6, honeycomb4d_mdot(e3s, e3t, e3s, e3t)));
            e3s = e3s / nn;
            e3t = e3t / nn;
            let e4s = vec4(0.0, 0.0, 0.0, 1.0);
            let e4t = 0.0;
            cd = honeycomb4d_mdot(e4s, e4t, xs, xt);
            e4s = e4s + cd * xs;
            e4t = e4t + cd * xt;
            cd = honeycomb4d_mdot(e4s, e4t, e1s, e1t);
            e4s = e4s - cd * e1s;
            e4t = e4t - cd * e1t;
            cd = honeycomb4d_mdot(e4s, e4t, e2s, e2t);
            e4s = e4s - cd * e2s;
            e4t = e4t - cd * e2t;
            cd = honeycomb4d_mdot(e4s, e4t, e3s, e3t);
            e4s = e4s - cd * e3s;
            e4t = e4t - cd * e3t;
            nn = sqrt(max(1.0e-6, honeycomb4d_mdot(e4s, e4t, e4s, e4t)));
            e4s = e4s / nn;
            e4t = e4t / nn;
            let uu = rand();
            let th1 = rand() * 6.28318530718;
            let th2 = rand() * 6.28318530718;
            let ra = sqrt(1.0 - uu);
            let rb = sqrt(uu);
            let d1 = ra * cos(th1);
            let d2 = ra * sin(th1);
            let d3 = rb * cos(th2);
            let d4 = rb * sin(th2);
            let us = d1 * e1s + d2 * e2s + d3 * e3s + d4 * e4s;
            let ut = d1 * e1t + d2 * e2t + d3 * e3t + d4 * e4t;
            let ch = cosh(thickness);
            let sh = sinh(thickness);
            xs = ch * xs + sh * us;
            xt = ch * xt + sh * ut;
        }
        depth = 0.0;
    } else {
        let b = vec4(pv.x, pv.y, pv.z, ctx.w) / size;
        if (projection == u32(2)) {
            let w = max(1.0e-6, abs(b.w));
            let bs = vec3(b.x, b.y, b.z) / w;
            let s2 = b.x * b.x + b.y * b.y + b.z * b.z + w * w;
            let t = (1.0 + s2) / (2.0 * w);
            xs = vec4(bs.x, bs.y, bs.z, t - 1.0 / w);
            xt = t;
        } else {
            let r2 = (b.x * b.x + b.y * b.y + b.z * b.z + b.w * b.w);
            if (r2 >= 1.0) {
                b = b / (r2 + 1.0e-9);
                r2 = (b.x * b.x + b.y * b.y + b.z * b.z + b.w * b.w);
            }
            if (projection == u32(1)) {
                let t = 1.0 / sqrt(max(1.0e-9, 1.0 - r2));
                xs = b * t;
                xt = t;
            } else {
                let d = max(1.0e-9, 1.0 - r2);
                xs = 2.0 * b / d;
                xt = (1.0 + r2) / d;
            }
        }
    }
    let creg = this.creg;
    let prev = u32(99);
    for i in 0..steps {
        let idx = min(u32(rand() * 5.0), u32(4));
        if (idx == prev) {
            idx = (idx + u32(1) + min(u32(rand() * 4.0), u32(3))) % u32(5);
        }
        prev = idx;
        let d = honeycomb4d_mdot(xs, xt, vec4(mirror_s[4 * idx + 0], mirror_s[4 * idx + 1], mirror_s[4 * idx + 2], mirror_s[4 * idx + 3]), mirror_t[idx]);
        xs = xs - 2.0 * d * vec4(mirror_s[4 * idx + 0], mirror_s[4 * idx + 1], mirror_s[4 * idx + 2], mirror_s[4 * idx + 3]);
        xt = xt - 2.0 * d * mirror_t[idx];
        if (dc_mode == u32(1)) {
            creg = mix(creg, fract((to_float(idx) + 0.5) / 5.0 * dc_scale), color_speed);
        }
    }
    if (dc_mode == u32(1)) {
        this.creg = creg;
        color = creg;
    }
    depth = depth + to_float(steps);
    this.depth = depth;
    let t = max(1.0, xt);
    let out4 = vec4(0.0, 0.0, 0.0, 0.0);
    if (projection == u32(1)) {
        out4 = (xs / t) * size;
    } else if (projection == u32(2)) {
        let iw = max(1.0e-6, xt - xs.w);
        out4 = vec4(xs.x / iw, xs.y / iw, xs.z / iw, 1.0 / iw) * size;
    } else {
        out4 = (xs / (1.0 + t)) * size;
    }
    w_out = out4.w;
    if (dc_mode == u32(2)) {
        color = 0.5 + 0.5 * tanh(2.0 * dc_scale * out4.z / size);
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
    #{ z: vec3(out4.x, out4.y, out4.z), color: color, w: w_out }
}
