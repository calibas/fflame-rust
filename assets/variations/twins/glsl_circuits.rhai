/// Circuit-board-style fractal pattern. Layered fractal-fold formula
/// applied 1..24 times (per `loops`) and accumulated.
///
/// **Divergence from JWildfire**: JWildfire's `formula` writes to a
/// class-level `double S` field that persists across every call to
/// every transform's variation (across millions of chaos-game iters).
/// This is broken multithread semantics — `S` is initialized once at
/// flame load and grows without bound. Our port uses a per-call local
/// `S` so the algorithm is well-defined; visual output may differ
/// from JWildfire because the global accumulation is not replicated
/// (and cannot be on GPU).
///
/// 9 params: standard 4 (sans seed — none in source) + `rate`,
/// `intensity`, `focus`, `pulse`, `glow`, `loops`, `zoom`.
/// 
/// # Authors
/// - Jesus Sosa

// GLSL Circuits, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    density_pixels: #{
        value: 1000000, min: 100, max: 10000000, free: true,
        caption: "Density Pixels",
        hint: "Discrete grid resolution.",
    },
    time: #{
        value: 0.0, min: -10000.0, max: 10000.0, free: true,
        caption: "Time",
        hint: "Animation parameter. Drives the per-iteration rotation matrices and the circuit phase.",
    },
    rate: #{
        value: 0.8, min: 0.0, max: 5.0, free: true,
        caption: "Rate",
        hint: "Time scaling. Multiplies `time · 0.01` to derive the inner phase N.",
    },
    intensity: #{
        value: 0.9, min: 0.0, max: 5.0, free: true,
        caption: "Intensity",
        hint: "Brightness scalar applied to the per-formula color mix.",
    },
    focus: #{
        value: 0.6, min: 0.0, max: 5.0, free: true,
        caption: "Focus",
        hint: "Depth-of-field-like blur. Multiplies the per-sample pixel offset; higher = more visible per-pixel jitter.",
    },
    pulse: #{
        value: 1.5, min: 0.0, max: 10.0, free: true,
        caption: "Pulse",
        hint: "Color cycle rate. Multiplies the time term in the per-iteration color mod.",
    },
    glow: #{
        value: 0.0, min: 0.0, max: 5.0, free: true,
        caption: "Glow",
        hint: "Initial value of the local `S` accumulator (`S₀ = (101 + glow) · intensity`).",
    },
    loops: #{
        value: 1, min: 1, max: 10, free: true,
        caption: "Loops",
        hint: "Outer (anti-alias) loop count. **GPU-clamped to ≤ 10.** Inner `formula` loop count also follows: each AA sample re-runs the 11-iter formula with K = floor(loops/4 + floor(5·zoom)) inner iters.",
    },
    zoom: #{
        value: 0.0, min: -1.0, max: 1.0, free: true,
        caption: "Zoom",
        hint: "Zoom factor. Also gates the inner formula loop count (`K = floor(loops/4 + floor(5·zoom))`).",
    },
    gradient: #{
        value: 0, min: 0, max: 1, free: true,
        caption: "Gradient",
        hint: "JWildfire color output mode. **Only mode 0 honored.**",
    },
};

const INFO = #{ title: "GLSL Circuits", name: "rhai_glsl_circuits", category: "plugin", phase: "any", z: "never" };

fn circuits_formula(z_in, t, intensity, zoom, pulse, loops) {
    let z = z_in;
    let m = 0.0;
    let o = 0.0;
    let ot = 1000.0;
    let ot2 = 1000.0;
    let k = to_float(floor(loops / 4.0 + to_float(floor(5.0 * zoom))));
    let color = vec3(0.0, 0.0, 0.0);
    for i in 0..11 {
        let d = z.norm_sqr();
        let c_d = clamp(d, 0.1, 0.5);
        let t3 = complex(abs(z.re), abs(z.im)) / c_d;
        z = complex(t3.re - t, t3.im - t);
        let l = z.abs();
        o = min(max(abs(min(z.re, z.im)), -l + 0.25), abs(l - 0.25));
        ot = min(ot, o);
        ot2 = min(l * 0.1, ot2);
        let t4 = ot - o;
        m = max(m, to_float(i) * (1.0 - abs((if t4 > 0.0 { 1.0 } else if t4 < 0.0 { -1.0 } else { 0.0 }))));
        if (k <= 0.0) {
            break;
        }
        k = k - 1.0;
    }
    m = m + 1.0;
    let w = (intensity * zoom) * m;
    let safe_w = (if w > 0.0 { 1.0 } else if w < 0.0 { -1.0 } else { 0.0 }) * max(1.0e-32, abs(w));
    let circ = pow(max(0.0, w - ot2) / safe_w, 6.0);
    this = this + max(pow(max(0.0, w - ot) / safe_w, 0.25), circ);
    let t1 = vec3(0.1, 0.1, 0.1) + vec3(0.45, 0.75, m * 0.1);
    let col_n = normalize(t1);
    let arg = m / 9.0 - t * pulse + ot2 * 2.0;
    let arg_mod = arg - to_float(floor(arg));
    let t5 = 0.4 + arg_mod;
    let t2 = vec3(t5, t5, t5);
    color = color + col_n * t2;
    let f1 = circ * (10.0 - m) * 3.0;
    color + vec3(1.0, 0.7, 0.3) * f1
}

