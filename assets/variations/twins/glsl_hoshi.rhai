/// Star/hoshi tiling — iterative fold + rotate (steps iterations,
/// default 28) with HSV color derived from the final point's angle.
/// Time-driven per-iteration rotation makes the pattern animate.
///
/// 6 params: `density_pixels`, `seed` (CPU-only), `time`, `steps`,
/// `scale`, `translate`.
/// 
/// # Authors
/// - Jesus Sosa

// GLSL Hoshi, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    density_pixels: #{
        value: 1000000, min: 100, max: 10000000, free: true,
        caption: "Density Pixels",
        hint: "Discrete grid resolution.",
    },
    seed: #{
        value: 100000, min: 0, max: 1000000, free: true,
        caption: "Seed",
        hint: "JWildfire CPU-only — set `time` directly.",
    },
    time: #{
        value: 10.0, min: 0.0001, max: 10000.0, free: true, exponential: false,
        caption: "Time",
        hint: "Animation parameter. Drives per-iteration rotation angles. Avoid 0 — `10/time` divides by it. Clamped to ≥ 1e-4 at runtime.",
    },
    steps: #{
        value: 28, min: 1, max: 50, free: true,
        caption: "Steps",
        hint: "Iteration count. **GPU-clamped to ≤ 50.** Default 28.",
    },
    scale: #{
        value: 1.25, min: 0.0, max: 5.0, free: true,
        caption: "Scale",
        hint: "Per-iteration scale multiplier applied after the fold.",
    },
    translate: #{
        value: 1.5, min: -5.0, max: 5.0, free: true,
        caption: "Translate",
        hint: "Per-iteration translation along both x and y (applied as `p -= vec2(translate, translate)` after scaling).",
    },
    gradient: #{
        value: 0, min: 0, max: 1, free: true,
        caption: "Gradient",
        hint: "JWildfire color output mode. **Only mode 0 honored.**",
    },
};

const INFO = #{ title: "GLSL Hoshi", name: "rhai_glsl_hoshi", category: "plugin", phase: "any", z: "never" };

fn hoshi_hsv(h_in, s, v) {
    let t1 = vec3(3.0, 2.0, 1.0);
    t1 = (vec3(t1.x + h_in, t1.y + h_in, t1.z + h_in)) / 3.0;
    let t3 = (t1 - vec3(to_float(floor(t1.x)), to_float(floor(t1.y)), to_float(floor(t1.z)))) * 6.0;
    let t2 = vec3(t3.x - 3.0, t3.y - 3.0, t3.z - 3.0);
    let t4 = abs(t2);
    t2 = vec3(t4.x - 1.0, t4.y - 1.0, t4.z - 1.0);
    let t5 = vec3(0.0, 0.0, 0.0);
    let t6 = vec3(1.0, 1.0, 1.0);
    t2 = vec3(clamp(t2.x, t5.x, t6.x), clamp(t2.y, t5.y, t6.y), clamp(t2.z, t5.z, t6.z));
    let t7 = vec3(3.1, 3.1, 3.1);
    let t8 = vec3(s, s, s);
    vec3(mix(t7.x, t2.x, t8.x), mix(t7.y, t2.y, t8.y), mix(t7.z, t2.z, t8.z)) * v
}

fn hoshi_rotate(p, a) {
    let c = cos(a);
    let s = sin(a);
    complex(p.re * c - p.im * s, p.re * s + p.im * c)
}

