/// Second variant of KaliSet (Mikael Hvidtfeldt Christensen, ported by
/// Jesus Sosa). Same family as `glsl_kaliset` but with:
///   - An explicit rotation angle (`shift_x` × 2π) instead of two
///     time-driven trig terms
///   - A user-controlled inversion radius (`radio`)
///   - Per-channel "color frequency" knobs (`fr`, `fg`, `fb`) feeding
///     the cos-based RGB
///   - `rsum` accumulates via `max` instead of a damped sum
///
/// 11 params (most knobs of any of the four).
/// 
/// # Authors
/// - Jesus Sosa

// GLSL KaliSet 2, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    density_pixels: #{
        value: 1000000, min: 100, max: 10000000, free: true,
        caption: "Density Pixels",
        hint: "Discrete grid resolution. See module-level doc.",
    },
    seed: #{
        value: 5000, min: 0, max: 10000, free: true,
        caption: "Seed",
        hint: "JWildfire CPU-only — set `time` directly.",
    },
    time: #{
        value: 85.5, min: -10000.0, max: 10000.0, free: true,
        caption: "Time",
        hint: "Animation parameter. Drives the per-step Y shift via `2 + 2 · sin(0.03 · time)`.",
    },
    n_iters: #{
        value: 100, min: 1, max: 100, free: true,
        caption: "N",
        hint: "Inner iteration count. **GPU-clamped to ≤ 100.**",
    },
    radio: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Radio",
        hint: "Inversion radius squared (`rad²` in the cpp). Points outside this radius get folded inward via `r² → rad²/r²`. JWildfire calls this 'radio'; we keep the name for round-trip.",
    },
    shift_x: #{
        value: -0.356, min: -5.0, max: 5.0, free: true,
        caption: "Shift X",
        hint: "Rotation angle in units of full turns (multiplied by `2π` internally). Small values produce slow rotation; 1.0 = full revolution per step. Despite the name 'Shift', this is angular, not linear.",
    },
    shift_y: #{
        value: 0.686, min: -5.0, max: 5.0, free: true,
        caption: "Shift Y",
        hint: "Zoom scale: actual per-step zoom is `2 + 5 · shift_y`. JWildfire reuses the 'Shift' label.",
    },
    fr: #{
        value: 1.8, min: -10.0, max: 10.0, free: true,
        caption: "Red Fac.",
        hint: "Red-channel frequency multiplier on the final `col` value: `red = cos(col · fr)`.",
    },
    fg: #{
        value: 1.9, min: -10.0, max: 10.0, free: true,
        caption: "Green Fac.",
        hint: "Green-channel frequency.",
    },
    fb: #{
        value: 2.2, min: -10.0, max: 10.0, free: true,
        caption: "Blue Fac.",
        hint: "Blue-channel frequency.",
    },
    gradient: #{
        value: 0, min: 0, max: 1, free: true,
        caption: "Gradient",
        hint: "JWildfire color output mode. **Only mode 0 honored.**",
    },
};

const INFO = #{ title: "GLSL KaliSet 2", name: "rhai_glsl_kaliset2", category: "plugin", phase: "any", z: "never" };

fn variation(z, p, ctx) {
    let rgb = ctx.rgb;
    let resolution = max(100.0, to_float(p.density_pixels));
    let time = p.time;
    let n_iters = min(u32(to_float(p.n_iters)), u32(100));
    let rad2 = p.radio;
    let shift_x = p.shift_x;
    let shift_y = p.shift_y;
    let fr = p.fr;
    let fg = p.fg;
    let fb = p.fb;
    let i_f = to_float(floor(rand() * resolution));
    let j_f = to_float(floor(rand() * resolution));
    let v = complex((i_f + 0.5) / resolution - 0.5, (j_f + 0.5) / resolution - 0.5) * 20.0;
    let two_pi = 6.283185307;
    let angle = two_pi * shift_x;
    let c_cos = cos(angle);
    let s_sin = sin(angle);
    let shift = complex(0.0, 2.0 + 2.0 * sin(0.03 * time));
    let zoom = 2.0 + 5.0 * shift_y;
    let rad2_safe = max(1.0e-32, abs(rad2));
    let rsum = 0.0;
    for k in 0..to_int(n_iters) {
        let rr = v.re * v.re + v.im * v.im;
        if (rr > rad2) {
            rr = rad2_safe / rr;
            v = v * rr;
        }
        rsum = max(rsum, rr);
        v = complex(c_cos * v.re - s_sin * v.im, s_sin * v.re + c_cos * v.im) * zoom + shift;
    }
    let col = rsum * rsum * (500.0 / to_float(n_iters) / rad2_safe);
    rgb = vec3(cos(col * fr), cos(col * fg), cos(col * fb));
    #{ z: complex(i_f / resolution - 0.5, j_f / resolution - 0.5), rgb: rgb }
}

fn variation3(q, p, ctx) {
    let rgb = ctx.rgb;
    let resolution = max(100.0, to_float(p.density_pixels));
    let time = p.time;
    let n_iters = min(u32(to_float(p.n_iters)), u32(100));
    let rad2 = p.radio;
    let shift_x = p.shift_x;
    let shift_y = p.shift_y;
    let fr = p.fr;
    let fg = p.fg;
    let fb = p.fb;
    let i_f = to_float(floor(rand() * resolution));
    let j_f = to_float(floor(rand() * resolution));
    let v = complex((i_f + 0.5) / resolution - 0.5, (j_f + 0.5) / resolution - 0.5) * 20.0;
    let two_pi = 6.283185307;
    let angle = two_pi * shift_x;
    let c_cos = cos(angle);
    let s_sin = sin(angle);
    let shift = complex(0.0, 2.0 + 2.0 * sin(0.03 * time));
    let zoom = 2.0 + 5.0 * shift_y;
    let rad2_safe = max(1.0e-32, abs(rad2));
    let rsum = 0.0;
    for k in 0..to_int(n_iters) {
        let rr = v.re * v.re + v.im * v.im;
        if (rr > rad2) {
            rr = rad2_safe / rr;
            v = v * rr;
        }
        rsum = max(rsum, rr);
        v = complex(c_cos * v.re - s_sin * v.im, s_sin * v.re + c_cos * v.im) * zoom + shift;
    }
    let col = rsum * rsum * (500.0 / to_float(n_iters) / rad2_safe);
    rgb = vec3(cos(col * fr), cos(col * fg), cos(col * fb));
    #{ z: vec3(i_f / resolution - 0.5, j_f / resolution - 0.5, q.z), rgb: rgb }
}
