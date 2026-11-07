/// Apollonian-gasket IFS — randomly picks one of 3 Möbius-style branches
/// per iteration to spawn points along an Apollonian gasket fractal.
///
/// # Authors
/// - Jesus Sosa
/// - Paul Bourke

// Apollony, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Apollony", name: "rhai_apollony", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let r = 1.7320508075688772;
    let denom = (1.0 + r - z.re) * (1.0 + r - z.re) + z.im * z.im + 1.0e-30;
    let a0 = 3.0 * (1.0 + r - z.re) / denom - (1.0 + r) / (2.0 + r);
    let b0 = 3.0 * z.im / denom;
    let mag2 = a0 * a0 + b0 * b0 + 1.0e-30;
    let f1x = a0 / mag2;
    let f1y = -b0 / mag2;
    let w = to_int(4.0 * rand()) % 3;
    if (w == 0) {
        return complex(a0, b0);
    } else if (w == 1) {
        return complex(-f1x * 0.5 - f1y * r * 0.5, f1x * r * 0.5 - f1y * 0.5);
    }
    complex(-f1x * 0.5 + f1y * r * 0.5, -f1x * r * 0.5 - f1y * 0.5)
}
