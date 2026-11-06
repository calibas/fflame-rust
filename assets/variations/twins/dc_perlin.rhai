/// Perlin-noise base shape with direct-color output. 14 params:
///
/// - `shape` (0=square / 1=disc / 2=blur) — picks the 2D point distribution
/// - `map` (0=flat / 1=spherical / 2=hsphere / 3=qsphere / 4=bubble / 5=bubble2) —
///   warps the 2D point into a 3D noise coordinate
/// - `select_centre`, `select_range` — noise-acceptance notch band:
///   only samples whose final `e` lies in `[centre-range, centre+range]`
///   are accepted; others are retried up to `select_bailout` times
/// - `centre`, `range` — palette-position `vc = centre + range·p` (mod 1)
/// - `edge` — softness of the shape boundary (square/disc/blur all use it)
/// - `scale` — noise-input multiplier (high = fine grain)
/// - `octaves` — Perlin octaves (GPU-clamped to ≤ 8 in `perlin_noise_3d`)
/// - `amps`, `freqs` — Perlin amplitude/frequency decay per octave
/// - `z` — Z-slice through the noise field (animate for flow)
/// - `select_bailout` — retry cap (**GPU-clamped to ≤ 4**)
/// - `color_only` — when 1 with `shape=square`, use the affine input as
///   the 2D point instead of random (lets you overlay the noise color
///   on another variation's XY shape)
///
/// # Authors
/// - slobo777
/// - Andreas Maschke

// DC Perlin, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    shape: #{
        value: 0, min: 0, max: 2, free: true,
        caption: "Shape",
        hint: "0 = square (uniform [-0.5, 0.5]²), 1 = disc (uniform unit disc, triangle-distributed radius), 2 = blur (uniform radius in [0, 1+edge]). The shape is what gets plotted; the noise only drives color and acceptance filtering.",
    },
    map: #{
        value: 0, min: 0, max: 5, free: true,
        caption: "Map",
        hint: "How the 2D shape point gets mapped into the 3D noise coordinate. 0=flat (just scaled XY, Z=scale·z param). 1=spherical (XY/(r²+ε)), 2/3=hsphere/qsphere (XY/(r²+0.5/0.25)). 4=bubble, 5=bubble2 (Z replaced by sqrt(0.25-r²), gives a hemispherical noise sampling that wraps at the shape boundary).",
    },
    select_centre: #{
        value: 0.0, min: -1.0, max: 1.0,
        caption: "Select Centre",
        hint: "Centre of the noise-value notch band used to accept/retry samples. Combined with `select_range` as the band `[centre - range, centre + range]`.",
    },
    select_range: #{
        value: 1.0, min: 0.1, max: 2.0,
        caption: "Select Range",
        hint: "Half-width of the notch band. Larger = more samples accepted (less variation per call); smaller = stricter filter (more retries up to `select_bailout`).",
    },
    centre: #{
        value: 0.25, min: -10.0, max: 10.0, free: true,
        caption: "Centre",
        hint: "Palette-position offset added to `range · noise_value` before the mod-1 wrap. Shifts the color gradient.",
    },
    range: #{
        value: 0.25, min: -10.0, max: 10.0, free: true,
        caption: "Range",
        hint: "Multiplier on the Perlin noise value `p` before adding `centre`. Controls how much of the palette the noise traverses.",
    },
    edge: #{
        value: 0.0, min: 0.0, max: 1.0, free: true,
        caption: "Edge",
        hint: "Softness of the shape boundary. 0 = hard edge; > 0 softens by blending a noise-derived edge weight into the acceptance value `e`. Combines with `scale` and the noise output to produce the final notch test.",
    },
    scale: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Scale",
        hint: "Multiplier on the 3D noise coordinate. Higher = finer noise detail; lower = broader patterns.",
    },
    octaves: #{
        value: 2, min: 1, max: 8, free: true,
        caption: "Octaves",
        hint: "Number of Perlin octaves summed. **GPU-clamped to [1, 8]**. Each additional octave doubles the noise calls per sample.",
    },
    amps: #{
        value: 2.0, min: 0.1, max: 10.0, free: true,
        caption: "Amps",
        hint: "Per-octave amplitude divisor. Each octave's contribution is divided by `amps^octave_index`; values > 1 emphasize lower frequencies.",
    },
    freqs: #{
        value: 2.0, min: 0.1, max: 10.0, free: true,
        caption: "Freqs",
        hint: "Per-octave frequency multiplier. Each octave's noise input is multiplied by `freqs^octave_index`; the classical fBm uses freqs=2 (doubling).",
    },
    z: #{
        value: 0.0, min: -100.0, max: 100.0, free: true,
        caption: "Z",
        hint: "Z-slice through the noise field. Animate for a flowing/morphing pattern.",
    },
    select_bailout: #{
        value: 10, min: 1, max: 100, free: true,
        caption: "Select Bailout",
        hint: "Maximum retries when the acceptance notch rejects a sample. **GPU-clamped to ≤ 4** to keep the worst-case dispatch under the TDR budget — with octaves=8, bailout=4 is already 32 noise calls per sample.",
    },
    color_only: #{
        value: 0, min: 0, max: 1, free: true,
        caption: "Color Only",
        hint: "When 1 with `shape=square`, the 2D point is the affine input instead of random — lets you overlay this variation's color on another variation's XY. Has no effect for disc/blur shapes. (JWildfire also has a `pAmount=0` shortcut that bypasses shape selection entirely — not implementable here since variation amount isn't visible at the variation level. Use `color_only=1` + `shape=square` for the same effect.)",
    },
};

