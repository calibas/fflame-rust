// Sqrt ACosh, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Sqrt ACosh", name: "rhai_sqrt_acosh" };

fn variation(z, p) {
    let two_over_pi = 0.6366197723675814;
    let r0 = sqrt(z.re * z.re + z.im * z.im);
    let w_re = sqrt(0.5 * (r0 + z.re));
    let w_im_mag = sqrt(0.5 * max(r0 - z.re, 0.0));
    let w_im = (if z.im < 0.0 { -w_im_mag } else { w_im_mag });
    let z2_re = w_re * w_re - w_im * w_im - 1.0;
    let z2_im = 2.0 * w_re * w_im;
    let r1 = sqrt(z2_re * z2_re + z2_im * z2_im);
    let s_re = sqrt(0.5 * (r1 + z2_re));
    let s_im_mag = sqrt(0.5 * max(r1 - z2_re, 0.0));
    let s_im = (if z2_im < 0.0 { -s_im_mag } else { s_im_mag });
    let arg_re = w_re + s_re;
    let arg_im = w_im + s_im;
    let log_re = 0.5 * ln(arg_re * arg_re + arg_im * arg_im + 1.1754944e-38);
    let log_im = atan2(arg_im, arg_re);
    let sign_flip = (if rand() < 0.5 { -1.0 } else { 1.0 });
    complex(sign_flip * two_over_pi * log_re, sign_flip * two_over_pi * log_im)
}
