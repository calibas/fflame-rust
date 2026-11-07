/// Inverse hyperbolic cosecant on the complex input (arcsinh of 1/z), then
/// swaps the real and imaginary parts. Random branch selection per
/// iteration produces symmetric two-branch patterns.
/// 
/// # Authors
/// - Whittaker Courtney

// ACosecH, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "ACosecH", name: "rhai_acosech", category: "advanced_2d", phase: "any", z: "preserve_z" };

fn variation(z, p) {
    let two_over_pi = 0.6366197723675814;
    let p_mag2 = max(1.0e-20, z.norm_sqr());
    let w_re = z.re / p_mag2;
    let w_im = -z.im / p_mag2;
    let w2_re = w_re * w_re - w_im * w_im + 1.0;
    let w2_im = 2.0 * w_re * w_im;
    let r = sqrt(w2_re * w2_re + w2_im * w2_im);
    let s_re = sqrt(0.5 * (r + w2_re));
    let s_im_mag = sqrt(0.5 * max(0.0, r - w2_re));
    let s_im = (if w2_im < 0.0 { -s_im_mag } else { s_im_mag });
    let arg_re = w_re + s_re;
    let arg_im = w_im + s_im;
    let log_re = 0.5 * ln(arg_re * arg_re + arg_im * arg_im + 1.1754944e-38);
    let log_im = atan2(arg_im, arg_re);
    let sign_flip = (if rand() < 0.5 { -1.0 } else { 1.0 });
    complex(sign_flip * two_over_pi * log_im, sign_flip * two_over_pi * log_re)
}

fn variation3(v, p) {
    let two_over_pi = 0.6366197723675814;
    let p_mag2 = max(1.0e-20, v.x * v.x + v.y * v.y);
    let w_re = v.x / p_mag2;
    let w_im = -v.y / p_mag2;
    let w2_re = w_re * w_re - w_im * w_im + 1.0;
    let w2_im = 2.0 * w_re * w_im;
    let r = sqrt(w2_re * w2_re + w2_im * w2_im);
    let s_re = sqrt(0.5 * (r + w2_re));
    let s_im_mag = sqrt(0.5 * max(0.0, r - w2_re));
    let s_im = (if w2_im < 0.0 { -s_im_mag } else { s_im_mag });
    let arg_re = w_re + s_re;
    let arg_im = w_im + s_im;
    let log_re = 0.5 * ln(arg_re * arg_re + arg_im * arg_im + 1.1754944e-38);
    let log_im = atan2(arg_im, arg_re);
    let sign_flip = (if rand() < 0.5 { -1.0 } else { 1.0 });
    vec3(sign_flip * two_over_pi * log_im, sign_flip * two_over_pi * log_re, v.z)
}
