/// Linear-fractional projective transform — applies a 2D homography `out =
/// ((A1·x + B1·y + C1) / U, (A2·x + B2·y + C2) / U)` where `U = A·x + B·y +
/// C`. With identity coefficients reduces to the input; arbitrary
/// coefficients implement any 2D projective warp.
///
/// # Authors
/// - eralex61

// Projective, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    a: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "A",
        hint: "Denominator X coefficient.",
    },
    b: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "B",
        hint: "Denominator Y coefficient.",
    },
    c: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "C",
        hint: "Denominator constant term.",
    },
    a1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "A1",
        hint: "Numerator X coefficient for X output.",
    },
    b1: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "B1",
        hint: "Numerator Y coefficient for X output.",
    },
    c1: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "C1",
        hint: "Numerator constant term for X output.",
    },
    a2: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "A2",
        hint: "Numerator X coefficient for Y output.",
    },
    b2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "B2",
        hint: "Numerator Y coefficient for Y output.",
    },
    c2: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "C2",
        hint: "Numerator constant term for Y output.",
    },
};

const INFO = #{ title: "Projective", name: "rhai_projective", category: "advanced_2d", phase: "any", z: "preserve_z" };

fn variation(z, p) {
    let a = p.a;
    let b = p.b;
    let c = p.c;
    let a1 = p.a1;
    let b1 = p.b1;
    let c1 = p.c1;
    let a2 = p.a2;
    let b2 = p.b2;
    let c2 = p.c2;
    let u = a * z.re + b * z.im + c;
    let inv_u = 1.0 / (if abs(u) < 1.0e-30 { 1.0e-30 } else { u });
    complex((a1 * z.re + b1 * z.im + c1) * inv_u, (a2 * z.re + b2 * z.im + c2) * inv_u)
}

fn variation3(v, p) {
    let a = p.a;
    let b = p.b;
    let c = p.c;
    let a1 = p.a1;
    let b1 = p.b1;
    let c1 = p.c1;
    let a2 = p.a2;
    let b2 = p.b2;
    let c2 = p.c2;
    let u = a * v.x + b * v.y + c;
    let inv_u = 1.0 / (if abs(u) < 1.0e-30 { 1.0e-30 } else { u });
    vec3((a1 * v.x + b1 * v.y + c1) * inv_u, (a2 * v.x + b2 * v.y + c2) * inv_u, v.z)
}
