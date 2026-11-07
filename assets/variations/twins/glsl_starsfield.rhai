/// Animated star field with rotating layers. Each of 10 layers picks
/// a hash-based offset, rotates the uv, renders a soft-edged "star"
/// point via smoothstep, fading in/out over a triangle wave. Final
/// color adds a `glow` term derived from a 2D hash.
///
/// 5 params.
/// 
/// # Authors
/// - Jesus Sosa

// GLSL StarsField, the built-in variation's twin, translated from its WGSL.

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
        hint: "Animation parameter. Drives the global rotation and the per-layer phase.",
    },
    z_distance: #{
        value: 2.0, min: 0.1, max: 10.0, free: true,
        caption: "Z Distance",
        hint: "Layer perspective distance. Used in the layer-local `f = fract(uv·5)·z - 1` mapping; lower = closer (larger stars).",
    },
    glow: #{
        value: 2.0, min: 0.0, max: 10.0, free: true,
        caption: "Glow",
        hint: "Brightness of the per-pixel hash glow added on top of all layers.",
    },
    gradient: #{
        value: 0, min: 0, max: 1, free: true,
        caption: "Gradient",
        hint: "JWildfire color output mode. **Only mode 0 honored.**",
    },
};

const INFO = #{ title: "GLSL StarsField", name: "rhai_glsl_starsfield", category: "plugin", phase: "any", z: "never" };

fn starsfield_hash11(p_in) {
    let pp = fract(p_in * 35.35);
    pp = pp + pp * (pp + 45.85);
    fract(pp * 7858.58)
}

fn starsfield_hash21(p_in) {
    let t1 = complex(451.45, 231.95);
    let pp = fract(complex(p_in.re * t1.re, p_in.im * t1.im));
    let t3 = pp + complex(78.78, 78.78);
    let t2 = (pp.re * t3.re + pp.im * t3.im);
    pp = complex(pp.re + t2, pp.im + t2);
    fract(pp.re * pp.im)
}

fn starsfield_hash22(p_in) {
    let t1 = vec3(p_in.re, p_in.im, p_in.re);
    t1 = t1 * vec3(451.45, 231.95, 7878.5);
    let q = vec3(fract(t1.x), fract(t1.y), fract(t1.z));
    let t2 = dot(q, q + vec3(78.78, 78.78, 78.78));
    q = vec3(q.x + t2, q.y + t2, q.z + t2);
    let t3 = complex(q.x, q.z);
    let t5 = q.y;
    let t4 = complex(t5, t5);
    fract(complex(t3.re * t4.re, t3.im * t4.im))
}

fn starsfield_layer(uv_in, z_distance) {
    let uv = uv_in * 5.0;
    let i = floor(uv);
    let t1 = fract(uv) * z_distance;
    let f = complex(t1.re - 1.0, t1.im - 1.0);
    let pp = starsfield_hash22(i) * 0.3;
    let d = (f - pp).abs();
    let t2 = 0.1 + 0.8 * starsfield_hash21(i);
    let t3 = clamp((d - t2) / (0.01 - t2), 0.0, 1.0);
    let c = (t3 * t3 * (3.0 - 2.0 * t3));
    let safe_d = max(1.0e-32, d);
    c = c * (1.0 / safe_d) * 0.2;
    c
}

fn starsfield_rotate(uv, a) {
    let c = cos(a);
    let s = sin(a);
    complex(uv.re * c - uv.im * s, uv.re * s + uv.im * c)
}

