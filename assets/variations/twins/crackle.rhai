/// Voronoi-cell base shape — picks a blurred unit-disc point, locates it
/// in a noise-distorted square lattice, warps by inside-ness `pow(L,
/// power)`. JWildfire's defaults render as a tile of irregular bubbles.
///
/// 5 params: `cellsize` (square grid scale), `power` (warp curve), `distort`
/// (noise lattice perturbation), `scale` (warp amplitude), `z` (noise
/// z-slice — animate this for a flowing effect).
///
/// # Authors
/// - slobo777
/// - Andreas Maschke

// Crackle, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    cellsize: #{
        value: 1.0, min: 0.01, max: 10.0, free: true,
        caption: "Cell Size",
        hint: "Side length of the underlying square Voronoi grid. Smaller cells = finer detail but more visible aliasing. Values near 0 cause an early-return (no contribution) since 'an infinite number of invisible cells? No thanks!' (per the cpp).",
    },
    power: #{
        value: 0.2, min: -10.0, max: 10.0, free: true,
        caption: "Power",
        hint: "Exponent on the cell inside-ness `L` before re-applying. Values < 1 push the warp toward the cell boundary (bubble-like); > 1 push toward the centers (mosaic). Negative values invert.",
    },
    distort: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Distort",
        hint: "Strength of the simplex-noise perturbation applied to cell centers. 0 = perfect square grid; non-zero displaces each cell center by `distort * simplex_noise_3d(...)` in both axes.",
    },
    scale: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Scale",
        hint: "Multiplier applied after the `pow(L, power)` warp. Combines with `power` to control the overall cell-radius distribution.",
    },
    z: #{
        value: 0.0, min: -100.0, max: 100.0, free: true,
        caption: "Z",
        hint: "Z-slice through the noise field used to perturb cell centers. Animating this gives a flowing 'living' effect to the cell layout.",
    },
};

const INFO = #{ title: "Crackle", name: "rhai_crackle", category: "plugin", phase: "any" };

fn voronoi_closest(points, n, u) {
    let best = u32(0);
    let best_d2 = 1.0e30;
    let cap = min(n, u32(9));
    for i_n in 0..to_int(cap) {
        let i = u32(i_n);
        let d = points[i] - u;
        let d2 = d.norm_sqr();
        if (d2 < best_d2) {
            best_d2 = d2;
            best = i;
        }
    }
    best
}

fn voronoi_vratio(p, q, u) {
    let pmq = p - q;
    if (pmq.re == 0.0 && pmq.im == 0.0) {
        return 1.0;
    }
    let t1 = u - q;
    2.0 * (t1.re * pmq.re + t1.im * pmq.im) / pmq.norm_sqr()
}

fn voronoi_inside(points, n, q, u) {
    let ratio_max = -1.0e30;
    let cap = min(n, u32(9));
    let pq = points[q];
    for i_n in 0..to_int(cap) {
        let i = u32(i_n);
        if (i == q) {
            continue;
        }
        let r = voronoi_vratio(points[i], pq, u);
        if (r > ratio_max) {
            ratio_max = r;
        }
    }
    ratio_max
}

fn crackle_cell_hash2(x, y, zi) {
    let h = (u32(x) * u32(0x8da6b343)) ^ (u32(y) * u32(0xd8163841)) ^ (u32(zi) * u32(0xcb1ab31f));
    h = h ^ (h >> u32(13));
    h = h * u32(0x9e3779b1);
    h = h ^ (h >> u32(16));
    let h2 = h * u32(0x85ebca6b);
    let a = to_float(h & u32(0xffff)) * (2.0 / 65535.0) - 1.0;
    let b = to_float((h2 >> u32(8)) & u32(0xffff)) * (2.0 / 65535.0) - 1.0;
    complex(a, b)
}

fn crackle_fast_cell_noise(x, y, z) {
    let zf = to_float(floor(z));
    let zi = to_int(zf);
    let t = z - zf;
    let sm = t * t * (3.0 - 2.0 * t);
    mix(crackle_cell_hash2(x, y, zi), crackle_cell_hash2(x, y, zi + 1), sm)
}

fn noise_mod289_v3(x) {
    let t1 = x * (1.0 / 289.0);
    x - vec3(to_float(floor(t1.x)), to_float(floor(t1.y)), to_float(floor(t1.z))) * 289.0
}

fn noise_mod289_v4(x) {
    let t1 = x * (1.0 / 289.0);
    x - vec4(to_float(floor(t1.x)), to_float(floor(t1.y)), to_float(floor(t1.z)), to_float(floor(t1.w))) * 289.0
}

fn noise_permute_v4(x) {
    let t1 = (x * 34.0);
    noise_mod289_v4((vec4(t1.x + 10.0, t1.y + 10.0, t1.z + 10.0, t1.w + 10.0)) * x)
}

