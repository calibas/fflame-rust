/// Bends the plane into a horseshoe — opposite quadrants get folded
/// into the same region of the output. The result has a distinctive
/// U-shaped silhouette.
///
/// # Authors
/// - Scott Draves

// Horseshoe, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Horseshoe", name: "rhai_horseshoe", category: "basic_2d", phase: "any", inverse_branches: 2 };

fn variation(z, p) {
    let r = z.abs() + 1.0e-6;
    let r_inv = 1.0 / r;
    complex((z.re - z.im) * (z.re + z.im) * r_inv, 2.0 * z.re * z.im * r_inv)
}

fn inverse(z, p, branch) {
    let s = z.abs();
    let r = (s + sqrt(s * s + 4.0e-6 * s)) / 2.0;
    let th = atan2(z.im, z.re) / 2.0 + (if branch == 0 { 0.0 } else { 3.14159265358979 });
    complex(r * cos(th), r * sin(th))
}
