// Hyperbolic, the built-in variation's twin: x over the squared radius, y kept.

const INFO = #{ title: "Hyperbolic", name: "rhai_hyperbolic" };

fn variation(z, p) {
    let r2 = z.norm_sqr() + 1e-6;
    complex(z.re / r2, z.im)
}
