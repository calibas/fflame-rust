/// Quadratic-conic mask — evaluates `r = a·x² + b·xy + c·y² + d·x + e·y +
/// f` (a general 2D quadratic) and either keeps or discards the input based
/// on whether `r` is positive or negative (controlled by `mode`). The
/// boundary `r = 0` is a conic section — an ellipse, parabola, or
/// hyperbola, depending on coefficients.
///
/// # Authors
/// - zephyrtronium

// Chunk, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    a: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "A",
        hint: "x² coefficient.",
    },
    b: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "B",
        hint: "x·y coefficient.",
    },
    c: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "C",
        hint: "y² coefficient.",
    },
    d: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "D",
        hint: "x linear coefficient.",
    },
    e: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "E",
        hint: "y linear coefficient.",
    },
    f: #{
        value: -1.0, min: -10.0, max: 10.0, free: true,
        caption: "F",
        hint: "Constant offset.",
    },
    mode: #{
        value: false,
        caption: "Mode",
        hint: "When on, keep points where `r > 0`. When off, keep where `r ≤ 0`.",
    },
};

const INFO = #{ title: "Chunk", name: "rhai_chunk", category: "advanced_2d", phase: "any", z: "preserve_z" };

fn variation(z, p, ctx) {
    let a = p.a;
    let b = p.b;
    let c = p.c;
    let d = p.d;
    let e = p.e;
    let f = p.f;
    let mode = to_float(p.mode);
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let r = w * (a * z.re * z.re + b * z.re * z.im + c * z.im * z.im + d * z.re + e * z.im + f);
    let keep = false;
    if (mode < 0.5) {
        keep = r <= 0.0;
    } else {
        keep = r > 0.0;
    }
    if (keep) {
        return complex(z.re * inv_w, z.im * inv_w);
    }
    complex(0.0, 0.0)
}

fn variation3(v, p, ctx) {
    let a = p.a;
    let b = p.b;
    let c = p.c;
    let d = p.d;
    let e = p.e;
    let f = p.f;
    let mode = to_float(p.mode);
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let r = w * (a * v.x * v.x + b * v.x * v.y + c * v.y * v.y + d * v.x + e * v.y + f);
    let keep = false;
    if (mode < 0.5) {
        keep = r <= 0.0;
    } else {
        keep = r > 0.0;
    }
    if (keep) {
        return vec3(v.x * inv_w, v.y * inv_w, v.z);
    }
    vec3(0.0, 0.0, 0.0)
}
