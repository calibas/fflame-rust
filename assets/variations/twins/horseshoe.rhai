/// Bends the plane into a horseshoe — opposite quadrants get folded
/// into the same region of the output. The result has a distinctive
/// U-shaped silhouette.
///
/// # Authors
/// - Scott Draves

// Horseshoe, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Horseshoe", name: "rhai_horseshoe", category: "basic_2d", phase: "any" };

fn variation(z, p) {
    let r = z.abs() + 1.0e-6;
    let r_inv = 1.0 / r;
    complex((z.re - z.im) * (z.re + z.im) * r_inv, 2.0 * z.re * z.im * r_inv)
}
