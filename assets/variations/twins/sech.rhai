/// Complex hyperbolic secant (note: ported with a JWildfire formula quirk
/// that makes it equivalent to a sign-flipped csch — preserved so existing
/// flames render the same).
///
/// # Authors
/// - cothe

// Sech, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Sech", name: "rhai_sech", category: "advanced_2d", phase: "any", z: "preserve_z" };

fn variation(z, p) {
    let pi_4 = 0.7853981633974483;
    let u = z.re * pi_4;
    let v = z.im * pi_4;
    let a = sinh(u) * cos(v);
    let b = cosh(u) * sin(v);
    let d = a * a + b * b;
    let inv = 1.0 / (if d == 0.0 { 1.0e-20 } else { d });
    complex(a * inv, -b * inv)
}

fn variation3(q, p) {
    let pi_4 = 0.7853981633974483;
    let u = q.x * pi_4;
    let v = q.y * pi_4;
    let a = sinh(u) * cos(v);
    let b = cosh(u) * sin(v);
    let d = a * a + b * b;
    let inv = 1.0 / (if d == 0.0 { 1.0e-20 } else { d });
    vec3(a * inv, -b * inv, 0.0)
}
