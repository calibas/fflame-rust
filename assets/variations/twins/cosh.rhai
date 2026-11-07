/// Complex hyperbolic cosine. Sister function to Sinh — even symmetry
/// instead of odd.
///
/// # Authors
/// - cothe

// Cosh, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Cosh", name: "rhai_cosh", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let s = sin(z.im);
    let c = cos(z.im);
    let sh = sinh(z.re);
    let ch = cosh(z.re);
    complex(ch * c, sh * s)
}
