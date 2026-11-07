/// Inverse hyperbolic tangent on the complex input. Maps the unit disc onto
/// the entire plane; everything inside |z|=1 expands outward, everything
/// outside compresses inward.
/// 
/// # Authors
/// - Tatyana Zabanova
/// - DarkBeam

// ArcTanh, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "ArcTanh", name: "rhai_arctanh", category: "advanced_2d", phase: "any", z: "preserve_z" };

fn variation(z, p) {
    let one_over_pi = 0.3183098861837907;
    let denom_mag2 = max(1.0e-20, (1.0 - z.re) * (1.0 - z.re) + z.im * z.im);
    let ratio_re = ((1.0 + z.re) * (1.0 - z.re) - z.im * z.im) / denom_mag2;
    let ratio_im = (z.im * (1.0 - z.re) + z.im * (1.0 + z.re)) / denom_mag2;
    let log_re = 0.5 * ln(ratio_re * ratio_re + ratio_im * ratio_im + 1.1754944e-38);
    let log_im = atan2(ratio_im, ratio_re);
    complex(one_over_pi * 0.5 * log_re, one_over_pi * 0.5 * log_im)
}

fn variation3(v, p) {
    let one_over_pi = 0.3183098861837907;
    let denom_mag2 = max(1.0e-20, (1.0 - v.x) * (1.0 - v.x) + v.y * v.y);
    let ratio_re = ((1.0 + v.x) * (1.0 - v.x) - v.y * v.y) / denom_mag2;
    let ratio_im = (v.y * (1.0 - v.x) + v.y * (1.0 + v.x)) / denom_mag2;
    let log_re = 0.5 * ln(ratio_re * ratio_re + ratio_im * ratio_im + 1.1754944e-38);
    let log_im = atan2(ratio_im, ratio_re);
    vec3(one_over_pi * 0.5 * log_re, one_over_pi * 0.5 * log_im, 0.0)
}
