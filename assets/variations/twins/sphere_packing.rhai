/// Apollonian & ring sphere packings (3D) / circle packings (2D) via
/// inversions, with tangency-graph seeding — the genuinely 3D Kleinian.
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5

// Sphere Packing, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    mode: #{
        value: 0, choices: ["Apollonian (Dual)", "Tangent Spheres", "Ring", "Ring + Caps"],
        caption: "Mode",
        hint: "The mirror configuration. Apollonian: inversions in the five dual spheres of the symmetric Soddy configuration — the limit set is the exact Apollonian sphere packing (gasket in 2D). Tangent Spheres: the five Soddy spheres themselves as mirrors — denser sibling fractal. Ring: N spheres around the equator kissing the outer sphere (and each other at Ring Scale 1) — a freely tunable packing family. Ring + Caps: adds two polar cap spheres kissing outer + ring (3D; renders as Ring in 2D).",
    },
    size: #{
        value: 1.0, min: 0.1, max: 4.0,
        caption: "Size",
        hint: "Radius of the outer sphere/circle in world units.",
    },
    avoid_repeat: #{
        value: true,
        caption: "Avoid Repeat",
        hint: "Block choosing the same mirror twice in a row (an inversion is an involution, so a repeat cancels to the identity). Off = all mirrors equally likely every call.",
    },
    dc_mode: #{
        value: 0, choices: ["Off", "Mirror", "Mirror Blend", "Curvature", "Generation", "Angle"],
        caption: "Color Mode",
        hint: "Direct-color source (needs the transform's Direct Color > 0). Mirror: the last inversion applied — coarse flat regions (each mirror maps everything into its own ball; Color Speed unused). Mirror Blend: register pulled toward each mirror's slot after EVERY inversion — Color Speed is the region size (1 = coarse basins, lower = each step refines one hierarchy level). Curvature: the canonical Apollonian coloring — the carried sphere is transported through every inversion in closed form, so each sphere of the packing gets its own palette position by SIZE (needs Reseed > 0). Generation (uses Color Scale as palette cycles; ignores Color Speed, since spacing is derived from Reseed): palette sweeps with the number of inversions since the last reseed — colors by hierarchy depth. Angle: azimuth of the output point.",
    },
    dc_scale: #{
        value: 1.0, min: 0.1, max: 8.0,
        caption: "Color Scale",
        hint: "Palette-index multiplier for the color modes.",
    },
    color_speed: #{
        value: 0.5, min: 0.0, max: 1.0,
        caption: "Color Speed",
        hint: "Mirror Blend: blend rate = color region size (1 = coarse per-mirror basins, low = fine cells from deep itinerary). Generation: palette sweep rate per hierarchy level.",
    },
    steps: #{
        value: 3, min: 1, max: 8,
        caption: "Steps",
        hint: "Inversions applied per call (only the last point is plotted). Higher values let respawned points converge onto the packing before plotting — suppresses pre-convergence haze — at proportional GPU cost.",
    },
    reseed: #{
        value: 0.25, min: 0.0, max: 0.5,
        caption: "Reseed",
        hint: "Probability per call of planting the point on the seed geometry (see Seed) before the walk. The packing spheres are exact limit-set subsets for the tangent configurations, so seeded points render crisply from the first plot. 0 = pure feed-through chaos game.",
    },
    ring_n: #{
        value: 6, min: 2, max: 16,
        caption: "Ring N",
        hint: "Ring modes: number of spheres around the equator. At Ring Scale 1 neighbours kiss exactly (r = sin(π/N)/(1+sin(π/N))).",
    },
    ring_scale: #{
        value: 1.0, min: 0.3, max: 1.5,
        caption: "Ring Scale",
        hint: "Ring modes: radius multiplier on the tangent ring size. Each sphere stays kissing the OUTER sphere (it slides outward as it shrinks); < 1 opens dust gaps between neighbours, > 1 overlaps them — different fractal regimes.",
    },
    cap_scale: #{
        value: 1.0, min: 0.0, max: 1.5,
        caption: "Cap Scale",
        hint: "Ring + Caps: radius multiplier on the polar cap spheres (exact outer+ring tangency at 1; 0 removes them).",
    },
    size_jitter: #{
        value: 0.0, min: 0.0, max: 1.0,
        caption: "Size Jitter",
        hint: "Ring modes: deterministic per-sphere radius variation (each ring sphere shrinks by up to this fraction, hashed by index) — de-uniformizes the packing while every sphere keeps kissing the outer sphere.",
    },
    seed: #{
        value: 0, choices: ["Surfaces", "Centers", "Edges"],
        caption: "Seed",
        hint: "What Reseed plants. Surfaces: random point on a random configuration sphere (exact limit-set subset). Centers: the sphere centers as a vertex constellation — their orbit is its own fractal cloud. Edges: the tangency graph — straight segments joining centers of kissing spheres (the inner tetrahedron for Soddy, the ring cycle + cap spokes for rings), honeycomb-style.",
    },
    thickness: #{
        value: 0.0, min: 0.0, max: 0.5,
        caption: "Thickness",
        hint: "Euclidean jitter radius fattening Centers into balls and Edges into tubes.",
    },
    ring_tilt: #{
        value: 0.0, min: -360.0, max: 360.0, type: "angle",
        caption: "Ring Tilt",
        hint: "Ring modes (3D): alternate spheres tilt up/down in latitude by this angle — an antiprism crown instead of a flat equatorial ring. Outer-sphere tangency is preserved exactly for any tilt; the sphere radius auto-shrinks to whichever is tighter of adjacent-pair tangency and SAME-parity-pair tangency (same-side spheres crowd toward the pole as tilt grows — without the cap the mirrors overlap and the group degenerates into blur). Even Ring N closes the crown perfectly; odd N leaves one seam pair.",
    },
};

