/// 2D Mandelbox with shadertoy-style direct-RGB color output. Picks a
/// uniform random point, maps to `[-7, 7]²`, runs 20 iterations of
/// boxfold + ballfold + scaling with `time`-derived scale factor, then
/// derives RGB from the final distance estimate via interleaved
/// sin/cos.
///
/// 4 params: `density_pixels`, `seed` (CPU-only), `time`, `gradient`
/// (mode 0 honored only).
///
/// # Authors
/// - Jesus Sosa

// GLSL MandelBox2D, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    density_pixels: #{
        value: 1000000, min: 100, max: 10000000, free: true,
        caption: "Density Pixels",
        hint: "Discrete grid resolution for sampling. Higher = smoother continuous output; lower = visible pixelation. JWildfire's 'Density Pixels' slider; clamped at runtime to `[100, 10000000]`.",
    },
    seed: #{
        value: 10000, min: 0, max: 10000, free: true,
        caption: "Seed",
        hint: "JWildfire CPU-only: seeds a Java `Random` and assigns `time = rand(0, 10000)` as a side effect. **Not honored on GPU** — set `time` directly. Value accepted for round-trip.",
    },
    time: #{
        value: 0.0, min: -10000.0, max: 10000.0, free: true,
        caption: "Time",
        hint: "Animation parameter. Embedded in the Mandelbox iteration as `O.z = -5 · cos(time · 0.1)`. Animate this for a flowing/morphing pattern.",
    },
    gradient: #{
        value: 0, min: 0, max: 1, free: true,
        caption: "Gradient",
        hint: "JWildfire color output mode. **Only mode 0 (direct RGB) is honored.** Value accepted for round-trip; see module-level doc.",
    },
};

const INFO = #{ title: "GLSL MandelBox2D", name: "rhai_glsl_mandelbox2d", category: "plugin", phase: "any", z: "never" };

fn variation(q, p, ctx) {
    let rgb = ctx.rgb;
    let resolution = max(100.0, to_float(p.density_pixels));
    let time = p.time;
    let i_f = to_float(floor(rand() * resolution));
    let j_f = to_float(floor(rand() * resolution));
    let x = i_f + 0.5;
    let y = j_f + 0.5;
    let i_vec = complex(7.0 * (x + x - resolution) / resolution, 7.0 * (y + y - resolution) / resolution);
    let o = vec4(i_vec.re, i_vec.im, -5.0 * cos(time * 0.1), 1.0);
    let d = 1.0;
    for k_n in 0..20 {
        let k = u32(k_n);
        let t2 = -1.0;
        let t1 = complex(t2, t2);
        let t3 = complex(1.0, 1.0);
        i_vec = complex(clamp(i_vec.re, t1.re, t3.re), clamp(i_vec.im, t1.im, t3.im)) * 2.0 - i_vec;
        o.w = i_vec.abs();
        let b = 0.0;
        if (o.w < 0.5) {
            b = 4.0;
        } else if (o.w < 1.0) {
            b = 1.0 / o.w;
        } else {
            b = 1.0;
        }
        i_vec = i_vec * (o.z * b) + complex(o.x, o.y);
        d = b * d * abs(o.z) + 1.0;
    }
    let d_final = pow(i_vec.abs() / d, 0.1) * 5.0;
    let t4 = vec3(cos(d_final), sin(10.0 * d_final + 1.0), cos(3.0 * d_final + 1.0)) * 0.5;
    let color = vec3(t4.x + 0.5, t4.y + 0.5, t4.z + 0.5);
    rgb = color;
    #{ z: complex(i_f / resolution - 0.5, j_f / resolution - 0.5), rgb: rgb }
}

fn variation3(v, p, ctx) {
    let rgb = ctx.rgb;
    let resolution = max(100.0, to_float(p.density_pixels));
    let time = p.time;
    let i_f = to_float(floor(rand() * resolution));
    let j_f = to_float(floor(rand() * resolution));
    let i_vec = complex(7.0 * ((i_f + 0.5) + (i_f + 0.5) - resolution) / resolution, 7.0 * ((j_f + 0.5) + (j_f + 0.5) - resolution) / resolution);
    let o = vec4(i_vec.re, i_vec.im, -5.0 * cos(time * 0.1), 1.0);
    let d = 1.0;
    for k_n in 0..20 {
        let k = u32(k_n);
        let t2 = -1.0;
        let t1 = complex(t2, t2);
        let t3 = complex(1.0, 1.0);
        i_vec = complex(clamp(i_vec.re, t1.re, t3.re), clamp(i_vec.im, t1.im, t3.im)) * 2.0 - i_vec;
        o.w = i_vec.abs();
        let b = 0.0;
        if (o.w < 0.5) {
            b = 4.0;
        } else if (o.w < 1.0) {
            b = 1.0 / o.w;
        } else {
            b = 1.0;
        }
        i_vec = i_vec * (o.z * b) + complex(o.x, o.y);
        d = b * d * abs(o.z) + 1.0;
    }
    let d_final = pow(i_vec.abs() / d, 0.1) * 5.0;
    let t4 = vec3(cos(d_final), sin(10.0 * d_final + 1.0), cos(3.0 * d_final + 1.0)) * 0.5;
    rgb = vec3(t4.x + 0.5, t4.y + 0.5, t4.z + 0.5);
    #{ z: vec3(i_f / resolution - 0.5, j_f / resolution - 0.5, v.z), rgb: rgb }
}
