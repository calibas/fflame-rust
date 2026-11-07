/// Quaternion exponential extension — emits `(e^x · cos|v|, e^x ·
/// sin|v|/|v| · y, e^x · sin|v|/|v| · z)` where `|v| = sqrt(y² + z²)`.
/// Generalizes the complex exponential `(x + iy → e^x · (cos y, sin y))` to
/// a 3D quaternion-like form.
///
/// # Authors
/// - zephyrtronium

// Estiq, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Estiq", name: "rhai_estiq", category: "full_3d", phase: "any" };

fn variation(z, p) {
    let e = exp(z.re);
    let abs_v = max(1.0e-30, abs(z.im));
    let s = sin(abs_v);
    let c = cos(abs_v);
    let a = e * s / abs_v;
    complex(e * c, a * z.im)
}

fn variation3(v, p) {
    let e = exp(v.x);
    let abs_v = max(1.0e-30, sqrt(v.y * v.y + v.z * v.z));
    let s = sin(abs_v);
    let c = cos(abs_v);
    let a = e * s / abs_v;
    vec3(e * c, a * v.y, a * v.z)
}