const INFO = #{ title: "Sphere Packing", name: "rhai_sphere_packing", category: "full_3d" };

fn init(p) {
    #{ st0: 0.0, creg: 0.0, depth: 0.0, crad: 0.0, st4: 0.0, st5: 0.0, st6: 0.0 }
}

fn sp_hash01(k) {
    fract(sin(to_float(k) * 127.1 + 311.7) * 43758.5453)
}

fn sp_conf2(mode, k, n, rs, jit) {
    if (mode >= u32(2)) {
        if (k == u32(0)) {
            return vec3(0.0, 0.0, 1.0);
        }
        let i = k - u32(1);
        let rt = sin(3.14159265359 / to_float(n)) / (1.0 + sin(3.14159265359 / to_float(n)));
        let r = rt * rs * (1.0 - jit * sp_hash01(i));
        let d = 1.0 - r;
        let a = 6.28318530718 * to_float(i) / to_float(n);
        return vec3(d * cos(a), d * sin(a), r);
    }
    let t1 = k;
    if t1 == u32(0) {
        vec3(0.0, 0.0, 1.0)
    } else if t1 == u32(1) {
        vec3(0.0, 0.5358984, 0.4641016)
    } else if t1 == u32(2) {
        vec3(-0.4641016, -0.2679492, 0.4641016)
    } else {
        vec3(0.4641016, -0.2679492, 0.4641016)
    }
}

fn sp_mirror2(mode, k, n, rs, jit) {
    if (mode != u32(0)) {
        return sp_conf2(mode, k, n, rs, jit);
    }
    let t1 = k;
    if t1 == u32(0) {
        vec3(0.0, 0.0, 0.2679492)
    } else if t1 == u32(1) {
        vec3(0.0, -2.0, 1.7320508)
    } else if t1 == u32(2) {
        vec3(1.7320508, 1.0, 1.7320508)
    } else {
        vec3(-1.7320508, 1.0, 1.7320508)
    }
}

