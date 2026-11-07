/// Like ArcTanh but on sqrt(z) — a denser, more compressed variant of the
/// unit-disc-to-plane mapping. Random ±sign each iteration.
///  
/// # Authors
/// - Whittaker Courtney

// Sqrt ATanh, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Sqrt ATanh", name: "rhai_sqrt_atanh", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let one_over_pi = 0.3183098861837907;
    let r0 = sqrt(z.re * z.re + z.im * z.im);
    let w_re = sqrt(0.5 * (r0 + z.re));
    let w_im_mag = sqrt(0.5 * max(0.0, r0 - z.re));
    let w_im = (if z.im < 0.0 { -w_im_mag } else { w_im_mag });
    let denom_mag2 = max(1.0e-20, (1.0 - w_re) * (1.0 - w_re) + w_im * w_im);
    let ratio_re = ((1.0 + w_re) * (1.0 - w_re) - w_im * w_im) / denom_mag2;
    let ratio_im = (w_im * (1.0 - w_re) + w_im * (1.0 + w_re)) / denom_mag2;
    let log_re = 0.5 * ln(ratio_re * ratio_re + ratio_im * ratio_im + 1.1754944e-38);
    let log_im = atan2(ratio_im, ratio_re);
    let sign_flip = (if rand() < 0.5 { -1.0 } else { 1.0 });
    complex(sign_flip * one_over_pi * 0.5 * log_re, sign_flip * one_over_pi * 0.5 * log_im)
}
