/// Disc-style warp with 8 tunable knobs — computes a per-axis-weighted
/// radial argument `rPI = π · sqrt(x²·d·e + y²·f·g)`, then emits
/// `(sin(rPI)·a, cos(rPI)·b)` scaled by `atan2(y,x)/π · c · h`. The 8
/// parameters give independent control over the X/Y radial weights, the
/// sin/cos amplitudes, and the overall output scale.
///
/// # Authors
/// - Brad Stefanov

// Disc 3, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    a: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "A",
        hint: "Sin amplitude scale on the X output.",
    },
    b: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "B",
        hint: "Cos amplitude scale on the Y output.",
    },
    c: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "C",
        hint: "Angular-term scale (multiplier on `atan2(y,x)/π`).",
    },
    d: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "D",
        hint: "X-axis radial-weight first factor (multiplied with `e` for the x² term).",
    },
    e: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "E",
        hint: "X-axis radial-weight second factor.",
    },
    f: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "F",
        hint: "Y-axis radial-weight first factor (multiplied with `g` for the y² term).",
    },
    g: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "G",
        hint: "Y-axis radial-weight second factor.",
    },
    h: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "H",
        hint: "Overall output scale (multiplied into both X and Y).",
    },
};

const INFO = #{ title: "Disc 3", name: "rhai_disc3", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let a = p.a;
    let b = p.b;
    let c = p.c;
    let d = p.d;
    let e = p.e;
    let f = p.f;
    let g = p.g;
    let h = p.h;
    let pi = 3.14159265358979;
    let inv_pi = 0.3183098861837907;
    let r_pi = pi * sqrt(max(z.re * d * z.re * e + z.im * f * z.im * g, 0.0));
    let sinr = sin(r_pi) * a;
    let cosr = cos(r_pi) * b;
    let r = atan2(z.im, z.re) * inv_pi * c;
    complex(sinr * h * r, cosr * h * r)
}
