/// Divides each coordinate by the absolute difference of squared
/// coordinates. Produces a sharp diagonal cross pattern.
///
/// # Authors
/// - Scott Draves

// Cross, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Cross", name: "rhai_cross", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let r = abs((z.re - z.im) * (z.re + z.im) + 1.0e-6);
    if (r < 0.0) {
        r = r * -1.0;
    }
    r = 1.0 / r;
    complex(z.re * r, z.im * r)
}
