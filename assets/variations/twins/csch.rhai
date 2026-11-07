/// Complex hyperbolic cosecant. Singularities at 0.
///
/// # Authors
/// - cothe

// Csch, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Csch", name: "rhai_csch", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let s = sin(z.im);
    let c = cos(z.im);
    let sh = sinh(z.re);
    let ch = cosh(z.re);
    let d = cosh(2.0 * z.re) - cos(2.0 * z.im);
    let inv = 2.0 / (if d == 0.0 { 1.0e-20 } else { d });
    complex(inv * sh * c, -inv * ch * s)
}