const INFO = #{ title: "DC Perlin", name: "rhai_dc_perlin", category: "plugin", phase: "any", z: "preserve_z" };

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

fn perlin_noise_3d(v, a_scale, f_scale, octaves_raw) {
    let octaves = clamp(octaves_raw, u32(1), u32(8));
    let n = 0.0;
    let u = v;
    let a = 1.0;
    let i = u32(0);
    while i < octaves {
        n = n + simplex_noise_3d(u) / a;
        a = a * a_scale;
        u = u * f_scale;
        i = i + u32(1);
    }
    n
}

fn variation(w, p, ctx) {
    let color = ctx.color;
    let shape = u32(to_float(p.shape));
    let map = u32(to_float(p.map));
    let select_centre = p.select_centre;
    let select_range = p.select_range;
    let centre = p.centre;
    let range = p.range;
    let edge_p = p.edge;
    let scale = p.scale;
    let octaves = u32(to_float(p.octaves));
    let amps = p.amps;
    let freqs = p.freqs;
    let z = p.z;
    let select_bailout_raw = u32(to_float(p.select_bailout));
    let color_only = u32(to_float(p.color_only));
    let bailout = clamp(select_bailout_raw, u32(1), u32(4));
    let nb = select_centre - select_range;
    if (nb > 0.75) {
        nb = 0.75;
    }
    if (nb < -2.0) {
        nb = -3.0;
    }
    let nt = select_centre + select_range;
    if (nt < -0.75) {
        nt = -0.75;
    }
    if (nt > 3.0) {
        nt = 3.0;
    }
    let vx = 0.0;
    let vy = 0.0;
    let e = 0.0;
    let noise_p = 0.0;
    let t = u32(0);
    let edge_safe = max(1.0e-32, edge_p);
    while true {
        e = 0.0;
        let t1 = (shape);
        if t1 == u32(0) {
            let bx = (if color_only > u32(0) { w.re } else { rand() });
            let by = (if color_only > u32(0) { w.im } else { rand() });
            vx = (1.0 + edge_p) * (bx - 0.5);
            vy = (1.0 + edge_p) * (by - 0.5);
            let r = (if vx * vx > vy * vy { vx } else { vy });
            if (r > 1.0 - edge_p) {
                e = 0.5 * (r - 1.0 + edge_p) / edge_safe;
            }
        } else if t1 == u32(1) {
            let r = rand() + rand();
            if (r > 1.0) {
                r = 2.0 - r;
            }
            r = r * (1.0 + edge_p);
            if (r > 1.0 - edge_p) {
                e = 0.5 * (r - 1.0 + edge_p) / edge_safe;
            }
            let theta = rand() * 6.28318530718;
            vx = 0.5 * r * sin(theta);
            vy = 0.5 * r * cos(theta);
        } else if t1 == u32(2) {
            let r = (1.0 + edge_p) * rand();
            if (r > 1.0 - edge_p) {
                e = 0.5 * (r - 1.0 + edge_p) / edge_safe;
            }
            let theta = rand() * 6.28318530718;
            vx = 0.5 * r * sin(theta);
            vy = 0.5 * r * cos(theta);
        } else {
        }
        let nv = vec3(0.0, 0.0, 0.0);
        let t2 = (map);
        if t2 == u32(0) {
            nv = vec3(scale * vx, scale * vy, scale * z);
        } else if t2 == u32(1) {
            let r = 1.0 / (vx * vx + vy * vy + 1.0e-32);
            nv = vec3(scale * vx * r, scale * vy * r, scale * z);
        } else if t2 == u32(2) {
            let r = 1.0 / (vx * vx + vy * vy + 0.5);
            nv = vec3(scale * vx * r, scale * vy * r, scale * z);
        } else if t2 == u32(3) {
            let r = 1.0 / (vx * vx + vy * vy + 0.25);
            nv = vec3(scale * vx * r, scale * vy * r, scale * z);
        } else if t2 == u32(4) {
            let r2 = 0.25 - (vx * vx + vy * vy);
            let r = sqrt(abs(r2));
            nv = vec3(scale * vx, scale * vy, scale * (r + z));
        } else if t2 == u32(5) {
            let r2 = 0.25 - (vx * vx + vy * vy);
            let r = sqrt(abs(r2));
            nv = vec3(scale * vx, scale * vy, scale * (2.0 * r + z));
        } else {
            nv = vec3(0.0, 0.0, 0.0);
        }
        noise_p = perlin_noise_3d(nv, amps, freqs, octaves);
        if (noise_p > 0.0) {
            e = noise_p * (1.0 + e * e * 20.0) + 2.0 * e;
        } else {
            e = noise_p * (1.0 + e * e * 20.0) - 2.0 * e;
        }
        let accepted = (e >= nb) && (e <= nt);
        if (accepted || t >= bailout - u32(1)) {
            break;
        }
        t = t + u32(1);
    }
    let col = centre + range * noise_p;
    col = col - to_float(floor(col));
    color = col;
    #{ z: complex(vx, vy), color: color }
}

