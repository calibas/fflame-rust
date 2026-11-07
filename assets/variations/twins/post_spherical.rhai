/// Post-phase spherical inversion — applies `r = w / (x² + y² + ε)` then
/// `(x·r, y·r)` to the accumulated output. Scott Draves's classic
/// `spherical` variation, applied in post-phase rather than as a normal
/// variation.
///
/// # Authors
/// - Scott Draves

// Post Spherical, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Post Spherical", name: "rhai_post_spherical", category: "advanced_2d", phase: "post" };

fn variation(z, p, ctx) {
    let w = ctx.weight;
    let r = w / (z.re * z.re + z.im * z.im + 1.0e-30);
    complex(z.re * r, z.im * r)
}