fn sp_conf3(mode, k, n, rs, cs, jit, tilt) {
    if (mode >= u32(2)) {
        if (k == u32(0)) {
            return vec4(0.0, 0.0, 0.0, 1.0);
        }
        let ct = cos(tilt);
        let st = sin(tilt);
        let cth = ct * ct * cos(6.28318530718 / to_float(n)) - st * st;
        let sh2 = sqrt(max(1.0e-6, 0.5 * (1.0 - cth)));
        let rt = sh2 / (1.0 + sh2);
        if (n >= u32(3)) {
            let cth2 = ct * ct * cos(12.5663706144 / to_float(n)) + st * st;
            let shp = sqrt(max(1.0e-6, 0.5 * (1.0 - cth2)));
            rt = min(rt, shp / (1.0 + shp));
        }
        if (mode == u32(3) && k > n) {
            let rn = rt * rs;
            let dn = 1.0 - rn;
            let rho = (dn * dn + 1.0 - rn * rn - 2.0 * dn * st) / (2.0 * max(1.0e-4, 1.0 + rn - dn * st));
            let cr = max(1.0e-4, rho * cs);
            let h = 1.0 - cr;
            let sgn = (if k == n + u32(2) { -1.0 } else { 1.0 });
            return vec4(0.0, 0.0, sgn * h, cr);
        }
        let i = k - u32(1);
        let r = rt * rs * (1.0 - jit * sp_hash01(i));
        let d = 1.0 - r;
        let a = 6.28318530718 * to_float(i) / to_float(n);
        let ph = (if (i & u32(1)) == u32(1) { -tilt } else { tilt });
        return vec4(d * cos(a) * cos(ph), d * sin(a) * cos(ph), d * sin(ph), r);
    }
    let t1 = k;
    if t1 == u32(0) {
        vec4(0.0, 0.0, 0.0, 1.0)
    } else if t1 == u32(1) {
        vec4(0.3178372, 0.3178372, 0.3178372, 0.4494897)
    } else if t1 == u32(2) {
        vec4(0.3178372, -0.3178372, -0.3178372, 0.4494897)
    } else if t1 == u32(3) {
        vec4(-0.3178372, 0.3178372, -0.3178372, 0.4494897)
    } else {
        vec4(-0.3178372, -0.3178372, 0.3178372, 0.4494897)
    }
}

fn sp_mirror3(mode, k, n, rs, cs, jit, tilt) {
    if (mode != u32(0)) {
        return sp_conf3(mode, k, n, rs, cs, jit, tilt);
    }
    let t1 = k;
    if t1 == u32(0) {
        vec4(0.0, 0.0, 0.0, 0.3178372)
    } else if t1 == u32(1) {
        vec4(-1.7320508, -1.7320508, -1.7320508, 2.8284271)
    } else if t1 == u32(2) {
        vec4(-1.7320508, 1.7320508, 1.7320508, 2.8284271)
    } else if t1 == u32(3) {
        vec4(1.7320508, -1.7320508, 1.7320508, 2.8284271)
    } else {
        vec4(1.7320508, 1.7320508, -1.7320508, 2.8284271)
    }
}

fn sp_edge3(mode, e, n) {
    if (mode >= u32(2)) {
        if (mode == u32(3) && e >= n) {
            let i = e - n;
            let cap = n + u32(1) + (i / n);
            [cap, u32(1) + (i % n)]
        } else {
            [u32(1) + e, u32(1) + (e + u32(1)) % n]
        }
    } else {
        let t1 = e;
        if t1 == u32(0) {
            [u32(1), u32(2)]
        } else if t1 == u32(1) {
            [u32(1), u32(3)]
        } else if t1 == u32(2) {
            [u32(1), u32(4)]
        } else if t1 == u32(3) {
            [u32(2), u32(3)]
        } else if t1 == u32(4) {
            [u32(2), u32(4)]
        } else {
            [u32(3), u32(4)]
        }
    }
}

