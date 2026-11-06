/// Quaternion hyperbolic secant — 3D extension of Sech.
///
/// # Authors
/// - zephyrtronium

// Sechq, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Sechq", name: "rhai_sechq", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let v = max(1.0e-20, abs(z.im));
    let norm2 = max(1.0e-20, z.re * z.re + z.im * z.im);
    let ni = 1.0 / norm2;
    let s = sin(v);
    let c = cos(v);
    let sh = sinh(z.re);
    let ch = cosh(z.re);
    let coef = ni * sh * s / v;
    complex(ch * c * ni, -coef * z.im)
}

fn variation3(q, p) {
    let v = max(1.0e-20, sqrt(q.y * q.y + q.z * q.z));
    let norm2 = max(1.0e-20, q.x * q.x + q.y * q.y + q.z * q.z);
    let ni = 1.0 / norm2;
    let s = sin(v);
    let c = cos(v);
    let sh = sinh(q.x);
    let ch = cosh(q.x);
    let coef = ni * sh * s / v;
    vec3(ch * c * ni, -coef * q.y, -coef * q.z)
}
