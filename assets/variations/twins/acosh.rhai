/// Inverse hyperbolic cosine on the complex input. Each iteration randomly
/// picks one of the two branches, producing a symmetric upper/lower
/// pattern.
/// 
/// # Authors
/// - Whittaker Courtney

// ACosh, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "ACosh", name: "rhai_acosh", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let two_over_pi = 0.6366197723675814;
    let z2_re = z.re * z.re - z.im * z.im - 1.0;
    let z2_im = 2.0 * z.re * z.im;
    let r = sqrt(z2_re * z2_re + z2_im * z2_im);
    let s_re = sqrt(0.5 * (r + z2_re));
    let s_im_mag = sqrt(0.5 * max(0.0, r - z2_re));
    let s_im = (if z2_im < 0.0 { -s_im_mag } else { s_im_mag });
    let arg_re = z.re + s_re;
    let arg_im = z.im + s_im;
    let log_re = 0.5 * ln(arg_re * arg_re + arg_im * arg_im + 1.1754944e-38);
    let log_im = atan2(arg_im, arg_re);
    let sign_flip = (if rand() < 0.5 { -1.0 } else { 1.0 });
    complex(sign_flip * two_over_pi * log_re, sign_flip * two_over_pi * log_im)
}
