/// Quaternion cosecant — 3D extension of Csc (1/sin).
///
/// # Authors
/// - zephyrtronium

// Cscq, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Cscq", name: "rhai_cscq", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let v = max(abs(z.im), 1.0e-20);
    let norm2 = max(z.re * z.re + z.im * z.im, 1.0e-20);
    let ni = 1.0 / norm2;
    let s = sin(z.re);
    let c = cos(z.re);
    let sh = sinh(v);
    let ch = cosh(v);
    let coef = ni * c * sh / v;
    complex(s * ch * ni, -coef * z.im)
}

fn variation3(q, p) {
    let v = max(sqrt(q.y * q.y + q.z * q.z), 1.0e-20);
    let norm2 = max(q.x * q.x + q.y * q.y + q.z * q.z, 1.0e-20);
    let ni = 1.0 / norm2;
    let s = sin(q.x);
    let c = cos(q.x);
    let sh = sinh(v);
    let ch = cosh(v);
    let coef = ni * c * sh / v;
    vec3(s * ch * ni, -coef * q.y, -coef * q.z)
}
