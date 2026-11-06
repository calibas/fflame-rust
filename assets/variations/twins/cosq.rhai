/// Quaternion cosine — 3D extension of Cos. In 2D it collapses to the same
/// shape as Cos.
///
/// # Authors
/// - zephyrtronium

// Cosq, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Cosq", name: "rhai_cosq", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let v = max(abs(z.im), 1.0e-20);
    let s = sin(z.re);
    let c = cos(z.re);
    let sh = sinh(v);
    let ch = cosh(v);
    let coef = -s * sh / v;
    complex(c * ch, coef * z.im)
}

fn variation3(q, p) {
    let v = max(sqrt(q.y * q.y + q.z * q.z), 1.0e-20);
    let s = sin(q.x);
    let c = cos(q.x);
    let sh = sinh(v);
    let ch = cosh(v);
    let coef = -s * sh / v;
    vec3(c * ch, coef * q.y, coef * q.z)
}