fn variation(z, p, ctx) {
    let rgb = ctx.rgb;
    let resolution = max(100.0, to_float(p.density_pixels));
    let time = p.time;
    let z_distance = p.z_distance;
    let glow = p.glow;
    let i_f = to_float(floor(rand() * resolution));
    let j_f = to_float(floor(rand() * resolution));
    let uv = complex((i_f + 0.5) / resolution - 0.5, (j_f + 0.5) / resolution - 0.5);
    let col = vec3(0.0, 0.0, 0.0);
    uv = starsfield_rotate(uv, time);
    uv = uv + complex(cos(time), sin(time)) * 2.0;
    for ix_n in 0..10 {
        let ix = u32(ix_n);
        let i = to_float(ix) * 0.1;
        uv = starsfield_rotate(uv, starsfield_hash11(i) * 6.28);
        let t_arg = i - time;
        let t = fract(t_arg);
        let t1 = clamp((t - 0.0) / (1.0 - 0.0), 0.0, 1.0);
        let s = (t1 * t1 * (3.0 - 2.0 * t1));
        let t2 = clamp((t - 0.0) / (1.0 - 0.0), 0.0, 1.0);
        let f = (t2 * t2 * (3.0 - 2.0 * t2));
        let t3 = clamp((t - 1.0) / (0.0 - 1.0), 0.0, 1.0);
        f = f * (t3 * t3 * (3.0 - 2.0 * t3));
        let k = starsfield_hash22(complex(i, i * 5.0)) * 0.1;
        let l = starsfield_layer((uv - k) * s, z_distance);
        let t4 = vec3(0.0, 0.0, 0.0);
        let t5 = vec3(1.0, 1.0, 1.0);
        let t6 = vec3(l, l, l);
        col = col + vec3(mix(t4.x, t5.x, t6.x), mix(t4.y, t5.y, t6.y), mix(t4.z, t5.z, t6.z)) * f;
    }
    let glow_v = glow * starsfield_hash21(complex(uv.re + time, uv.im + time));
    col = col + vec3(glow_v, glow_v, glow_v);
    rgb = col;
    #{ z: complex(i_f / resolution - 0.5, j_f / resolution - 0.5), rgb: rgb }
}

fn variation3(v, p, ctx) {
    let rgb = ctx.rgb;
    let resolution = max(100.0, to_float(p.density_pixels));
    let time = p.time;
    let z_distance = p.z_distance;
    let glow = p.glow;
    let i_f = to_float(floor(rand() * resolution));
    let j_f = to_float(floor(rand() * resolution));
    let uv = complex((i_f + 0.5) / resolution - 0.5, (j_f + 0.5) / resolution - 0.5);
    let col = vec3(0.0, 0.0, 0.0);
    uv = starsfield_rotate(uv, time);
    uv = uv + complex(cos(time), sin(time)) * 2.0;
    for ix_n in 0..10 {
        let ix = u32(ix_n);
        let i = to_float(ix) * 0.1;
        uv = starsfield_rotate(uv, starsfield_hash11(i) * 6.28);
        let t_arg = i - time;
        let t = fract(t_arg);
        let t1 = clamp((t - 0.0) / (1.0 - 0.0), 0.0, 1.0);
        let s = (t1 * t1 * (3.0 - 2.0 * t1));
        let t2 = clamp((t - 0.0) / (1.0 - 0.0), 0.0, 1.0);
        let f = (t2 * t2 * (3.0 - 2.0 * t2));
        let t3 = clamp((t - 1.0) / (0.0 - 1.0), 0.0, 1.0);
        f = f * (t3 * t3 * (3.0 - 2.0 * t3));
        let k = starsfield_hash22(complex(i, i * 5.0)) * 0.1;
        let l = starsfield_layer((uv - k) * s, z_distance);
        let t4 = vec3(0.0, 0.0, 0.0);
        let t5 = vec3(1.0, 1.0, 1.0);
        let t6 = vec3(l, l, l);
        col = col + vec3(mix(t4.x, t5.x, t6.x), mix(t4.y, t5.y, t6.y), mix(t4.z, t5.z, t6.z)) * f;
    }
    let glow_v = glow * starsfield_hash21(complex(uv.re + time, uv.im + time));
    col = col + vec3(glow_v, glow_v, glow_v);
    rgb = col;
    #{ z: vec3(i_f / resolution - 0.5, j_f / resolution - 0.5, v.z), rgb: rgb }
}
