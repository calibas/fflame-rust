/// Quaternion secant — 3D extension of Sec (1/cos).
///
/// # Authors
/// - zephyrtronium

// Secq, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Secq", name: "rhai_secq", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let v = max(1.0e-20, abs(z.im));
    let norm2 = max(1.0e-20, z.re * z.re + z.im * z.im);
    let ni = 1.0 / norm2;
    let s = sin(-z.re);
    let c = cos(-z.re);
    let sh = sinh(v);
    let ch = cosh(v);
    let coef = ni * s * sh / v;
    complex(c * ch * ni, -coef * z.im)
}

fn variation3(q, p) {
    let v = max(1.0e-20, sqrt(q.y * q.y + q.z * q.z));
    let norm2 = max(1.0e-20, q.x * q.x + q.y * q.y + q.z * q.z);
    let ni = 1.0 / norm2;
    let s = sin(-q.x);
    let c = cos(-q.x);
    let sh = sinh(v);
    let ch = cosh(v);
    let coef = ni * s * sh / v;
    vec3(c * ch * ni, -coef * q.y, -coef * q.z)
}
