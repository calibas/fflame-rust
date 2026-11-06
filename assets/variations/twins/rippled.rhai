/// Tanh/cos ripple — emits `(tanh(x²+y²) · x, cos(x²+y²) · y)`. Soft
/// saturation along X (via tanh) combined with cosine modulation along Y
/// produces a ripple/wave pattern.
///
/// # Authors
/// - Raykoid666

// Rippled, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Rippled", name: "rhai_rippled", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let d = z.re * z.re + z.im * z.im + 1.0e-30;
    complex(tanh(d) * z.re, cos(d) * z.im)
}
