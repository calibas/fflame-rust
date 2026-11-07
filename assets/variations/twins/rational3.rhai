/// Degree-3 complex-rational warp — evaluates a complex-valued rational
/// function of `x + i·y`, with degree-3 polynomial numerator (coefficients
/// a, b, c, d) and denominator (e, f, g, h). The output is the complex
/// division `numerator / denominator`.
///
/// # Authors
/// - Xyrus02
/// - CozyG

// Rational 3, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    a: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "A",
        hint: "Numerator coefficient on the `x³ − 3xy²` term.",
    },
    b: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "B",
        hint: "Numerator coefficient on the `x² − y²` term.",
    },
    c: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "C",
        hint: "Numerator linear coefficient on x.",
    },
    d: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "D",
        hint: "Numerator constant term.",
    },
    e: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "E",
        hint: "Denominator coefficient on the `x³ − 3xy²` term.",
    },
    f: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "F",
        hint: "Denominator coefficient on the `x² − y²` term.",
    },
    g: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "G",
        hint: "Denominator linear coefficient on x.",
    },
    h: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "H",
        hint: "Denominator constant term.",
    },
};

const INFO = #{ title: "Rational 3", name: "rhai_rational3", category: "advanced_2d", phase: "any", z: "preserve_z" };

fn variation(z, p) {
    let a = p.a;
    let b = p.b;
    let c = p.c;
    let d = p.d;
    let e = p.e;
    let f = p.f;
    let g = p.g;
    let h = p.h;
    let xs = z.re * z.re;
    let ys = z.im * z.im;
    let xc = xs * z.re;
    let yc = ys * z.im;
    let zt3 = xc - 3.0 * z.re * ys;
    let zt2 = xs - ys;
    let zb3 = 3.0 * xs * z.im - yc;
    let zb2 = 2.0 * z.re * z.im;
    let tr = a * zt3 + b * zt2 + c * z.re + d;
    let ti = a * zb3 + b * zb2 + c * z.im;
    let br = e * zt3 + f * zt2 + g * z.re + h;
    let bi = e * zb3 + f * zb2 + g * z.im;
    let r3den = 1.0 / max(1.0e-30, br * br + bi * bi);
    complex((tr * br + ti * bi) * r3den, (ti * br - tr * bi) * r3den)
}

fn variation3(v, p) {
    let a = p.a;
    let b = p.b;
    let c = p.c;
    let d = p.d;
    let e = p.e;
    let f = p.f;
    let g = p.g;
    let h = p.h;
    let xs = v.x * v.x;
    let ys = v.y * v.y;
    let xc = xs * v.x;
    let yc = ys * v.y;
    let zt3 = xc - 3.0 * v.x * ys;
    let zt2 = xs - ys;
    let zb3 = 3.0 * xs * v.y - yc;
    let zb2 = 2.0 * v.x * v.y;
    let tr = a * zt3 + b * zt2 + c * v.x + d;
    let ti = a * zb3 + b * zb2 + c * v.y;
    let br = e * zt3 + f * zt2 + g * v.x + h;
    let bi = e * zb3 + f * zb2 + g * v.y;
    let r3den = 1.0 / max(1.0e-30, br * br + bi * bi);
    vec3((tr * br + ti * bi) * r3den, (ti * br - tr * bi) * r3den, v.z)
}