fn variation(z, p, ctx) {
    let rgb = ctx.rgb;
    let resolution = max(100.0, to_float(p.density_pixels));
    let time = p.time;
    let rate = p.rate;
    let intensity = p.intensity;
    let focus = p.focus;
    let pulse = p.pulse;
    let glow = p.glow;
    let loops_f = max(1.0, to_float(p.loops));
    let loops = min(u32(loops_f), u32(10));
    let zoom = p.zoom;
    let i_f = to_float(floor(rand() * resolution));
    let j_f = to_float(floor(rand() * resolution));
    let pos = complex((i_f + 0.5) / resolution - 0.5, (j_f + 0.5) / resolution - 0.5);
    let n = time * 0.01 * rate;
    let r = 0.0;
    if (n > 6.0 * rate) {
        r = r + 1.0;
        n = n - r * 8.0 * rate;
    }
    let t = 2.0 * rate;
    if (n < 4.0 * rate) {
        t = t + n;
    } else {
        t = 8.0 * rate - n;
    }
    let z_param = 1.05 - zoom;
    let uv = pos;
    let sph_raw = uv.abs() * 0.1;
    let sph = sqrt(max(0.0, 1.0 - sph_raw * sph_raw)) * 2.0;
    let safe_sph = max(1.0e-32, sph);
    let a = t * 3.14159265;
    let b = a + t;
    let c = cos(a) + sin(b);
    let cb = cos(b);
    let sb = sin(b);
    let ca = cos(a);
    let sa = sin(a);
    uv = complex(uv.re * cb - uv.im * sb, uv.re * sb + uv.im * cb);
    uv = complex(uv.re * ca + uv.im * sa, -uv.re * sa + uv.im * ca);
    uv = uv - complex(sin(c), cos(c)) / 3.14159265;
    uv = uv * z_param;
    let pix = 0.5 / resolution * z_param / safe_sph;
    let dof = (zoom * focus) + (t * 0.25);
    let color = vec3(0.0, 0.0, 0.0);
    let s_accum = (101.0 + glow) * intensity;
    for aa in 0..to_int(loops) {
        let aa_uv = complex(to_float(floor(to_float(aa) / 6.0)), to_float(aa) - 6.0 * to_float(floor(to_float(aa) / 6.0)));
        color = s_accum.circuits_formula(uv + aa_uv * pix * dof, t, intensity, zoom, pulse, loops_f);
    }
    let safe_loops = max(1.0, to_float(loops));
    s_accum = s_accum / safe_loops;
    color = color / safe_loops;
    let t1 = vec3(0.15, 0.15, 0.15);
    let t2 = vec3(s_accum, s_accum, s_accum);
    let mixed = vec3(mix(t1.x, color.x, t2.x), mix(t1.y, color.y, t2.y), mix(t1.z, color.z, t2.z)) * (1.0 - pos.abs());
    rgb = mixed * vec3(1.2, 1.1, 1.0);
    #{ z: complex(i_f / resolution - 0.5, j_f / resolution - 0.5), rgb: rgb }
}

fn variation3(v, p, ctx) {
    let rgb = ctx.rgb;
    let resolution = max(100.0, to_float(p.density_pixels));
    let time = p.time;
    let rate = p.rate;
    let intensity = p.intensity;
    let focus = p.focus;
    let pulse = p.pulse;
    let glow = p.glow;
    let loops_f = max(1.0, to_float(p.loops));
    let loops = min(u32(loops_f), u32(10));
    let zoom = p.zoom;
    let i_f = to_float(floor(rand() * resolution));
    let j_f = to_float(floor(rand() * resolution));
    let pos = complex((i_f + 0.5) / resolution - 0.5, (j_f + 0.5) / resolution - 0.5);
    let n = time * 0.01 * rate;
    let r = 0.0;
    if (n > 6.0 * rate) {
        r = r + 1.0;
        n = n - r * 8.0 * rate;
    }
    let t = 2.0 * rate;
    if (n < 4.0 * rate) {
        t = t + n;
    } else {
        t = 8.0 * rate - n;
    }
    let z_param = 1.05 - zoom;
    let uv = pos;
    let sph_raw = uv.abs() * 0.1;
    let sph = sqrt(max(0.0, 1.0 - sph_raw * sph_raw)) * 2.0;
    let safe_sph = max(1.0e-32, sph);
    let a = t * 3.14159265;
    let b = a + t;
    let c = cos(a) + sin(b);
    let cb = cos(b);
    let sb = sin(b);
    let ca = cos(a);
    let sa = sin(a);
    uv = complex(uv.re * cb - uv.im * sb, uv.re * sb + uv.im * cb);
    uv = complex(uv.re * ca + uv.im * sa, -uv.re * sa + uv.im * ca);
    uv = uv - complex(sin(c), cos(c)) / 3.14159265;
    uv = uv * z_param;
    let pix = 0.5 / resolution * z_param / safe_sph;
    let dof = (zoom * focus) + (t * 0.25);
    let color = vec3(0.0, 0.0, 0.0);
    let s_accum = (101.0 + glow) * intensity;
    for aa in 0..to_int(loops) {
        let aa_uv = complex(to_float(floor(to_float(aa) / 6.0)), to_float(aa) - 6.0 * to_float(floor(to_float(aa) / 6.0)));
        color = s_accum.circuits_formula(uv + aa_uv * pix * dof, t, intensity, zoom, pulse, loops_f);
    }
    let safe_loops = max(1.0, to_float(loops));
    s_accum = s_accum / safe_loops;
    color = color / safe_loops;
    let t1 = vec3(0.15, 0.15, 0.15);
    let t2 = vec3(s_accum, s_accum, s_accum);
    let mixed = vec3(mix(t1.x, color.x, t2.x), mix(t1.y, color.y, t2.y), mix(t1.z, color.z, t2.z)) * (1.0 - pos.abs());
    rgb = mixed * vec3(1.2, 1.1, 1.0);
    #{ z: vec3(i_f / resolution - 0.5, j_f / resolution - 0.5, v.z), rgb: rgb }
}
