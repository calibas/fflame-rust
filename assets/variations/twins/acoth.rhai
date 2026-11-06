/// Treats the input as a complex number and applies the inverse hyperbolic
/// cotangent, then swaps the real and imaginary parts. Creates two
/// singularity points at (±1, 0) with the pattern flowing between them.
/// 
/// # Authors
/// - Whittaker Courtney

// ACoth, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "ACoth", name: "rhai_acoth", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let two_over_pi = 0.6366197723675814;
    let denom_mag2 = max((z.re - 1.0) * (z.re - 1.0) + z.im * z.im, 1.0e-20);
    let ratio_re = ((z.re + 1.0) * (z.re - 1.0) + z.im * z.im) / denom_mag2;
    let ratio_im = -2.0 * z.im / denom_mag2;
    let log_re = 0.25 * ln(ratio_re * ratio_re + ratio_im * ratio_im + 1.1754944e-38);
    let log_im = 0.5 * atan2(ratio_im, ratio_re);
    complex(two_over_pi * log_im, two_over_pi * log_re)
}
