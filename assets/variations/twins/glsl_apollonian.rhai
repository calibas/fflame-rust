/// Apollonian gasket fractal — sphere packing pattern.
///
/// **Implementation note**: JWildfire's source delegates to `js.glsl.G.rot`
/// (Euler-angle rotation matrix from a `vec3`) and `js.glsl.G.app`
/// (Apollonian gasket iterator). Neither helper is in the source
/// snapshot we have. We use:
///   - `G.rot(a)` → ZYX Euler rotation matrix
///   - `G.app(p, k, m)` → the canonical Iñigo Quílez Apollonian fold:
///     8 iterations of `p = -1 + 2·fract(0.5 + 0.5·p); s = k/dot(p,p);
///     p *= s`, with `m * p` as the initial transform
///
/// These are the standard shadertoy formulas and produce visibly
/// Apollonian-like output, but bit-for-bit visual identity with
/// JWildfire would need the actual `js.glsl.G` source. If a future
/// audit finds JWildfire's helpers differ, the bodies here are the
/// only thing to change — the framework integration is fine.
///
/// 4 params: `density_pixels`, `seed` (CPU-only), `time`, `gradient`.
/// 
/// # Authors
/// - Jesus Sosa

// GLSL Apollonian, the built-in variation's twin, translated from its WGSL.

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
        hint: "Animation parameter. Drives the rotation matrix (Euler angles `t + (1,2,3)`), the fold factor `k = 1.2 + 0.1·sin(0.1·time)`, and the initial scale `f1 = 2 + 0.25·sin(0.3·time)`.",
    },
    gradient: #{
        value: 0, min: 0, max: 1, free: true,
        caption: "Gradient",
        hint: "JWildfire color output mode. **Only mode 0 honored.**",
    },
};

const INFO = #{ title: "GLSL Apollonian", name: "rhai_glsl_apollonian", category: "plugin", phase: "any", z: "never" };

fn apollonian_rot(a) {
    let sx = sin(a.x);
    let cx = cos(a.x);
    let sy = sin(a.y);
    let cy = cos(a.y);
    let sz = sin(a.z);
    let cz = cos(a.z);
    [cy * cz, cy * sz, -sy, sx * sy * cz - cx * sz, sx * sy * sz + cx * cz, sx * cy, cx * sy * cz + sx * sz, cx * sy * sz - sx * cz, cx * cy]
}

fn apollonian_fold(p_in, k, m) {
    let pp = vec3((m[0] * p_in.x + m[3] * p_in.y + m[6] * p_in.z), (m[1] * p_in.x + m[4] * p_in.y + m[7] * p_in.z), (m[2] * p_in.x + m[5] * p_in.y + m[8] * p_in.z));
    for i_n in 0..8 {
        let i = u32(i_n);
        let t1 = -1.0;
        let t4 = 0.5 * pp;
        let t3 = vec3(0.5 + t4.x, 0.5 + t4.y, 0.5 + t4.z);
        let t2 = 2.0 * vec3(fract(t3.x), fract(t3.y), fract(t3.z));
        pp = vec3(t1 + t2.x, t1 + t2.y, t1 + t2.z);
        let r2 = max(1.0e-32, dot(pp, pp));
        pp = pp * (k / r2);
    }
    pp
}

fn variation(z, p, ctx) {
    let rgb = ctx.rgb;
    let resolution = max(100.0, to_float(p.density_pixels));
    let time = p.time;
    let i_f = to_float(floor(rand() * resolution));
    let j_f = to_float(floor(rand() * resolution));
    let x = i_f + 0.5;
    let y = j_f + 0.5;
    let uv = complex(2.0 * x / resolution - 1.0, 2.0 * y / resolution - 1.0);
    let t = 0.05 * time;
    let m = apollonian_rot(vec3(t + 1.0, t + 2.0, t + 3.0));
    let k = 1.2 + 0.1 * sin(0.1 * time);
    let f1 = 2.0 + 0.25 * sin(0.3 * time);
    let v2 = uv * 2.0;
    let t1 = vec3(v2.re, v2.im, 0.0);
    let v = vec3((m[0] * t1.x + m[3] * t1.y + m[6] * t1.z), (m[1] * t1.x + m[4] * t1.y + m[7] * t1.z), (m[2] * t1.x + m[5] * t1.y + m[8] * t1.z)) * f1;
    let t2 = apollonian_fold(v, k, m);
    let v3 = vec3(sin(t2.x), sin(t2.y), sin(t2.z));
    let t3 = v3 * 0.6;
    rgb = vec3(t3.x + 0.5, t3.y + 0.5, t3.z + 0.5);
    #{ z: complex(i_f / resolution - 0.5, j_f / resolution - 0.5), rgb: rgb }
}

fn variation3(q, p, ctx) {
    let rgb = ctx.rgb;
    let resolution = max(100.0, to_float(p.density_pixels));
    let time = p.time;
    let i_f = to_float(floor(rand() * resolution));
    let j_f = to_float(floor(rand() * resolution));
    let uv = complex(2.0 * (i_f + 0.5) / resolution - 1.0, 2.0 * (j_f + 0.5) / resolution - 1.0);
    let t = 0.05 * time;
    let m = apollonian_rot(vec3(t + 1.0, t + 2.0, t + 3.0));
    let k = 1.2 + 0.1 * sin(0.1 * time);
    let f1 = 2.0 + 0.25 * sin(0.3 * time);
    let v2 = uv * 2.0;
    let t1 = vec3(v2.re, v2.im, 0.0);
    let v = vec3((m[0] * t1.x + m[3] * t1.y + m[6] * t1.z), (m[1] * t1.x + m[4] * t1.y + m[7] * t1.z), (m[2] * t1.x + m[5] * t1.y + m[8] * t1.z)) * f1;
    let t2 = apollonian_fold(v, k, m);
    let v3 = vec3(sin(t2.x), sin(t2.y), sin(t2.z));
    let t3 = v3 * 0.6;
    rgb = vec3(t3.x + 0.5, t3.y + 0.5, t3.z + 0.5);
    #{ z: vec3(i_f / resolution - 0.5, j_f / resolution - 0.5, q.z), rgb: rgb }
}
