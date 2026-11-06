/// Mandala pattern — kaleidoscope fold (twice, with cross-fold mix)
/// followed by an Apollonian-style iteration (up to `loops` iters,
/// default 64). Per-channel "invert" flags can flip each RGB component.
///
/// 10 params: `density_pixels` + 9 algorithm controls (no `seed` or
/// `time` here — JWildfire's source omits the seed-based time
/// randomization).
/// 
/// # Authors
/// - Jesus Sosa

// GLSL Mandala, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    density_pixels: #{
        value: 1000000, min: 100, max: 10000000, free: true,
        caption: "Density Pixels",
        hint: "Discrete grid resolution.",
    },
    m_x: #{
        value: 0.025, min: -1.0, max: 1.0, free: true,
        caption: "mX",
        hint: "Z-component scalar fed into the per-fold iteration as the initial depth (`p.z = mx · v`).",
    },
    m_y: #{
        value: -0.001245675, min: -1.0, max: 1.0, free: true,
        caption: "mY",
        hint: "Apollonian-fold offset along the blue channel (`mY / π`).",
    },
    scale: #{
        value: 2.0, min: 0.0, max: 5.5, free: true,
        caption: "Scale",
        hint: "Inverse zoom — uv is scaled by `5.5 - scale`. Higher = zoom in. JWildfire's UI label is 'scale' but the effect is inverted from the name.",
    },
    sides: #{
        value: 12.0, min: 2.0, max: 64.0, free: true,
        caption: "Sides",
        hint: "Kaleidoscope sector count. The kaleidoscope angle is `π / floor(sides)`.",
    },
    multiply: #{
        value: 1.5, min: 0.0, max: 10.0, free: true,
        caption: "Multiply",
        hint: "When > 0.001, a 'multiply' post-fold step is applied (mod the kaleidoscoped uv by `floor(multiply)`).",
    },
    loops: #{
        value: 64.0, min: 1.0, max: 73.0, free: true,
        caption: "Loops",
        hint: "Apollonian iteration count. **GPU-clamped to ≤ 73** (matches JWildfire's hard limit).",
    },
    invert_r: #{
        value: 0, min: 0, max: 1, free: true,
        caption: "Invert R",
        hint: "0 = pass red channel through; 1 = invert (`1 - r`).",
    },
    invert_g: #{
        value: 0, min: 0, max: 1, free: true,
        caption: "Invert G",
        hint: "Same for green.",
    },
    invert_b: #{
        value: 1, min: 0, max: 1, free: true,
        caption: "Invert B",
        hint: "Same for blue. Default 1 — JWildfire ships this on.",
    },
    gradient: #{
        value: 0, min: 0, max: 1, free: true,
        caption: "Gradient",
        hint: "JWildfire color output mode. **Only mode 0 honored.**",
    },
};

const INFO = #{ title: "GLSL Mandala", name: "rhai_glsl_mandala", category: "plugin", phase: "any", z: "never" };

fn mandala_kscope(v, k) {
    let a = atan2(v.im, v.re);
    let two_k = 2.0 * k;
    a = a - two_k * to_float(floor(a / two_k));
    a = abs(a - k);
    let r = v.abs();
    complex(r * cos(a), r * sin(a))
}

fn variation(w, p, ctx) {
    let rgb = ctx.rgb;
    let resolution = max(100.0, to_float(p.density_pixels));
    let mx = p.m_x;
    let my = p.m_y;
    let scale = p.scale;
    let sides = max(1.0, p.sides);
    let multiply = p.multiply;
    let loops = min(u32(p.loops), u32(73));
    let inv_r = u32(to_float(p.invert_r));
    let inv_g = u32(to_float(p.invert_g));
    let inv_b = u32(to_float(p.invert_b));
    let i_f = to_float(floor(rand() * resolution));
    let j_f = to_float(floor(rand() * resolution));
    let xt = i_f + 0.5;
    let yt = j_f + 0.5;
    let rcpi = 0.318309886183791;
    let zoom = 5.5 - scale;
    let uv = complex(zoom * (2.0 * xt / resolution - 1.0), zoom * (2.0 * yt / resolution - 1.0));
    let k = 3.14159265 / to_float(floor(sides));
    let s = mandala_kscope(uv, k);
    let t = mandala_kscope(s, k);
    let v = (t.re * s.re + t.im * s.im);
    let t7 = cos(v);
    let t6 = complex(t7, t7);
    let u = complex(mix(s.re, t.re, t6.re), mix(s.im, t.im, t6.im));
    if (multiply > 0.001) {
        let m_int = to_float(floor(multiply));
        let m_safe = max(1.0e-32, m_int);
        let t1 = complex(u.im, u.re);
        let t2 = t1 - m_safe * floor(t1 / m_safe);
        let t3 = complex(-u.re, -u.im);
        let t4 = complex(u.im, u.re);
        let t9 = t3.re;
        let t10 = t3.im;
        let t8 = complex((if t9 > 0.0 { 1.0 } else if t9 < 0.0 { -1.0 } else { 0.0 }), (if t10 > 0.0 { 1.0 } else if t10 < 0.0 { -1.0 } else { 0.0 }));
        let t12 = complex(abs(t3.re), abs(t3.im));
        let t13 = complex(1.0e-32, 1.0e-32);
        let t11 = complex(max(t12.re, t13.re), max(t12.im, t13.im));
        let t3_safe = complex(t8.re * t11.re, t8.im * t11.im);
        let t14 = floor(complex(t2.re / t3_safe.re, t2.im / t3_safe.im));
        let t5_mod = t2 - complex(t3_safe.re * t14.re, t3_safe.im * t14.im);
        let t5 = t4 + t5_mod;
        u = complex(t5.im, t5.re);
    }
    let pp = vec3(u.re, u.im, mx * v);
    let l = u32(0);
    while l < loops {
        let d = max(1.0e-32, dot(pp, pp));
        let t1 = vec3(1.3, 0.999, 0.678);
        let t2 = abs(pp) / d - vec3(1.0, 1.02, my * rcpi);
        let t3 = abs(t2);
        let t4 = t1 * t3;
        pp = vec3(t4.x, t4.z, t4.y);
        l = l + u32(1);
    }
    if (inv_r == u32(1)) {
        pp.x = 1.0 - pp.x;
    }
    if (inv_g == u32(1)) {
        pp.y = 1.0 - pp.y;
    }
    if (inv_b == u32(1)) {
        pp.z = 1.0 - pp.z;
    }
    rgb = pp;
    #{ z: complex(i_f / resolution - 0.5, j_f / resolution - 0.5), rgb: rgb }
}

