/// Sierpinski-carpet-style square tiling. Tests whether the sample
/// point falls in a "carpet" (an N-deep Sierpinski subdivision of the
/// unit square minus the center cell), running the test 4 times with
/// small offsets to give a multi-layer effect. Time-driven color
/// inversion via the `cos`/`tan`/`sin` of `time`.
///
/// 9 params: standard 4 + `n` (recursion depth), `dir` (zoom-in vs
/// zoom-out), `fr`/`fg`/`fb` (color frequencies).
/// 
/// # Authors
/// - Jesus Sosa

// GLSL Squares, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    density_pixels: #{
        value: 1000000, min: 100, max: 10000000, free: true,
        caption: "Density Pixels",
        hint: "Discrete grid resolution.",
    },
    seed: #{
        value: 5000, min: 0, max: 10000, free: true,
        caption: "Seed",
        hint: "JWildfire CPU-only — set `time` directly.",
    },
    time: #{
        value: 85.5, min: -10000.0, max: 10000.0, free: true,
        caption: "Time",
        hint: "Animation parameter. Drives both the per-iteration zoom (`pow(0.5, mod(direction · time, 1))`) and the initial color (`(cos(time), tan(time), sin(time))`).",
    },
    n: #{
        value: 3, min: 1, max: 8, free: true,
        caption: "N",
        hint: "Carpet recursion depth. **GPU-clamped to ≤ 8** — each level costs `mod` + branch. Default 3.",
    },
    direction: #{
        value: 1, min: 0, max: 1, free: true,
        caption: "Direction",
        hint: "Zoom direction. 1 = zoom in over time (`+0.1·time`); 0 = zoom out (`-1·time`).",
    },
    fr: #{
        value: 1.8, min: -10.0, max: 10.0, free: true,
        caption: "Red Fac.",
        hint: "Red-channel final-cosine frequency.",
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

const INFO = #{ title: "GLSL Squares", name: "rhai_glsl_squares", category: "plugin", phase: "any", z: "never" };

fn squares_hit(p_in, n, time, dir) {
    let direction = 0.0;
    if (dir == u32(1)) {
        direction = 0.1;
    } else {
        direction = -1.0;
    }
    let arg = direction * time;
    let arg_mod = arg - to_float(floor(arg));
    let scale = pow(0.5, arg_mod);
    let coord_iter = p_in / scale;
    for i_n in 0..to_int(n) {
        let i = u32(i_n);
        let sectors = floor(coord_iter * 3.0);
        if (sectors.re == 1.0 && sectors.im == 1.0) {
            return false;
        }
        coord_iter = coord_iter * 3.0 - sectors;
    }
    true
}

fn variation(w, p, ctx) {
    let rgb = ctx.rgb;
    let resolution = max(100.0, to_float(p.density_pixels));
    let time = p.time;
    let n = min(u32(to_float(p.n)), u32(8));
    let dir = u32(to_float(p.direction));
    let fr = p.fr;
    let fg = p.fg;
    let fb = p.fb;
    let i_f = to_float(floor(rand() * resolution));
    let j_f = to_float(floor(rand() * resolution));
    let x = i_f + 0.5;
    let y = j_f + 0.5;
    let coord_orig = complex(abs(x / resolution - 0.5), abs(y / resolution - 0.5));
    coord_orig = coord_orig - floor(coord_orig);
    let color = vec3(cos(time), tan(time), sin(time));
    for i_n in 0..4 {
        let i = u32(i_n);
        let t1 = to_float(i) * 0.1;
        let offset = complex(t1, t1);
        if (squares_hit(offset + coord_orig, n, time, dir)) {
            color = vec3(1.0, 1.0, 1.0) - color;
        }
    }
    rgb = vec3(cos(color.x * fr), cos(color.y * fg), cos(color.z * fb));
    #{ z: complex(i_f / resolution - 0.5, j_f / resolution - 0.5), rgb: rgb }
}

fn variation3(v, p, ctx) {
    let rgb = ctx.rgb;
    let resolution = max(100.0, to_float(p.density_pixels));
    let time = p.time;
    let n = min(u32(to_float(p.n)), u32(8));
    let dir = u32(to_float(p.direction));
    let fr = p.fr;
    let fg = p.fg;
    let fb = p.fb;
    let i_f = to_float(floor(rand() * resolution));
    let j_f = to_float(floor(rand() * resolution));
    let coord_orig = complex(abs((i_f + 0.5) / resolution - 0.5), abs((j_f + 0.5) / resolution - 0.5));
    coord_orig = coord_orig - floor(coord_orig);
    let color = vec3(cos(time), tan(time), sin(time));
    for i_n in 0..4 {
        let i = u32(i_n);
        let t1 = to_float(i) * 0.1;
        let offset = complex(t1, t1);
        if (squares_hit(offset + coord_orig, n, time, dir)) {
            color = vec3(1.0, 1.0, 1.0) - color;
        }
    }
    rgb = vec3(cos(color.x * fr), cos(color.y * fg), cos(color.z * fb));
    #{ z: vec3(i_f / resolution - 0.5, j_f / resolution - 0.5, v.z), rgb: rgb }
}
