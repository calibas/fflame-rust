/// Inverts X by the squared radius while leaving Y alone. Stretches things
/// horizontally near the origin and squashes them outside.
///
/// # Authors
/// - Scott Draves

// Hyperbolic, the built-in variation's twin: x over the squared radius, y kept.

const INFO = #{ title: "Hyperbolic", name: "rhai_hyperbolic", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let r2 = z.norm_sqr() + 1e-6;
    complex(z.re / r2, z.im)
}