fn variation(w, p, ctx) {
    let color = ctx.color;
    let mode = u32(to_float(p.mode));
    let size = max(1.0e-6, p.size);
    let avoid = to_float(p.avoid_repeat) > 0.5;
    let dc_mode = u32(to_float(p.dc_mode));
    let dc_scale = p.dc_scale;
    let color_speed = p.color_speed;
    let steps = to_int(to_float(p.steps));
    let reseed = p.reseed;
    let ring_n = u32(clamp(to_float(p.ring_n), 2.0, 16.0));
    let ring_scale = p.ring_scale;
    let jit = p.size_jitter;
    let seed_mode = u32(to_float(p.seed));
    let thickness = p.thickness;
    let scnt = u32(4);
    if (mode >= u32(2)) {
        scnt = u32(1) + ring_n;
    }
    let mcnt = scnt;
    let x = w / size;
    let crad = this.crad;
    let ccen = complex(this.st4, this.st5);
    let depth = this.depth;
    if (rand() < reseed) {
        if (seed_mode == u32(0)) {
            let cs = sp_conf2(mode, min(u32(rand() * to_float(scnt)), scnt - u32(1)), ring_n, ring_scale, jit);
            let a = rand() * 6.28318530718;
            x = complex(cs.x, cs.y) + cs.z * complex(cos(a), sin(a));
            crad = cs.z;
            ccen = complex(cs.x, cs.y);
        } else if (seed_mode == u32(1)) {
            let k = u32(1) + min(u32(rand() * to_float(scnt - u32(1))), scnt - u32(2));
            let cs = sp_conf2(mode, k, ring_n, ring_scale, jit);
            x = complex(cs.x, cs.y);
            crad = cs.z;
            ccen = complex(cs.x, cs.y);
        } else {
            let inner = scnt - u32(1);
            let e = min(u32(rand() * to_float(inner)), inner - u32(1));
            let s0 = sp_conf2(mode, u32(1) + e, ring_n, ring_scale, jit);
            let t1 = sp_conf2(mode, u32(1) + (e + u32(1)) % inner, ring_n, ring_scale, jit);
            let c1 = complex(t1.x, t1.y);
            x = mix(complex(s0.x, s0.y), c1, rand());
            crad = s0.z;
            ccen = complex(s0.x, s0.y);
        }
        depth = 0.0;
        if (thickness > 0.0) {
            let a = rand() * 6.28318530718;
            x = x + thickness * sqrt(rand()) * complex(cos(a), sin(a));
        }
    }
    let prev = u32(this.st0);
    let creg = this.creg;
    let k = u32(0);
    for i in 0..steps {
        k = min(u32(rand() * to_float(mcnt)), mcnt - u32(1));
        if (avoid && prev < mcnt && k == prev) {
            k = (k + u32(1)) % mcnt;
        }
        let cr = sp_mirror2(mode, k, ring_n, ring_scale, jit);
        let v = x - complex(cr.x, cr.y);
        let nn = max(1.0e-12, v.norm_sqr());
        x = complex(cr.x, cr.y) + (cr.z * cr.z / nn) * v;
        let u2 = ccen - complex(cr.x, cr.y);
        let dd = u2.norm_sqr() - crad * crad;
        let add = max(1.0e-12, abs(dd));
        ccen = complex(cr.x, cr.y) + (cr.z * cr.z * (if dd > 0.0 { 1.0 } else if dd < 0.0 { -1.0 } else { 0.0 }) / add) * u2;
        crad = cr.z * cr.z * crad / add;
        creg = mix(creg, (to_float(k) + 0.5) / to_float(mcnt), color_speed);
        prev = k;
    }
    this.st0 = to_float(prev);
    depth = depth + to_float(steps);
    this.depth = depth;
    this.crad = crad;
    this.st4 = ccen.re;
    this.st5 = ccen.im;
    if (dc_mode == u32(1)) {
        color = fract((to_float(k) + 0.5) / to_float(mcnt) * dc_scale);
    } else if (dc_mode == u32(2)) {
        this.creg = creg;
        color = fract(creg * dc_scale);
    } else if (dc_mode == u32(3)) {
        color = fract(0.5 + 0.15 * dc_scale * ln(1.0 / max(1.0e-9, crad)));
    } else if (dc_mode == u32(4)) {
        let g = max(1.0, depth / max(1.0, to_float(steps)));
        let q = 1.0 - clamp(reseed, 0.02, 1.0);
        let t = 1.0 - pow(q, g - 1.0) * (1.0 + q) * 0.5;
        color = fract(t * dc_scale);
    } else if (dc_mode == u32(5)) {
        color = fract((atan2(x.im, x.re) * 0.15915494 + 0.5) * dc_scale);
    }
    #{ z: x * size, color: color }
}