fn variation3(v, p, ctx) {
    let color = ctx.color;
    let shape = u32(to_float(p.shape));
    let map = u32(to_float(p.map));
    let select_centre = p.select_centre;
    let select_range = p.select_range;
    let centre = p.centre;
    let range = p.range;
    let edge_p = p.edge;
    let scale = p.scale;
    let octaves = u32(to_float(p.octaves));
    let amps = p.amps;
    let freqs = p.freqs;
    let z = p.z;
    let select_bailout_raw = u32(to_float(p.select_bailout));
    let color_only = u32(to_float(p.color_only));
    let bailout = clamp(select_bailout_raw, u32(1), u32(4));
    let nb = select_centre - select_range;
    if (nb > 0.75) {
        nb = 0.75;
    }
    if (nb < -2.0) {
        nb = -3.0;
    }
    let nt = select_centre + select_range;
    if (nt < -0.75) {
        nt = -0.75;
    }
    if (nt > 3.0) {
        nt = 3.0;
    }
    let vx = 0.0;
    let vy = 0.0;
    let e = 0.0;
    let noise_p = 0.0;
    let t = u32(0);
    let edge_safe = max(1.0e-32, edge_p);
    while true {
        e = 0.0;
        let t1 = (shape);
        if t1 == u32(0) {
            let bx = (if color_only > u32(0) { v.x } else { rand() });
            let by = (if color_only > u32(0) { v.y } else { rand() });
            vx = (1.0 + edge_p) * (bx - 0.5);
            vy = (1.0 + edge_p) * (by - 0.5);
            let r = (if vx * vx > vy * vy { vx } else { vy });
            if (r > 1.0 - edge_p) {
                e = 0.5 * (r - 1.0 + edge_p) / edge_safe;
            }
        } else if t1 == u32(1) {
            let r = rand() + rand();
            if (r > 1.0) {
                r = 2.0 - r;
            }
            r = r * (1.0 + edge_p);
            if (r > 1.0 - edge_p) {
                e = 0.5 * (r - 1.0 + edge_p) / edge_safe;
            }
            let theta = rand() * 6.28318530718;
            vx = 0.5 * r * sin(theta);
            vy = 0.5 * r * cos(theta);
        } else if t1 == u32(2) {
            let r = (1.0 + edge_p) * rand();
            if (r > 1.0 - edge_p) {
                e = 0.5 * (r - 1.0 + edge_p) / edge_safe;
            }
            let theta = rand() * 6.28318530718;
            vx = 0.5 * r * sin(theta);
            vy = 0.5 * r * cos(theta);
        } else {
        }
        let nv = vec3(0.0, 0.0, 0.0);
        let t2 = (map);
        if t2 == u32(0) {
            nv = vec3(scale * vx, scale * vy, scale * z);
        } else if t2 == u32(1) {
            let r = 1.0 / (vx * vx + vy * vy + 1.0e-32);
            nv = vec3(scale * vx * r, scale * vy * r, scale * z);
        } else if t2 == u32(2) {
            let r = 1.0 / (vx * vx + vy * vy + 0.5);
            nv = vec3(scale * vx * r, scale * vy * r, scale * z);
        } else if t2 == u32(3) {
            let r = 1.0 / (vx * vx + vy * vy + 0.25);
            nv = vec3(scale * vx * r, scale * vy * r, scale * z);
        } else if t2 == u32(4) {
            let r2 = 0.25 - (vx * vx + vy * vy);
            let r = sqrt(abs(r2));
            nv = vec3(scale * vx, scale * vy, scale * (r + z));
        } else if t2 == u32(5) {
            let r2 = 0.25 - (vx * vx + vy * vy);
            let r = sqrt(abs(r2));
            nv = vec3(scale * vx, scale * vy, scale * (2.0 * r + z));
        } else {
            nv = vec3(0.0, 0.0, 0.0);
        }
        noise_p = perlin_noise_3d(nv, amps, freqs, octaves);
        if (noise_p > 0.0) {
            e = noise_p * (1.0 + e * e * 20.0) + 2.0 * e;
        } else {
            e = noise_p * (1.0 + e * e * 20.0) - 2.0 * e;
        }
        let accepted = (e >= nb) && (e <= nt);
        if (accepted || t >= bailout - u32(1)) {
            break;
        }
        t = t + u32(1);
    }
    let col = centre + range * noise_p;
    col = col - to_float(floor(col));
    color = col;
    #{ z: vec3(vx, vy, v.z), color: color }
}
