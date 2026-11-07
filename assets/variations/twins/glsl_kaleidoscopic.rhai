/// Kaleidoscope fold followed by 44 iterations of the trirop Apollonian
/// fold (`abs(p)/dot(p,p)` with swizzle/scale). Produces colorful
/// shadertoy-style kaleidoscope patterns with a time-driven rotation.
///
/// 7 params: standard 4 + `sides` (kaleidoscope sector count), `zoom`,
/// `p1` (color phase parameter), `radial` (kaleidoscope mode flag).
/// 
/// # Authors
/// - Jesus Sosa

// GLSL Kaleidoscopic, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    density_pixels: #{
        value: 1000000, min: 100, max: 10000000, free: true,
        caption: "Density Pixels",
        hint: "Discrete grid resolution. See module-level doc.",
    },
    seed: #{
        value: 10000, min: 0, max: 10000, free: true,
        caption: "Seed",
        hint: "JWildfire CPU-only — set `time` directly.",
    },
    time: #{
        value: 0.0, min: -10000.0, max: 10000.0, free: true,
        caption: "Time",
        hint: "Animation parameter. Drives the kaleidoscope's rotation (angle += 0.1·time).",
    },
    sides: #{
        value: 8, min: 2, max: 32, free: true,
        caption: "Sides",
        hint: "Number of kaleidoscope sectors. The kaleidoscope angle is `π / sides`; higher = more rotational symmetry.",
    },
    zoom: #{
        value: 0.0, min: 0.0, max: 1.0, free: true,
        caption: "Zoom",
        hint: "Pre-fold zoom scalar. Multiplies the input by `0.1 + zoom`, so larger values zoom in. Also fed into the per-fold iteration as the initial Z (depth).",
    },
    p1: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "P1",
        hint: "Color phase parameter. Modulates the Apollonian fold's offset vector along the green channel (`p1 · 0.4`). Small changes here shift the visible color regions.",
    },
    radial: #{
        value: 0, min: 0, max: 1, free: true,
        caption: "Radial",
        hint: "Kaleidoscope mode. 0 = pass uv through unchanged. 1 = apply the kaleidoscope fold (sectors + reflection + rotation).",
    },
    gradient: #{
        value: 0, min: 0, max: 1, free: true,
        caption: "Gradient",
        hint: "JWildfire color output mode. **Only mode 0 (direct RGB) honored.**",
    },
};

const INFO = #{ title: "GLSL Kaleidoscopic", name: "rhai_glsl_kaleidoscopic", category: "plugin", phase: "any", z: "never" };

fn kaleidoscopic_kscope(uv, ka, time) {
    let a = atan2(uv.im, uv.re);
    let two_ka = 2.0 * ka;
    a = a - two_ka * to_float(floor(a / two_ka));
    a = abs(a - ka) + 0.1 * time;
    let r = uv.abs();
    complex(cos(a), sin(a)) * r
}

fn variation(w, p, ctx) {
    let rgb = ctx.rgb;
    let resolution = max(100.0, to_float(p.density_pixels));
    let time = p.time;
    let sides = max(1.0, to_float(p.sides));
    let zoom = p.zoom;
    let p1 = p.p1;
    let radial = u32(to_float(p.radial));
    let i_f = to_float(floor(rand() * resolution));
    let j_f = to_float(floor(rand() * resolution));
    let x = i_f + 0.5;
    let y = j_f + 0.5;
    let uv = complex(2.0 * x / resolution - 1.0, 2.0 * y / resolution - 1.0) * 12.0;
    uv = uv * (0.1 + zoom);
    if (radial == u32(1)) {
        let ka = 3.14159265 / sides;
        uv = kaleidoscopic_kscope(uv, ka, time);
    }
    let pp = vec3(uv.re, uv.im, zoom);
    for i_n in 0..44 {
        let i = u32(i_n);
        let d = max(1.0e-32, dot(pp, pp));
        let t1 = abs(pp) / d;
        let t0 = t1 - vec3(1.0, 1.02, p1 * 0.4);
        let t2 = vec3(1.3, 0.999, 0.678) * abs(t0);
        pp = vec3(t2.x, t2.z, t2.y);
    }
    rgb = pp;
    #{ z: complex(i_f / resolution - 0.5, j_f / resolution - 0.5), rgb: rgb }
}

fn variation3(v, p, ctx) {
    let rgb = ctx.rgb;
    let resolution = max(100.0, to_float(p.density_pixels));
    let time = p.time;
    let sides = max(1.0, to_float(p.sides));
    let zoom = p.zoom;
    let p1 = p.p1;
    let radial = u32(to_float(p.radial));
    let i_f = to_float(floor(rand() * resolution));
    let j_f = to_float(floor(rand() * resolution));
    let uv = complex(2.0 * (i_f + 0.5) / resolution - 1.0, 2.0 * (j_f + 0.5) / resolution - 1.0) * 12.0;
    uv = uv * (0.1 + zoom);
    if (radial == u32(1)) {
        let ka = 3.14159265 / sides;
        uv = kaleidoscopic_kscope(uv, ka, time);
    }
    let pp = vec3(uv.re, uv.im, zoom);
    for i_n in 0..44 {
        let i = u32(i_n);
        let d = max(1.0e-32, dot(pp, pp));
        let t1 = abs(pp) / d;
        let t0 = t1 - vec3(1.0, 1.02, p1 * 0.4);
        let t2 = vec3(1.3, 0.999, 0.678) * abs(t0);
        pp = vec3(t2.x, t2.z, t2.y);
    }
    rgb = pp;
    #{ z: vec3(i_f / resolution - 0.5, j_f / resolution - 0.5, v.z), rgb: rgb }
}
