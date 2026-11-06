/// 3D Julia-N with affine pre-transform — extends `julian2` to 3D: same
/// affine pre-transform `(X, Y) = (a·x + b·y + e, c·x + d·y + f)` and
/// random angular branch, plus a `z_local = x / |power|` term that feeds
/// into the radius computation and produces a Z output `radius_out ·
/// z_local`.
///
/// # Authors
/// - Xyrus02

// JuliaN 3D X, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    power: #{
        value: 0, min: -100, max: 100,
        caption: "Power",
        hint: "Number of angular branches. 0 produces zero output.",
    },
    dist: #{
        value: 1.0, min: -100.0, max: 100.0, free: true,
        caption: "Distance",
        hint: "Radial-power exponent — `cN = (dist/power − 1)/2`.",
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

const INFO = #{ title: "JuliaN 3D X", name: "rhai_julian3dx", category: "full_3d", phase: "any" };

fn prepare(p) {
    let safe_power = (if abs(to_float(p.power)) < 1.0e-30 { 1.0 } else { to_float(p.power) });
    #{ d0: abs(safe_power), cN: (p.dist / safe_power - 1.0) * 0.5 }
}

fn variation(z, p, ctx) {
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
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    if (abs(power) < 1.0e-30) {
        return complex(0.0, 0.0);
    }
    let z_local = z.re / absN;
    let radius_square = z.re * z.re + z.im * z.im;
    let radius_out = w * pow(max(1.0e-30, radius_square + z_local * z_local), cN);
    let xx = a * z.re + b * z.im + e;
    let yy = c * z.re + d * z.im + f;
    let alpha = (atan2(yy, xx) + two_pi * to_float(floor(rand() * absN))) / power;
    let gamma = radius_out * sqrt(max(0.0, radius_square));
    complex(gamma * cos(alpha) * inv_w, gamma * sin(alpha) * inv_w)
}

fn variation3(v, p, ctx) {
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
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    if (abs(power) < 1.0e-30) {
        return vec3(0.0, 0.0, v.z);
    }
    let z_local = v.x / absN;
    let radius_square = v.x * v.x + v.y * v.y;
    let radius_out = w * pow(max(1.0e-30, radius_square + z_local * z_local), cN);
    let fz = radius_out * z_local;
    let xx = a * v.x + b * v.y + e;
    let yy = c * v.x + d * v.y + f;
    let alpha = (atan2(yy, xx) + two_pi * to_float(floor(rand() * absN))) / power;
    let gamma = radius_out * sqrt(max(0.0, radius_square));
    vec3(gamma * cos(alpha) * inv_w, gamma * sin(alpha) * inv_w, fz * inv_w)
}
