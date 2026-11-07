/// Chladni-plate nodal-line attractor (sand-on-a-vibrating-plate
/// cymatics).
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5

// Chladni, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    m: #{
        value: 3.0, min: 1.0, max: 20.0,
        caption: "Mode M",
        hint: "First mode number of the standing wave. Integer values give classic Chladni figures; fractional values morph smoothly between them (good for animation).",
    },
    n: #{
        value: 5.0, min: 1.0, max: 20.0,
        caption: "Mode N",
        hint: "Second mode number. Patterns are most interesting when M ≠ N; swapping M and N mirrors the figure across the diagonal.",
    },
    a: #{
        value: 1.0, min: -2.0, max: 2.0,
        caption: "Mix A",
        hint: "Amplitude of the sin(πMx)·sin(πNy) term. With b = ±a you get the classic symmetric plate figures; unequal values skew the pattern.",
    },
    b: #{
        value: 1.0, min: -2.0, max: 2.0,
        caption: "Mix B",
        hint: "Amplitude of the mirrored sin(πNx)·sin(πMy) term. Negative values flip the interference and produce the diagonal-symmetric family.",
    },
    size: #{
        value: 1.0, min: 0.1, max: 4.0,
        caption: "Size",
        hint: "Spatial scale of the plate: one period of the pattern spans 2·size units. Purely a coordinate scale — same figure, bigger or smaller.",
    },
    steps: #{
        value: 3, min: 1, max: 6,
        caption: "Steps",
        hint: "Newton iterations toward the nodal line per call. 1 is soft and halo-like; 3+ lands points crisply on the line.",
    },
    strength: #{
        value: 0.9, min: 0.0, max: 1.0,
        caption: "Strength",
        hint: "Blend between the untouched input point (0) and the fully projected point (1). Below 1 the pattern reads as attraction rather than hard snapping, which composes better with other variations.",
    },
    jitter: #{
        value: 0.01, min: 0.0, max: 0.2,
        caption: "Jitter",
        hint: "Isotropic random offset added after projection — a sand-grain look. 0 keeps the nodal lines razor thin.",
    },
    dc_mode: #{
        value: 0, choices: ["Off", "Distance", "Amplitude", "Mode Mix"],
        caption: "Color Mode",
        hint: "Direct-color source, applied through the transform's Direct Color slider. Distance: palette position 1 on the nodal lines fading to 0 away from them (great at low Strength). Amplitude: colors the cells between lines by signed vibration phase — the physical checkerboard of a real plate. Mode Mix: colors along the figure by the two modes' signed push-pull balance (varies along the nodal curves, unlike a ratio which is constant on them).",
    },
    dc_scale: #{
        value: 1.0, min: 0.1, max: 4.0,
        caption: "Color Scale",
        hint: "Contrast for the direct-color modes: Distance falloff sharpness, Amplitude saturation. No effect when Color Mode is Off.",
    },
};

const INFO = #{ title: "Chladni", name: "rhai_chladni", category: "advanced_2d", z: "preserve_z" };

fn chladni_field(q, m, n, a, b, size) {
    let pi = 3.14159265359;
    let sx = q / size * pi;
    let smx = sin(m * sx.re);
    let cmx = cos(m * sx.re);
    let snx = sin(n * sx.re);
    let cnx = cos(n * sx.re);
    let smy = sin(m * sx.im);
    let cmy = cos(m * sx.im);
    let sny = sin(n * sx.im);
    let cny = cos(n * sx.im);
    let k = pi / size;
    vec3(a * smx * sny + b * snx * smy, k * (a * m * cmx * sny + b * n * cnx * smy), k * (a * n * smx * cny + b * m * snx * cmy))
}