fn variation(z, p, ctx) {
    let rgb = ctx.rgb;
    let resolution = max(100.0, to_float(p.density_pixels));
    let time = max(1.0e-4, p.time);
    let steps = min(u32(to_float(p.steps)), u32(50));
    let scale = p.scale;
    let translate = p.translate;
    let i_f = to_float(floor(rand() * resolution));
    let j_f = to_float(floor(rand() * resolution));
    let pp = complex(2.0 * i_f / resolution - 1.0, 2.0 * j_f / resolution - 1.0);
    pp = pp * 0.182;
    let x_init = pp.im;
    let t2 = (pp - 4.0 * floor(pp / 4.0));
    let t1 = complex(t2.re - 2.0, t2.im - 2.0);
    pp = complex(abs(t1.re), abs(t1.im));
    let t3 = -0.5;
    let fold = complex(t3, t3);
    let trans = complex(translate, translate);
    for k in 0..to_int(steps) {
        let orig_i = to_float(steps) - to_float(k);
        let t4 = pp - fold;
        pp = complex(abs(t4.re), abs(t4.im)) + fold;
        pp = pp * scale - trans;
        let denom = 0.10 + sin(time * 0.0005 + orig_i * 0.5000001) * 0.4999 + 0.5 + (10.0 / time) + sin(time) / 100.0;
        pp = hoshi_rotate(pp, 3.14159 / max(1.0e-32, denom));
    }
    let ii = x_init * x_init + atan2(pp.im, pp.re) + time * 0.02;
    let h = to_float(floor(ii * 4.0)) / 8.0 + 1.107;
    let m = ii * 2.0 / 5.0;
    let m_mod = m - 0.25 * to_float(floor(m / 0.25));
    let t5 = -0.1;
    let t6 = m_mod * 900.0;
    let t7 = clamp((t6 - t5) / (0.8 - t5), 0.0, 1.0);
    h = h + (t7 * t7 * (3.0 - 2.0 * t7)) / 0.010 - 0.5;
    let t8 = -3.0;
    let t9 = pp.abs();
    let t10 = clamp((t9 - t8) / (3.0 - t8), 0.0, 1.0);
    let v = (t10 * t10 * (3.0 - 2.0 * t10));
    rgb = hoshi_hsv(h, 1.0, v);
    #{ z: complex(i_f / resolution - 0.5, j_f / resolution - 0.5), rgb: rgb }
}

fn variation3(q, p, ctx) {
    let rgb = ctx.rgb;
    let resolution = max(100.0, to_float(p.density_pixels));
    let time = max(1.0e-4, p.time);
    let steps = min(u32(to_float(p.steps)), u32(50));
    let scale = p.scale;
    let translate = p.translate;
    let i_f = to_float(floor(rand() * resolution));
    let j_f = to_float(floor(rand() * resolution));
    let pp = complex(2.0 * i_f / resolution - 1.0, 2.0 * j_f / resolution - 1.0) * 0.182;
    let x_init = pp.im;
    let t2 = (pp - 4.0 * floor(pp / 4.0));
    let t1 = complex(t2.re - 2.0, t2.im - 2.0);
    pp = complex(abs(t1.re), abs(t1.im));
    let t3 = -0.5;
    let fold = complex(t3, t3);
    let trans = complex(translate, translate);
    for k in 0..to_int(steps) {
        let orig_i = to_float(steps) - to_float(k);
        let t4 = pp - fold;
        pp = complex(abs(t4.re), abs(t4.im)) + fold;
        pp = pp * scale - trans;
        let denom = 0.10 + sin(time * 0.0005 + orig_i * 0.5000001) * 0.4999 + 0.5 + (10.0 / time) + sin(time) / 100.0;
        pp = hoshi_rotate(pp, 3.14159 / max(1.0e-32, denom));
    }
    let ii = x_init * x_init + atan2(pp.im, pp.re) + time * 0.02;
    let h = to_float(floor(ii * 4.0)) / 8.0 + 1.107;
    let m = ii * 2.0 / 5.0;
    let m_mod = m - 0.25 * to_float(floor(m / 0.25));
    let t5 = -0.1;
    let t6 = m_mod * 900.0;
    let t7 = clamp((t6 - t5) / (0.8 - t5), 0.0, 1.0);
    h = h + (t7 * t7 * (3.0 - 2.0 * t7)) / 0.010 - 0.5;
    let t8 = -3.0;
    let t9 = pp.abs();
    let t10 = clamp((t9 - t8) / (3.0 - t8), 0.0, 1.0);
    let v = (t10 * t10 * (3.0 - 2.0 * t10));
    rgb = hoshi_hsv(h, 1.0, v);
    #{ z: vec3(i_f / resolution - 0.5, j_f / resolution - 0.5, q.z), rgb: rgb }
}
