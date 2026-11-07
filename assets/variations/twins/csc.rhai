/// Complex cosecant (1/sin). Singularities at 0 and ±π.
///
/// # Authors
/// - cothe

// Csc, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Csc", name: "rhai_csc", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let s = sin(z.re);
    let c = cos(z.re);
    let sh = sinh(z.im);
    let ch = cosh(z.im);
    let d = cosh(2.0 * z.im) - cos(2.0 * z.re);
    let inv = 2.0 / (if d == 0.0 { 1.0e-20 } else { d });
    complex(inv * s * ch, -inv * c * sh)
}
