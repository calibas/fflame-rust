/// Complex exponential — `e^x · (cos(y), sin(y))`. Stretches the plane
/// exponentially along X while wrapping Y onto a unit-circle phase.
///
/// # Authors
/// - cothe

// Exp, the built-in variation's twin: the complex exponential.

const INFO = #{ title: "Exp", name: "rhai_exp", category: "advanced_2d", phase: "any", inverse_branches: 3 };

fn variation(z, p) {
    let e = exp(z.re);
    complex(e * cos(z.im), e * sin(z.im))
}

fn inverse(z, p, branch) {
    let m = z.abs();
    if m <= 0.0 {
        return no_preimage();
    }
    complex(ln(m), atan2(z.im, z.re) + 6.28318530717959 * (to_float(branch) - 1.0))
}