fn noise_taylor_inv_sqrt(r) {
    let t1 = 0.85373472095314 * r;
    vec4(1.79284291400159 - t1.x, 1.79284291400159 - t1.y, 1.79284291400159 - t1.z, 1.79284291400159 - t1.w)
}

fn simplex_noise_3d(v) {
    let c = complex(1.0 / 6.0, 1.0 / 3.0);
    let t2 = dot(v, vec3(c.im, c.im, c.im));
    let t1 = vec3(v.x + t2, v.y + t2, v.z + t2);
    let i = vec3(to_float(floor(t1.x)), to_float(floor(t1.y)), to_float(floor(t1.z)));
    let t3 = v - i;
    let t4 = dot(i, vec3(c.re, c.re, c.re));
    let x0 = vec3(t3.x + t4, t3.y + t4, t3.z + t4);
    let t5 = vec3(x0.y, x0.z, x0.x);
    let t6 = vec3(x0.x, x0.y, x0.z);
    let g = vec3((if t6.x < t5.x { 0.0 } else { 1.0 }), (if t6.y < t5.y { 0.0 } else { 1.0 }), (if t6.z < t5.z { 0.0 } else { 1.0 }));
    let l = vec3(1.0 - g.x, 1.0 - g.y, 1.0 - g.z);
    let i1 = min(vec3(g.x, g.y, g.z), vec3(l.z, l.x, l.y));
    let i2 = max(vec3(g.x, g.y, g.z), vec3(l.z, l.x, l.y));
    let x1 = x0 - i1 + vec3(c.re, c.re, c.re);
    let x2 = x0 - i2 + vec3(c.im, c.im, c.im);
    let x3 = vec3(x0.x - 0.5, x0.y - 0.5, x0.z - 0.5);
    let i_mod = noise_mod289_v3(i);
    let t9 = i_mod.z;
    let t10 = vec4(0.0, i1.z, i2.z, 1.0);
    let t8 = noise_permute_v4(vec4(t9 + t10.x, t9 + t10.y, t9 + t10.z, t9 + t10.w));
    let t11 = i_mod.y;
    let t7 = noise_permute_v4(vec4(t8.x + t11, t8.y + t11, t8.z + t11, t8.w + t11) + vec4(0.0, i1.y, i2.y, 1.0));
    let t12 = i_mod.x;
    let p = noise_permute_v4(vec4(t7.x + t12, t7.y + t12, t7.z + t12, t7.w + t12) + vec4(0.0, i1.x, i2.x, 1.0));
    let n_ = 0.142857142857;
    let ns = n_ * vec3(2.0, 0.5, 1.0) - vec3(0.0, 1.0, 0.0);
    let t13 = p * ns.z * ns.z;
    let j = p - 49.0 * vec4(to_float(floor(t13.x)), to_float(floor(t13.y)), to_float(floor(t13.z)), to_float(floor(t13.w)));
    let t14 = j * ns.z;
    let x_ = vec4(to_float(floor(t14.x)), to_float(floor(t14.y)), to_float(floor(t14.z)), to_float(floor(t14.w)));
    let t15 = j - 7.0 * x_;
    let y_ = vec4(to_float(floor(t15.x)), to_float(floor(t15.y)), to_float(floor(t15.z)), to_float(floor(t15.w)));
    let xc = x_ * ns.x + rgba(ns.y, ns.y, ns.y, ns.y);
    let yc = y_ * ns.x + rgba(ns.y, ns.y, ns.y, ns.y);
    let t16 = abs(xc);
    let h = vec4(1.0 - t16.x, 1.0 - t16.y, 1.0 - t16.z, 1.0 - t16.w) - abs(yc);
    let b0 = vec4(xc.x, xc.y, yc.x, yc.y);
    let b1 = vec4(xc.z, xc.w, yc.z, yc.w);
    let t17 = vec4(to_float(floor(b0.x)), to_float(floor(b0.y)), to_float(floor(b0.z)), to_float(floor(b0.w))) * 2.0;
    let s0 = vec4(t17.x + 1.0, t17.y + 1.0, t17.z + 1.0, t17.w + 1.0);
    let t18 = vec4(to_float(floor(b1.x)), to_float(floor(b1.y)), to_float(floor(b1.z)), to_float(floor(b1.w))) * 2.0;
    let s1 = vec4(t18.x + 1.0, t18.y + 1.0, t18.z + 1.0, t18.w + 1.0);
    let t19 = vec4(0.0, 0.0, 0.0, 0.0);
    let sh = -vec4((if t19.x < h.x { 0.0 } else { 1.0 }), (if t19.y < h.y { 0.0 } else { 1.0 }), (if t19.z < h.z { 0.0 } else { 1.0 }), (if t19.w < h.w { 0.0 } else { 1.0 }));
    let a0 = rgba(b0.x, b0.z, b0.y, b0.w) + rgba(s0.x, s0.z, s0.y, s0.w) * rgba(sh.x, sh.x, sh.y, sh.y);
    let a1 = rgba(b1.x, b1.z, b1.y, b1.w) + rgba(s1.x, s1.z, s1.y, s1.w) * rgba(sh.z, sh.z, sh.w, sh.w);
    let p0 = vec3(a0.x, a0.y, h.x);
    let p1 = vec3(a0.z, a0.w, h.y);
    let p2 = vec3(a1.x, a1.y, h.z);
    let p3 = vec3(a1.z, a1.w, h.w);
    let norm = noise_taylor_inv_sqrt(vec4(dot(p0, p0), dot(p1, p1), dot(p2, p2), dot(p3, p3)));
    p0 = p0 * norm.x;
    p1 = p1 * norm.y;
    p2 = p2 * norm.z;
    p3 = p3 * norm.w;
    let t20 = vec4(dot(x0, x0), dot(x1, x1), dot(x2, x2), dot(x3, x3));
    let m = max(vec4(0.6 - t20.x, 0.6 - t20.y, 0.6 - t20.z, 0.6 - t20.w), vec4(0.0, 0.0, 0.0, 0.0));
    m = m * m;
    let t21 = m * m;
    let t22 = vec4(dot(p0, x0), dot(p1, x1), dot(p2, x2), dot(p3, x3));
    42.0 * (t21.x * t22.x + t21.y * t22.y + t21.z * t22.z + t21.w * t22.w)
}

