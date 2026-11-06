/// Combines an inverse-radius scaling with sine/cosine of both angle and
/// radius. The result spirals inward in a logarithmic pattern.
///
/// # Authors
/// - Scott Draves

// Spiral, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Spiral", name: "rhai_spiral", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let r = z.abs() + 1.0e-6;
    let theta = atan2(z.re, z.im);
    let r_inv = 1.0 / r;
    complex(r_inv * (cos(theta) + sin(r)), r_inv * (sin(theta) - cos(r)))
}
