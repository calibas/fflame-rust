/// Quaternion sine — extends the complex Sin to 3D by treating (x, y, z) as
/// a split quaternion. In 2D mode (z = 0) it collapses to the same shape as
/// Sin.
///
/// # Authors
/// - zephyrtronium

// Sinq, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Sinq", name: "rhai_sinq", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let v = max(abs(z.im), 1.0e-20);
    let s = sin(z.re);
    let c = cos(z.re);
    let sh = sinh(v);
    let ch = cosh(v);
    let coef = c * sh / v;
    complex(s * ch, coef * z.im)
}

fn variation3(q, p) {
    let v = max(sqrt(q.y * q.y + q.z * q.z), 1.0e-20);
    let s = sin(q.x);
    let c = cos(q.x);
    let sh = sinh(v);
    let ch = cosh(v);
    let coef = c * sh / v;
    vec3(s * ch, coef * q.y, coef * q.z)
}
