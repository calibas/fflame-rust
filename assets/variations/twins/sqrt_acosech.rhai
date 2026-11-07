/// Like ACosecH but on sqrt(z) — denser version of the symmetric two-branch
/// pattern. Random ±sign each iteration.
///  
/// # Authors
/// - Whittaker Courtney

// Sqrt ACosecH, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Sqrt ACosecH", name: "rhai_sqrt_acosech", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let two_over_pi = 0.6366197723675814;
    let r0 = sqrt(z.re * z.re + z.im * z.im);
    let w_re = sqrt(0.5 * (r0 + z.re));
    let w_im_mag = sqrt(0.5 * max(0.0, r0 - z.re));
    let w_im = (if z.im < 0.0 { -w_im_mag } else { w_im_mag });
    let w_mag2 = max(1.0e-20, w_re * w_re + w_im * w_im);
    let u_re = w_re / w_mag2;
    let u_im = -w_im / w_mag2;
    let u2_re = u_re * u_re - u_im * u_im + 1.0;
    let u2_im = 2.0 * u_re * u_im;
    let r1 = sqrt(u2_re * u2_re + u2_im * u2_im);
    let s_re = sqrt(0.5 * (r1 + u2_re));
    let s_im_mag = sqrt(0.5 * max(0.0, r1 - u2_re));
    let s_im = (if u2_im < 0.0 { -s_im_mag } else { s_im_mag });
    let arg_re = u_re + s_re;
    let arg_im = u_im + s_im;
    let log_re = 0.5 * ln(arg_re * arg_re + arg_im * arg_im + 1.1754944e-38);
    let log_im = atan2(arg_im, arg_re);
    let sign_flip = (if rand() < 0.5 { -1.0 } else { 1.0 });
    complex(sign_flip * two_over_pi * log_re, sign_flip * two_over_pi * log_im)
}