fn voronoi_crackle_position(x, y, z, s, d) {
    if (d == 0.0) {
        return complex(to_float(x) * s, to_float(y) * s);
    }
    let fx = to_float(x) * 2.5;
    let fy = to_float(y) * 2.5;
    let fz = z * 2.5;
    let nx = simplex_noise_3d(vec3(fx, fy, fz));
    let ny = simplex_noise_3d(vec3(fy + 30.2, fx - 12.1, fz + 19.8));
    complex((to_float(x) + d * nx) * s, (to_float(y) + d * ny) * s)
}

fn voronoi_cell_position(x, y, z, s, d, fast) {
    if (fast) {
        if (d == 0.0) {
            return complex(to_float(x) * s, to_float(y) * s);
        }
        let n = crackle_fast_cell_noise(x, y, z * 2.5);
        return complex((to_float(x) + d * n.re) * s, (to_float(y) + d * n.im) * s);
    }
    voronoi_crackle_position(x, y, z, s, d)
}

fn crackle_body(cellsize, power, distort, scale, z, fast) {
    if (abs(cellsize) < 1.0e-6) {
        return vec3(0.0, 0.0, 0.0);
    }
    let s = cellsize * 0.5;
    let blurr = (rand() + rand()) * 0.5 + (rand() - 0.5) * 0.25;
    let theta = 6.28318530718 * rand();
    let u = complex(blurr * sin(theta), blurr * cos(theta));
    let xcv = to_int(to_float(floor(u.re / s)));
    let ycv = to_int(to_float(floor(u.im / s)));
    let p_arr = array(9, complex(0.0, 0.0));
    let i = u32(0);
    for di in -1..=1 {
        for dj in -1..=1 {
            p_arr[i] = voronoi_cell_position(xcv + di, ycv + dj, z, s, distort, fast);
            i = i + u32(1);
        }
    }
    let q = voronoi_closest(p_arr, u32(9), u);
    let q_di = to_int(q / u32(3)) - 1;
    let q_dj = to_int(q % u32(3)) - 1;
    xcv = xcv + q_di;
    ycv = ycv + q_dj;
    if (q != u32(4)) {
        let p2 = array(9, complex(0.0, 0.0));
        i = u32(0);
        for di in -1..=1 {
            for dj in -1..=1 {
                let odi = di + q_di;
                let odj = dj + q_dj;
                if (odi >= -1 && odi <= 1 && odj >= -1 && odj <= 1) {
                    p2[i] = p_arr[u32((odi + 1) * 3 + (odj + 1))];
                } else {
                    p2[i] = voronoi_cell_position(xcv + di, ycv + dj, z, s, distort, fast);
                }
                i = i + u32(1);
            }
        }
        p_arr = p2;
    }
    let l = voronoi_inside(p_arr, u32(9), u32(4), u);
    let dxy = u - p_arr[4];
    let safe_l = l + 1.0e-32;
    let trg_l = pow(safe_l, power) * scale;
    let r = trg_l / safe_l;
    dxy = dxy * r + p_arr[4];
    vec3(dxy.re, dxy.im, l)
}

fn variation(z, p) {
    let r = crackle_body(p.cellsize, p.power, p.distort, p.scale, p.z, false);
    complex(r.x, r.y)
}
