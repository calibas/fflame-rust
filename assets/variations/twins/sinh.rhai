/// Complex hyperbolic sine, applied to `z·π/4`. Stretches the plane
/// vertically.
///
/// # Authors
/// - cothe

// Sinh, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Sinh", name: "rhai_sinh", category: "advanced_2d", phase: "any", z: "preserve_z" };

fn variation(z, p) {
    let pi_4 = 0.7853981633974483;
    let u = z.re * pi_4;
    let v = z.im * pi_4;
    complex(sinh(u) * cos(v), cosh(u) * sin(v))
}

fn variation3(q, p) {
    let pi_4 = 0.7853981633974483;
    let u = q.x * pi_4;
    let v = q.y * pi_4;
    vec3(sinh(u) * cos(v), cosh(u) * sin(v), 0.0)
}
