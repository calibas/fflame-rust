/// Inverts X by the squared radius while leaving Y alone. Stretches things
/// horizontally near the origin and squashes them outside.
///
/// # Authors
/// - Scott Draves

// Hyperbolic, the built-in variation's twin: x over the squared radius, y kept.

const INFO = #{ title: "Hyperbolic", name: "rhai_hyperbolic", category: "advanced_2d", phase: "any", inverse_branches: 2 };

fn variation(z, p) {
    let r2 = z.norm_sqr() + 1e-6;
    complex(z.re / r2, z.im)
}

fn inverse(z, p, branch) {
    let u = z.re;
    let k = z.im * z.im + 1.0e-6;
    let d = 1.0 - 4.0 * u * u * k;
    if d < 0.0 || (branch == 1 && u == 0.0) {
        return no_preimage();
    }
    let s = 1.0 + sqrt(d);
    let x = if branch == 0 { 2.0 * u * k / s } else { s / (2.0 * u) };
    complex(x, z.im)
}

fn image_gap(z, p) {
    let a = abs(z.re);
    let b = abs(z.im);
    2.0 * (a * b - 0.5) / (a + b + sqrt((a - b) * (a - b) + 2.0))
}