fn variation(z, p, ctx) {
    let color = ctx.color;
    let m = p.m;
    let n = p.n;
    let a = p.a;
    let b = p.b;
    let size = max(1.0e-6, p.size);
    let steps = to_int(to_float(p.steps));
    let strength = p.strength;
    let jitter = p.jitter;
    let dc_mode = u32(to_float(p.dc_mode));
    let dc_scale = p.dc_scale;
    let cell = size / max(1.0e-3, min(m, n));
    let max_step = 0.5 * cell;
    let q = z;
    for i in 0..steps {
        let fd = chladni_field(q, m, n, a, b, size);
        let g = complex(fd.y, fd.z);
        let step = fd.x * g / (g.norm_sqr() + 1.0e-6);
        let sl = step.abs();
        if (sl > max_step) {
            step = step * (max_step / sl);
        }
        q = q - step;
    }
    if (dc_mode != u32(0)) {
        let fd = chladni_field(z, m, n, a, b, size);
        if (dc_mode == u32(1)) {
            let dist = abs(fd.x) / (complex(fd.y, fd.z).abs() + 1.0e-6);
            color = exp(-6.0 * dc_scale * dist / cell);
        } else if (dc_mode == u32(2)) {
            let f_norm = abs(a) + abs(b) + 1.0e-6;
            color = 0.5 + 0.5 * tanh(2.0 * dc_scale * fd.x / f_norm);
        } else {
            let pi = 3.14159265359;
            let sx = z / size * pi;
            let t1 = a * sin(m * sx.re) * sin(n * sx.im);
            let t2 = b * sin(n * sx.re) * sin(m * sx.im);
            let f_norm = abs(a) + abs(b) + 1.0e-6;
            color = 0.5 + 0.5 * tanh(2.0 * dc_scale * (t1 - t2) / f_norm);
        }
    }
    let out = mix(z, q, strength);
    if (jitter > 0.0) {
        out = out + complex(rand() - 0.5, rand() - 0.5) * (2.0 * jitter);
    }
    #{ z: out, color: color }
}

fn variation3(v, p, ctx) {
    let color = ctx.color;
    let m = p.m;
    let n = p.n;
    let a = p.a;
    let b = p.b;
    let size = max(1.0e-6, p.size);
    let steps = to_int(to_float(p.steps));
    let strength = p.strength;
    let jitter = p.jitter;
    let dc_mode = u32(to_float(p.dc_mode));
    let dc_scale = p.dc_scale;
    let cell = size / max(1.0e-3, min(m, n));
    let max_step = 0.5 * cell;
    let q = complex(v.x, v.y);
    for i in 0..steps {
        let fd = chladni_field(q, m, n, a, b, size);
        let g = complex(fd.y, fd.z);
        let step = fd.x * g / (g.norm_sqr() + 1.0e-6);
        let sl = step.abs();
        if (sl > max_step) {
            step = step * (max_step / sl);
        }
        q = q - step;
    }
    if (dc_mode != u32(0)) {
        let fd = chladni_field(complex(v.x, v.y), m, n, a, b, size);
        if (dc_mode == u32(1)) {
            let dist = abs(fd.x) / (complex(fd.y, fd.z).abs() + 1.0e-6);
            color = exp(-6.0 * dc_scale * dist / cell);
        } else if (dc_mode == u32(2)) {
            let f_norm = abs(a) + abs(b) + 1.0e-6;
            color = 0.5 + 0.5 * tanh(2.0 * dc_scale * fd.x / f_norm);
        } else {
            let pi = 3.14159265359;
            let sx = complex(v.x, v.y) / size * pi;
            let t1 = a * sin(m * sx.re) * sin(n * sx.im);
            let t2 = b * sin(n * sx.re) * sin(m * sx.im);
            let f_norm = abs(a) + abs(b) + 1.0e-6;
            color = 0.5 + 0.5 * tanh(2.0 * dc_scale * (t1 - t2) / f_norm);
        }
    }
    let out = mix(complex(v.x, v.y), q, strength);
    if (jitter > 0.0) {
        out = out + complex(rand() - 0.5, rand() - 0.5) * (2.0 * jitter);
    }
    #{ z: vec3(out.re, out.im, v.z), color: color }
}
