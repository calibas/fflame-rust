/// Complex hyperbolic cotangent. Singularities at 0.
///
/// # Authors
/// - cothe

// Coth, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Coth", name: "rhai_coth", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let s2 = sin(2.0 * z.im);
    let c2 = cos(2.0 * z.im);
    let sh2 = sinh(2.0 * z.re);
    let ch2 = cosh(2.0 * z.re);
    let d = ch2 - c2;
    let inv = 1.0 / (if d == 0.0 { 1.0e-20 } else { d });
    complex(inv * sh2, inv * s2)
}
