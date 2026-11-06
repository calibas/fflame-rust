/// JuliaN with affine pre-transform — applies a 2D affine `(X, Y) = (a·x +
/// b·y + e, c·x + d·y + f)` first, then runs standard JuliaN: picks a
/// random angular branch from `|power|`, computes the new angle `(atan2(Y,
/// X) + 2π·k)/power`, and emits at radius `(X²+Y²)^(dist/(2·power))`.
///
/// # Authors
/// - Xyrus02

// JuliaN 2, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    power: #{
        value: 0, min: -100, max: 100,
        caption: "Power",
        hint: "Number of angular branches. 0 produces zero output.",
    },
    dist: #{
        value: 1.0, min: -100.0, max: 100.0, free: true,
        caption: "Distance",
        hint: "Radial-power exponent — output radius is `(X²+Y²)^(dist/(2·power))`.",
    },
    a: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "A",
        hint: "Pre-affine xx coefficient.",
    },
    b: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "B",
        hint: "Pre-affine xy coefficient.",
    },
    c: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "C",
        hint: "Pre-affine yx coefficient.",
    },
    d: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "D",
        hint: "Pre-affine yy coefficient.",
    },
    e: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "E",
        hint: "Pre-affine x offset.",
    },
    f: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "F",
        hint: "Pre-affine y offset.",
    },
};

const INFO = #{ title: "JuliaN 2", name: "rhai_julian2", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let safe_power = (if abs(to_float(p.power)) < 1.0e-30 { 1.0 } else { to_float(p.power) });
    #{ d0: abs(safe_power), cN: p.dist / (2.0 * safe_power) }
}

fn variation(z, p) {
    let power = to_float(p.power);
    let a = p.a;
    let b = p.b;
    let c = p.c;
    let d = p.d;
    let e = p.e;
    let f = p.f;
    let absN = max(1.0, p.d0);
    let cN = p.cN;
    let two_pi = 6.28318530717959;
    if (abs(power) < 1.0e-30) {
        return complex(0.0, 0.0);
    }
    let x = a * z.re + b * z.im + e;
    let y = c * z.re + d * z.im + f;
    let angle = (atan2(y, x) + two_pi * to_float(floor(rand() * absN))) / power;
    let r = pow(max(1.0e-30, x * x + y * y), cN);
    complex(r * cos(angle), r * sin(angle))
}
