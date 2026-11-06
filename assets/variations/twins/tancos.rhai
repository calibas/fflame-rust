/// Tan/cos blend — X gets scaled by tanh of the squared radius, Y by cos of
/// the squared radius. Produces wavy concentric ring patterns.
///
/// # Authors
/// - Raykoid666

// TanCos, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "TanCos", name: "rhai_tancos", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let d1 = 1.0e-30 + z.re * z.re + z.im * z.im;
    let inv = 1.0 / d1;
    complex(inv * tanh(d1) * 2.0 * z.re, inv * cos(d1) * 2.0 * z.im)
}
