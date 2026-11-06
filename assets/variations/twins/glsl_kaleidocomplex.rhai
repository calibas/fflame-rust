/// Complex-valued kaleidoscope iteration. Iterates a `vec4` state
/// (treating `(x, y)` and `(z, w)` as two paired complex numbers)
/// through abs/shift/complex-multiply/cross-product steps,
/// accumulating per-channel color via `exp(-|...|)` terms. Has a
/// per-channel "color frequency" stage at the end (`cos(col · F*)`).
///
/// 9 params: standard 4 + `i_max` (iter count), `color` (intensity),
/// `fr`/`fg`/`fb` (per-channel frequencies).
/// 
/// # Authors
/// - Jesus Sosa

// GLSL KaleidoComplex, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    density_pixels: #{
        value: 1000000, min: 100, max: 10000000, free: true,
        caption: "Density Pixels",
        hint: "Discrete grid resolution.",
    },
    seed: #{
        value: 10000, min: 0, max: 10000, free: true,
        caption: "Seed",
        hint: "JWildfire CPU-only — set `time` directly.",
    },
    time: #{
        value: 0.0, min: -10000.0, max: 10000.0, free: true,
        caption: "Time",
        hint: "Animation parameter. Embedded into the complex multiply as `cmult(z, vec2(sin(time), cos(time)))`.",
    },
    i_max: #{
        value: 12, min: 1, max: 50, free: true,
        caption: "iMax",
        hint: "Inner iteration cap. **GPU-clamped to ≤ 50.** Inner loop also breaks early when |z|² > 1e7 or the (z, w) pair magnitudes diverge.",
    },
    color: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "Color",
        hint: "Color intensity scalar. Multiplies the per-iteration `exp(-|...|)` accumulators (the variable name is `fc` in the JWildfire source — kept the JWF UI label 'Color').",
    },
    fr: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Red Fac.",
        hint: "Red-channel frequency: final red = `cos(col.r · fr)`.",
    },
    fg: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Green Fac.",
        hint: "Green-channel frequency.",
    },
    fb: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Blue Fac.",
        hint: "Blue-channel frequency.",
    },
    gradient: #{
        value: 0, min: 0, max: 1, free: true,
        caption: "Gradient",
        hint: "JWildfire color output mode. **Only mode 0 honored.**",
    },
};

const INFO = #{ title: "GLSL KaleidoComplex", name: "rhai_glsl_kaleidocomplex", category: "plugin", phase: "any", z: "never" };

fn kaleidocomplex_cmult(a, b) {
    complex(a.re * b.re - a.im * b.im, a.re * b.im + a.im * b.re)
}

fn variation(q, p, ctx) {
    let rgb = ctx.rgb;
    let resolution = max(100.0, to_float(p.density_pixels));
    let time = p.time;
    let i_max = min(to_int(to_float(p.i_max)), 50);
    let fc = p.color;
    let fr = p.fr;
    let fg = p.fg;
    let fb = p.fb;
    let i_f = to_float(floor(rand() * resolution));
    let j_f = to_float(floor(rand() * resolution));
    let uv = complex((i_f + 0.5) / resolution - 0.5, (j_f + 0.5) / resolution - 0.5);
    let z = vec4(abs(uv.re) / 0.125, abs(uv.im) / 0.125, 0.0, 0.0);
    let col = vec3(0.0, 0.0, 0.0);
    let dist = complex(0.0, 0.0);
    let ii = -1;
    let r = 1.0;
    let st = complex(sin(time), cos(time));
    let one_one = complex(1.0, 1.0);
    for i in -1..i_max {
        r = r * -1.0;
        ii = ii + 1;
        let t1 = kaleidocomplex_cmult(complex(z.x, z.y), one_one);
        z.x = t1.re;
        z.y = t1.im;
        let t3 = complex(z.x, z.y);
        let t2 = complex(abs(t3.re), abs(t3.im));
        let t4 = (r - 10.5);
        t1 = complex(t2.re + t4, t2.im + t4);
        z.x = t1.re;
        z.y = t1.im;
        t1 = kaleidocomplex_cmult(complex(z.x, z.y), st);
        z.x = t1.re;
        z.y = t1.im;
        let zz_new = fc * (z.x * z.z - z.y * z.w);
        let zw_new = fc * (z.y * z.z - z.x * z.w);
        z.z = zz_new;
        z.w = zw_new;
        dist = complex(z.x * z.x + z.y * z.y, z.z * z.z + z.w * z.w);
        if (ii > 0 && (abs(z.x) < 0.51 || abs(z.y) < 0.51)) {
            col.x = exp(-abs(z.x * z.x * z.y));
            col.y = exp(-abs(z.y * z.x * z.y));
            col.z = exp(-abs(min(z.x, z.y)));
            break;
        }
        let ii_f = to_float(ii);
        let t5 = z.x;
        let safe_x = (if t5 > 0.0 { 1.0 } else if t5 < 0.0 { -1.0 } else { 0.0 }) * max(1.0e-32, abs(z.x));
        let t6 = z.y;
        let safe_y = (if t6 > 0.0 { 1.0 } else if t6 < 0.0 { -1.0 } else { 0.0 }) * max(1.0e-32, abs(z.y));
        let xy_sum = z.x + z.y;
        let safe_xy = (if xy_sum > 0.0 { 1.0 } else if xy_sum < 0.0 { -1.0 } else { 0.0 }) * max(1.0e-32, abs(xy_sum));
        col.x = col.x + fc * exp(-abs(-z.x / max(1.0e-32, ii_f) + ii_f / safe_x));
        col.y = col.y + fc * exp(-abs(-z.y / max(1.0e-32, ii_f) + ii_f / safe_y));
        col.z = col.z + fc * exp(-abs(-xy_sum / max(1.0e-32, ii_f) + ii_f / safe_xy));
        if (dist.re > 1.0e7 || dist.im > 1.0e11) {
            break;
        }
    }
    rgb = vec3(cos(col.x * fr), cos(col.y * fg), cos(col.z * fb));
    #{ z: complex(i_f / resolution - 0.5, j_f / resolution - 0.5), rgb: rgb }
}

