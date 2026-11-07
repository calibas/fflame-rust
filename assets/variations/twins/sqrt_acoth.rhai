/// Like ACoth but applied to sqrt(z) instead of z. The square-root pre-step
/// roughly halves the angle, producing a denser, more intricate version of
/// ACoth's two-singularity pattern. Random ±sign each iteration.
/// 
/// # Authors
/// - Whittaker Courtney

// Sqrt ACoth, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Sqrt ACoth", name: "rhai_sqrt_acoth", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let two_over_pi = 0.6366197723675814;
    let r = sqrt(z.re * z.re + z.im * z.im);
    let w_re = sqrt(0.5 * (r + z.re));
    let w_im_mag = sqrt(0.5 * max(0.0, r - z.re));
    let w_im = (if z.im < 0.0 { -w_im_mag } else { w_im_mag });
    let denom_mag2 = max(1.0e-20, (w_re - 1.0) * (w_re - 1.0) + w_im * w_im);
    let ratio_re = ((w_re + 1.0) * (w_re - 1.0) + w_im * w_im) / denom_mag2;
    let ratio_im = -2.0 * w_im / denom_mag2;
    let log_re = 0.25 * ln(ratio_re * ratio_re + ratio_im * ratio_im + 1.1754944e-38);
    let log_im = 0.5 * atan2(ratio_im, ratio_re);
    let sign_flip = (if rand() < 0.5 { -1.0 } else { 1.0 });
    complex(sign_flip * two_over_pi * log_re, sign_flip * two_over_pi * log_im)
}
