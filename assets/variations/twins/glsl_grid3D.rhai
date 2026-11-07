/// 3D raymarched grid pattern (kabuto's shadertoy, forked by tigrou
/// and kapsy). Casts a ray through 30 raymarch steps into a procedural
/// 3-iteration field, accumulates color by distance, then maps to RGB.
/// Output is grayscale (the JWildfire source returns `vec3(sum)`).
///
/// 3 params (no algorithm knobs — JWildfire only exposes the
/// standard 3).
/// 
/// # Authors
/// - Jesus Sosa

// GLSL Grid 3D, the built-in variation's twin, translated from its WGSL.

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
        value: 200.0, min: 1.0, max: 1000.0, free: true,
        caption: "Time",
        hint: "Animation parameter. Drives the camera rotation (`a = time · 0.021`) and the camera Y position (`pos.y = time · 0.1`).",
    },
    gradient: #{
        value: 0, min: 0, max: 1, free: true,
        caption: "Gradient",
        hint: "JWildfire color output mode. **Only mode 0 honored.** Output is grayscale.",
    },
};

const INFO = #{ title: "GLSL Grid 3D", name: "rhai_glsl_grid3d", category: "plugin", phase: "any", z: "never" };

fn grid3d_field(p_in) {
    let pp = p_in * 0.1;
    let f = 0.1;
    for i_n in 0..3 {
        let i = u32(i_n);
        pp = vec3(pp.y, pp.z, pp.x);
        let t1 = vec3(fract(pp.x), fract(pp.y), fract(pp.z));
        pp = abs(vec3(t1.x - 0.5, t1.y - 0.5, t1.z - 0.5));
        pp = pp * 2.0;
        f = f * 2.0;
    }
    let pp_sq = pp * pp;
    let sum = pp_sq + vec3(pp_sq.y, pp_sq.z, pp_sq.x);
    let safe_f = max(1.0e-32, f);
    let t3 = max(sum, vec3(0.0, 0.0, 0.0));
    let t2 = vec3(sqrt(t3.x), sqrt(t3.y), sqrt(t3.z)) / safe_f;
    vec3(t2.x - 0.05, t2.y - 0.05, t2.z - 0.05)
}

fn variation(w, p, ctx) {
    let rgb = ctx.rgb;
    let resolution = max(100.0, to_float(p.density_pixels));
    let time = p.time;
    let i_f = to_float(floor(rand() * resolution));
    let j_f = to_float(floor(rand() * resolution));
    let x = i_f + 0.5;
    let y = j_f + 0.5;
    let max_iter = u32(30);
    let dir = normalize(vec3(x / resolution - 0.5, y / resolution - 0.5, 1.0));
    let a = time * 0.021;
    let pos = vec3(0.0, time * 0.1, 0.0);
    let ca = cos(a);
    let sa = sin(a);
    dir = vec3(dir.x, dir.y * ca - dir.z * sa, dir.y * sa + dir.z * ca);
    dir = vec3(dir.x * ca - dir.z * sa, dir.y, dir.x * sa + dir.z * ca);
    let color = vec3(0.0, 0.0, 0.0);
    for i_n in 0..to_int(max_iter) {
        let i = u32(i_n);
        let f2 = grid3d_field(pos);
        let f = min(min(f2.x, f2.y), f2.z);
        pos = pos + dir * f;
        let t1 = to_float(max_iter - i);
        let t0 = vec3(t1, t1, t1) / (vec3(f2.x + 0.1, f2.y + 0.1, f2.z + 0.1));
        color = color + t0;
    }
    let t2 = color * (0.09 / to_float(max_iter * max_iter));
    let denom = vec3(1.0 + t2.x, 1.0 + t2.y, 1.0 + t2.z);
    let t3 = denom.x;
    let t4 = denom.y;
    let t5 = denom.z;
    let safe_denom = vec3((if t3 > 0.0 { 1.0 } else if t3 < 0.0 { -1.0 } else { 0.0 }), (if t4 > 0.0 { 1.0 } else if t4 < 0.0 { -1.0 } else { 0.0 }), (if t5 > 0.0 { 1.0 } else if t5 < 0.0 { -1.0 } else { 0.0 })) * max(abs(denom), vec3(1.0e-32, 1.0e-32, 1.0e-32));
    let color3 = vec3(1.0, 1.0, 1.0) - vec3(1.0, 1.0, 1.0) / safe_denom;
    let sum = color3.x + color3.y + color3.z;
    rgb = vec3(sum, sum, sum);
    #{ z: complex(i_f / resolution - 0.5, j_f / resolution - 0.5), rgb: rgb }
}

fn variation3(v, p, ctx) {
    let rgb = ctx.rgb;
    let resolution = max(100.0, to_float(p.density_pixels));
    let time = p.time;
    let i_f = to_float(floor(rand() * resolution));
    let j_f = to_float(floor(rand() * resolution));
    let max_iter = u32(30);
    let dir = normalize(vec3((i_f + 0.5) / resolution - 0.5, (j_f + 0.5) / resolution - 0.5, 1.0));
    let a = time * 0.021;
    let pos = vec3(0.0, time * 0.1, 0.0);
    let ca = cos(a);
    let sa = sin(a);
    dir = vec3(dir.x, dir.y * ca - dir.z * sa, dir.y * sa + dir.z * ca);
    dir = vec3(dir.x * ca - dir.z * sa, dir.y, dir.x * sa + dir.z * ca);
    let color = vec3(0.0, 0.0, 0.0);
    for i_n in 0..to_int(max_iter) {
        let i = u32(i_n);
        let f2 = grid3d_field(pos);
        let f = min(min(f2.x, f2.y), f2.z);
        pos = pos + dir * f;
        let t1 = to_float(max_iter - i);
        let t0 = vec3(t1, t1, t1) / (vec3(f2.x + 0.1, f2.y + 0.1, f2.z + 0.1));
        color = color + t0;
    }
    let t2 = color * (0.09 / to_float(max_iter * max_iter));
    let denom = vec3(1.0 + t2.x, 1.0 + t2.y, 1.0 + t2.z);
    let t3 = denom.x;
    let t4 = denom.y;
    let t5 = denom.z;
    let safe_denom = vec3((if t3 > 0.0 { 1.0 } else if t3 < 0.0 { -1.0 } else { 0.0 }), (if t4 > 0.0 { 1.0 } else if t4 < 0.0 { -1.0 } else { 0.0 }), (if t5 > 0.0 { 1.0 } else if t5 < 0.0 { -1.0 } else { 0.0 })) * max(abs(denom), vec3(1.0e-32, 1.0e-32, 1.0e-32));
    let color3 = vec3(1.0, 1.0, 1.0) - vec3(1.0, 1.0, 1.0) / safe_denom;
    let sum = color3.x + color3.y + color3.z;
    rgb = vec3(sum, sum, sum);
    #{ z: vec3(i_f / resolution - 0.5, j_f / resolution - 0.5, v.z), rgb: rgb }
}