fn variation3(v, p, ctx) {
    let rgb = ctx.rgb;
    let resolution = max(100.0, to_float(p.density_pixels));
    let time = p.time;
    let i_max = min(to_int(to_float(p.i_max)), 50);
    let fc = p.color;
    let fr = p.fr;
    let fg = p.fg;
    let fb = p.fb;
    let i_f = to_float(floor(rand() * resolution));
    let j_f = to_float(floor(rand() * resolution));
    let uv = complex((i_f + 0.5) / resolution - 0.5, (j_f + 0.5) / resolution - 0.5);
    let z = vec4(abs(uv.re) / 0.125, abs(uv.im) / 0.125, 0.0, 0.0);
    let col = vec3(0.0, 0.0, 0.0);
    let dist = complex(0.0, 0.0);
    let ii = -1;
    let r = 1.0;
    let st = complex(sin(time), cos(time));
    let one_one = complex(1.0, 1.0);
    for i in -1..i_max {
        r = r * -1.0;
        ii = ii + 1;
        let t1 = kaleidocomplex_cmult(complex(z.x, z.y), one_one);
        z.x = t1.re;
        z.y = t1.im;
        let t3 = complex(z.x, z.y);
        let t2 = complex(abs(t3.re), abs(t3.im));
        let t4 = (r - 10.5);
        t1 = complex(t2.re + t4, t2.im + t4);
        z.x = t1.re;
        z.y = t1.im;
        t1 = kaleidocomplex_cmult(complex(z.x, z.y), st);
        z.x = t1.re;
        z.y = t1.im;
        let zz_new = fc * (z.x * z.z - z.y * z.w);
        let zw_new = fc * (z.y * z.z - z.x * z.w);
        z.z = zz_new;
        z.w = zw_new;
        dist = complex(z.x * z.x + z.y * z.y, z.z * z.z + z.w * z.w);
        if (ii > 0 && (abs(z.x) < 0.51 || abs(z.y) < 0.51)) {
            col.x = exp(-abs(z.x * z.x * z.y));
            col.y = exp(-abs(z.y * z.x * z.y));
            col.z = exp(-abs(min(z.x, z.y)));
            break;
        }
        let ii_f = to_float(ii);
        let t5 = z.x;
        let safe_x = (if t5 > 0.0 { 1.0 } else if t5 < 0.0 { -1.0 } else { 0.0 }) * max(1.0e-32, abs(z.x));
        let t6 = z.y;
        let safe_y = (if t6 > 0.0 { 1.0 } else if t6 < 0.0 { -1.0 } else { 0.0 }) * max(1.0e-32, abs(z.y));
        let xy_sum = z.x + z.y;
        let safe_xy = (if xy_sum > 0.0 { 1.0 } else if xy_sum < 0.0 { -1.0 } else { 0.0 }) * max(1.0e-32, abs(xy_sum));
        col.x = col.x + fc * exp(-abs(-z.x / max(1.0e-32, ii_f) + ii_f / safe_x));
        col.y = col.y + fc * exp(-abs(-z.y / max(1.0e-32, ii_f) + ii_f / safe_y));
        col.z = col.z + fc * exp(-abs(-xy_sum / max(1.0e-32, ii_f) + ii_f / safe_xy));
        if (dist.re > 1.0e7 || dist.im > 1.0e11) {
            break;
        }
    }
    rgb = vec3(cos(col.x * fr), cos(col.y * fg), cos(col.z * fb));
    #{ z: vec3(i_f / resolution - 0.5, j_f / resolution - 0.5, v.z), rgb: rgb }
}
