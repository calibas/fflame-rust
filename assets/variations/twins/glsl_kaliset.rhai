/// "KaliSet" iterative fractal (Mikael Hvidtfeldt Christensen / Kali).
/// Maps the sample point into `[-10, 10]²`, then iterates a sphere
/// inversion followed by a time-rotated scale-and-shift, accumulating
/// the running squared-radius into `rsum`. RGB output is
/// `(cos(col), cos(2col), cos(4col))` where `col = rsum/2`.
///
/// 7 params: standard 4 + `N` (iter count), `shift_x`/`shift_y`
/// (per-step offset).
/// 
/// # Authors
/// - Jesus Sosa

// GLSL KaliSet, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    density_pixels: #{
        value: 1000000, min: 100, max: 10000000, free: true,
        caption: "Density Pixels",
        hint: "Discrete grid resolution for sampling. See module-level doc for shared semantics.",
    },
    seed: #{
        value: 10000, min: 0, max: 10000, free: true,
        caption: "Seed",
        hint: "JWildfire CPU-only — accepted for round-trip; set `time` directly.",
    },
    time: #{
        value: 200.0, min: 1.0, max: 1000.0, free: true,
        caption: "Time",
        hint: "Animation parameter. Drives the rotation angle (`time/603 · 2π` for cos, `time/407.4 · 2π` for sin) and the zoom factor (`time/184 + 1`). JWildfire clamps to `[1, 1000]`; we follow suit.",
    },
    n_iters: #{
        value: 60, min: 1, max: 100, free: true,
        caption: "N",
        hint: "Inner iteration count. **GPU-clamped to ≤ 100.** Higher = more detail but linearly more cost. JWildfire's default is 60.",
    },
    shift_x: #{
        value: -0.22, min: -5.0, max: 5.0, free: true,
        caption: "Shift X",
        hint: "Per-iteration X offset added after rotation and zoom. Small changes here dramatically alter the fractal's structure.",
    },
    shift_y: #{
        value: -0.21, min: -5.0, max: 5.0, free: true,
        caption: "Shift Y",
        hint: "Per-iteration Y offset. Same sensitivity as shift_x.",
    },
    gradient: #{
        value: 0, min: 0, max: 1, free: true,
        caption: "Gradient",
        hint: "JWildfire color output mode. **Only mode 0 (direct RGB) honored.** Value accepted for round-trip.",
    },
};

const INFO = #{ title: "GLSL KaliSet", name: "rhai_glsl_kaliset", category: "plugin", phase: "any", z: "never" };

fn variation(z, p, ctx) {
    let rgb = ctx.rgb;
    let resolution = max(100.0, to_float(p.density_pixels));
    let time = clamp(p.time, 1.0, 1000.0);
    let n_iters = min(u32(to_float(p.n_iters)), u32(100));
    let shift_x = p.shift_x;
    let shift_y = p.shift_y;
    let i_f = to_float(floor(rand() * resolution));
    let j_f = to_float(floor(rand() * resolution));
    let x = i_f + 0.5;
    let y = j_f + 0.5;
    let v = complex(x / resolution - 0.5, y / resolution - 0.5) * 20.0;
    let pi2 = 7.2;
    let c_cos = cos(time / 603.0 * pi2);
    let s_sin = sin(time / 407.4 * pi2);
    let shift = complex(shift_x, shift_y);
    let zoom = time / 184.0 + 1.0;
    let rsum = 0.0;
    let k = u32(0);
    while k < n_iters {
        let rr = v.re * v.re + v.im * v.im;
        if (rr > 1.0) {
            rr = 1.0 / rr;
            v = v * rr;
        }
        rsum = rsum * 0.99 + rr;
        v = complex(c_cos * v.re - s_sin * v.im, s_sin * v.re + c_cos * v.im) * zoom + shift;
        k = k + u32(1);
    }
    let col = rsum * 0.5;
    rgb = vec3(cos(col * 1.0), cos(col * 2.0), cos(col * 4.0));
    #{ z: complex(i_f / resolution - 0.5, j_f / resolution - 0.5), rgb: rgb }
}

fn variation3(q, p, ctx) {
    let rgb = ctx.rgb;
    let resolution = max(100.0, to_float(p.density_pixels));
    let time = clamp(p.time, 1.0, 1000.0);
    let n_iters = min(u32(to_float(p.n_iters)), u32(100));
    let shift_x = p.shift_x;
    let shift_y = p.shift_y;
    let i_f = to_float(floor(rand() * resolution));
    let j_f = to_float(floor(rand() * resolution));
    let v = complex((i_f + 0.5) / resolution - 0.5, (j_f + 0.5) / resolution - 0.5) * 20.0;
    let pi2 = 7.2;
    let c_cos = cos(time / 603.0 * pi2);
    let s_sin = sin(time / 407.4 * pi2);
    let shift = complex(shift_x, shift_y);
    let zoom = time / 184.0 + 1.0;
    let rsum = 0.0;
    let k = u32(0);
    while k < n_iters {
        let rr = v.re * v.re + v.im * v.im;
        if (rr > 1.0) {
            rr = 1.0 / rr;
            v = v * rr;
        }
        rsum = rsum * 0.99 + rr;
        v = complex(c_cos * v.re - s_sin * v.im, s_sin * v.re + c_cos * v.im) * zoom + shift;
        k = k + u32(1);
    }
    let col = rsum * 0.5;
    rgb = vec3(cos(col * 1.0), cos(col * 2.0), cos(col * 4.0));
    #{ z: vec3(i_f / resolution - 0.5, j_f / resolution - 0.5, q.z), rgb: rgb }
}
