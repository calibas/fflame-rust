/// Complex secant (1/cos). Singularities at ±π/2 create high-density rings
/// around the poles.
///
/// # Authors
/// - cothe

// Sec, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Sec", name: "rhai_sec", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let s = sin(z.re);
    let c = cos(z.re);
    let sh = sinh(z.im);
    let ch = cosh(z.im);
    let d = cos(2.0 * z.re) + cosh(2.0 * z.im);
    let inv = 2.0 / (if d == 0.0 { 1.0e-20 } else { d });
    complex(inv * c * ch, inv * s * sh)
}
