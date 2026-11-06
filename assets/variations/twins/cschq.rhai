/// Quaternion hyperbolic cosecant — 3D extension of Csch.
///
/// # Authors
/// - zephyrtronium

// Cschq, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Cschq", name: "rhai_cschq", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let v = max(abs(z.im), 1.0e-20);
    let norm2 = max(z.re * z.re + z.im * z.im, 1.0e-20);
    let ni = 1.0 / norm2;
    let s = sin(v);
    let c = cos(v);
    let sh = sinh(z.re);
    let ch = cosh(z.re);
    let coef = ni * ch * s / v;
    complex(sh * c * ni, -coef * z.im)
}

fn variation3(q, p) {
    let v = max(sqrt(q.y * q.y + q.z * q.z), 1.0e-20);
    let norm2 = max(q.x * q.x + q.y * q.y + q.z * q.z, 1.0e-20);
    let ni = 1.0 / norm2;
    let s = sin(v);
    let c = cos(v);
    let sh = sinh(q.x);
    let ch = cosh(q.x);
    let coef = ni * ch * s / v;
    vec3(sh * c * ni, -coef * q.y, -coef * q.z)
}
