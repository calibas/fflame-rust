/// Complex exponential — `e^x · (cos(y), sin(y))`. Stretches the plane
/// exponentially along X while wrapping Y onto a unit-circle phase.
///
/// # Authors
/// - cothe

// Exp, the built-in variation's twin: the complex exponential.

const INFO = #{ title: "Exp", name: "rhai_exp", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let e = exp(z.re);
    complex(e * cos(z.im), e * sin(z.im))
}