fn variation3(q, p, ctx) {
    let rgb = ctx.rgb;
    let resolution = max(100.0, to_float(p.density_pixels));
    let mx = p.m_x;
    let my = p.m_y;
    let scale = p.scale;
    let sides = max(1.0, p.sides);
    let multiply = p.multiply;
    let loops = min(u32(p.loops), u32(73));
    let inv_r = u32(to_float(p.invert_r));
    let inv_g = u32(to_float(p.invert_g));
    let inv_b = u32(to_float(p.invert_b));
    let i_f = to_float(floor(rand() * resolution));
    let j_f = to_float(floor(rand() * resolution));
    let zoom = 5.5 - scale;
    let uv = complex(zoom * (2.0 * (i_f + 0.5) / resolution - 1.0), zoom * (2.0 * (j_f + 0.5) / resolution - 1.0));
    let rcpi = 0.318309886183791;
    let k = 3.14159265 / to_float(floor(sides));
    let s = mandala_kscope(uv, k);
    let t = mandala_kscope(s, k);
    let v = (t.re * s.re + t.im * s.im);
    let t7 = cos(v);
    let t6 = complex(t7, t7);
    let u = complex(mix(s.re, t.re, t6.re), mix(s.im, t.im, t6.im));
    if (multiply > 0.001) {
        let m_int = to_float(floor(multiply));
        let m_safe = max(1.0e-32, m_int);
        let t1 = complex(u.im, u.re);
        let t2 = t1 - m_safe * floor(t1 / m_safe);
        let t3 = complex(-u.re, -u.im);
        let t4 = complex(u.im, u.re);
        let t9 = t3.re;
        let t10 = t3.im;
        let t8 = complex((if t9 > 0.0 { 1.0 } else if t9 < 0.0 { -1.0 } else { 0.0 }), (if t10 > 0.0 { 1.0 } else if t10 < 0.0 { -1.0 } else { 0.0 }));
        let t12 = complex(abs(t3.re), abs(t3.im));
        let t13 = complex(1.0e-32, 1.0e-32);
        let t11 = complex(max(t12.re, t13.re), max(t12.im, t13.im));
        let t3_safe = complex(t8.re * t11.re, t8.im * t11.im);
        let t14 = floor(complex(t2.re / t3_safe.re, t2.im / t3_safe.im));
        let t5_mod = t2 - complex(t3_safe.re * t14.re, t3_safe.im * t14.im);
        let t5 = t4 + t5_mod;
        u = complex(t5.im, t5.re);
    }
    let pp = vec3(u.re, u.im, mx * v);
    let l = u32(0);
    while l < loops {
        let d = max(1.0e-32, dot(pp, pp));
        let t1 = vec3(1.3, 0.999, 0.678);
        let t2 = abs(pp) / d - vec3(1.0, 1.02, my * rcpi);
        let t3 = abs(t2);
        let t4 = t1 * t3;
        pp = vec3(t4.x, t4.z, t4.y);
        l = l + u32(1);
    }
    if (inv_r == u32(1)) {
        pp.x = 1.0 - pp.x;
    }
    if (inv_g == u32(1)) {
        pp.y = 1.0 - pp.y;
    }
    if (inv_b == u32(1)) {
        pp.z = 1.0 - pp.z;
    }
    rgb = pp;
    #{ z: vec3(i_f / resolution - 0.5, j_f / resolution - 0.5, q.z), rgb: rgb }
}