fn variation3(pt, p, ctx) {
    let color = ctx.color;
    let mode = u32(to_float(p.mode));
    let size = max(1.0e-6, p.size);
    let avoid = to_float(p.avoid_repeat) > 0.5;
    let dc_mode = u32(to_float(p.dc_mode));
    let dc_scale = p.dc_scale;
    let color_speed = p.color_speed;
    let steps = to_int(to_float(p.steps));
    let reseed = p.reseed;
    let ring_n = u32(clamp(to_float(p.ring_n), 2.0, 16.0));
    let ring_scale = p.ring_scale;
    let cap_scale = p.cap_scale;
    let jit = p.size_jitter;
    let seed_mode = u32(to_float(p.seed));
    let thickness = p.thickness;
    let tilt = p.ring_tilt * 0.01745329252;
    let scnt = u32(5);
    if (mode == u32(2)) {
        scnt = u32(1) + ring_n;
    } else if (mode == u32(3)) {
        scnt = u32(3) + ring_n;
    }
    let mcnt = scnt;
    let ecnt = u32(6);
    if (mode == u32(2)) {
        ecnt = ring_n;
    } else if (mode == u32(3)) {
        ecnt = u32(3) * ring_n;
    }
    let x = pt / size;
    let crad = this.crad;
    let ccen = vec3(this.st4, this.st5, this.st6);
    let depth = this.depth;
    if (rand() < reseed) {
        if (seed_mode == u32(0)) {
            let sp = sp_conf3(mode, min(u32(rand() * to_float(scnt)), scnt - u32(1)), ring_n, ring_scale, cap_scale, jit, tilt);
            let za = rand() * 2.0 - 1.0;
            let ph = rand() * 6.28318530718;
            let sa = sqrt(max(0.0, 1.0 - za * za));
            x = vec3(sp.x, sp.y, sp.z) + sp.w * vec3(sa * cos(ph), sa * sin(ph), za);
            crad = sp.w;
            ccen = vec3(sp.x, sp.y, sp.z);
        } else if (seed_mode == u32(1)) {
            let k = u32(1) + min(u32(rand() * to_float(scnt - u32(1))), scnt - u32(2));
            let sp = sp_conf3(mode, k, ring_n, ring_scale, cap_scale, jit, tilt);
            x = vec3(sp.x, sp.y, sp.z);
            crad = sp.w;
            ccen = vec3(sp.x, sp.y, sp.z);
        } else {
            let e = min(u32(rand() * to_float(ecnt)), ecnt - u32(1));
            let ij = sp_edge3(mode, e, ring_n);
            let s0 = sp_conf3(mode, ij[0], ring_n, ring_scale, cap_scale, jit, tilt);
            let t1 = sp_conf3(mode, ij[1], ring_n, ring_scale, cap_scale, jit, tilt);
            let c1 = vec3(t1.x, t1.y, t1.z);
            x = mix(vec3(s0.x, s0.y, s0.z), c1, rand());
            crad = s0.w;
            ccen = vec3(s0.x, s0.y, s0.z);
        }
        depth = 0.0;
        if (thickness > 0.0) {
            let za = rand() * 2.0 - 1.0;
            let ph = rand() * 6.28318530718;
            let sa = sqrt(max(0.0, 1.0 - za * za));
            x = x + thickness * pow(rand(), 0.3333333) * vec3(sa * cos(ph), sa * sin(ph), za);
        }
    }
    let prev = u32(this.st0);
    let creg = this.creg;
    let k = u32(0);
    for i in 0..steps {
        k = min(u32(rand() * to_float(mcnt)), mcnt - u32(1));
        if (avoid && prev < mcnt && k == prev) {
            k = (k + u32(1)) % mcnt;
        }
        let sp = sp_mirror3(mode, k, ring_n, ring_scale, cap_scale, jit, tilt);
        let v = x - vec3(sp.x, sp.y, sp.z);
        let nn = max(1.0e-12, dot(v, v));
        x = vec3(sp.x, sp.y, sp.z) + (sp.w * sp.w / nn) * v;
        let u3 = ccen - vec3(sp.x, sp.y, sp.z);
        let dd = dot(u3, u3) - crad * crad;
        let add = max(1.0e-12, abs(dd));
        ccen = vec3(sp.x, sp.y, sp.z) + (sp.w * sp.w * (if dd > 0.0 { 1.0 } else if dd < 0.0 { -1.0 } else { 0.0 }) / add) * u3;
        crad = sp.w * sp.w * crad / add;
        creg = mix(creg, (to_float(k) + 0.5) / to_float(mcnt), color_speed);
        prev = k;
    }
    this.st0 = to_float(prev);
    depth = depth + to_float(steps);
    this.depth = depth;
    this.crad = crad;
    this.st4 = ccen.x;
    this.st5 = ccen.y;
    this.st6 = ccen.z;
    if (dc_mode == u32(1)) {
        color = fract((to_float(k) + 0.5) / to_float(mcnt) * dc_scale);
    } else if (dc_mode == u32(2)) {
        this.creg = creg;
        color = fract(creg * dc_scale);
    } else if (dc_mode == u32(3)) {
        color = fract(0.5 + 0.15 * dc_scale * ln(1.0 / max(1.0e-9, crad)));
    } else if (dc_mode == u32(4)) {
        let g = max(1.0, depth / max(1.0, to_float(steps)));
        let q = 1.0 - clamp(reseed, 0.02, 1.0);
        let t = 1.0 - pow(q, g - 1.0) * (1.0 + q) * 0.5;
        color = fract(t * dc_scale);
    } else if (dc_mode == u32(5)) {
        color = fract((atan2(x.y, x.x) * 0.15915494 + 0.5) * dc_scale);
    }
    #{ z: x * size, color: color }
}
