/// Inverse hyperbolic sine on the complex input. Maps the entire plane onto
/// a horizontal strip — acts as a `spreading` transform.
/// 
/// # Authors
/// - Tatyana Zabanova
/// - DarkBeam

// ArcSinh, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "ArcSinh", name: "rhai_arcsinh", category: "advanced_2d", phase: "any", z: "preserve_z" };

fn variation(z, p) {
    let two_over_pi = 0.6366197723675814;
    let z2_re = z.re * z.re - z.im * z.im + 1.0;
    let z2_im = 2.0 * z.re * z.im;
    let r = sqrt(z2_re * z2_re + z2_im * z2_im);
    let s_re = sqrt(0.5 * (r + z2_re));
    let s_im_mag = sqrt(0.5 * max(0.0, r - z2_re));
    let s_im = (if z2_im < 0.0 { -s_im_mag } else { s_im_mag });
    let arg_re = z.re + s_re;
    let arg_im = z.im + s_im;
    let log_re = 0.5 * ln(arg_re * arg_re + arg_im * arg_im + 1.1754944e-38);
    let log_im = atan2(arg_im, arg_re);
    complex(two_over_pi * log_re, two_over_pi * log_im)
}

fn variation3(v, p) {
    let two_over_pi = 0.6366197723675814;
    let z2_re = v.x * v.x - v.y * v.y + 1.0;
    let z2_im = 2.0 * v.x * v.y;
    let r = sqrt(z2_re * z2_re + z2_im * z2_im);
    let s_re = sqrt(0.5 * (r + z2_re));
    let s_im_mag = sqrt(0.5 * max(0.0, r - z2_re));
    let s_im = (if z2_im < 0.0 { -s_im_mag } else { s_im_mag });
    let arg_re = v.x + s_re;
    let arg_im = v.y + s_im;
    let log_re = 0.5 * ln(arg_re * arg_re + arg_im * arg_im + 1.1754944e-38);
    let log_im = atan2(arg_im, arg_re);
    vec3(two_over_pi * log_re, two_over_pi * log_im, 0.0)
}
