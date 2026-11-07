/// Variant of ArcSecH with translation by ±i depending on the imaginary
/// sign — produces two parallel arcs instead of one.
/// 
/// # Authors
/// - Tatyana Zabanova
/// - DarkBeam

// ArcSech2, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "ArcSech2", name: "rhai_arcsech2", category: "advanced_2d", phase: "any", z: "preserve_z" };

fn variation(z, p) {
    let scale = 2.0 * 0.6366197723675814;
    let p_mag2 = max(1.0e-20, z.norm_sqr());
    let w_re = z.re / p_mag2;
    let w_im = -z.im / p_mag2;
    let a_re = w_re + 1.0;
    let a_im = w_im;
    let b_re = w_re - 1.0;
    let b_im = w_im;
    let a_r = sqrt(a_re * a_re + a_im * a_im);
    let sa_re = sqrt(0.5 * (a_r + a_re));
    let sa_im_mag = sqrt(0.5 * max(0.0, a_r - a_re));
    let sa_im = (if a_im < 0.0 { -sa_im_mag } else { sa_im_mag });
    let b_r = sqrt(b_re * b_re + b_im * b_im);
    let sb_re = sqrt(0.5 * (b_r + b_re));
    let sb_im_mag = sqrt(0.5 * max(0.0, b_r - b_re));
    let sb_im = (if b_im < 0.0 { -sb_im_mag } else { sb_im_mag });
    let prod_re = sa_re * sb_re - sa_im * sb_im;
    let prod_im = sa_re * sb_im + sa_im * sb_re;
    let arg_re = w_re + prod_re;
    let arg_im = w_im + prod_im;
    let log_re = 0.5 * ln(arg_re * arg_re + arg_im * arg_im + 1.1754944e-38);
    let log_im = atan2(arg_im, arg_re);
    let res_re = scale * log_re;
    let res_im = scale * log_im;
    (if res_im < 0.0 { complex(res_re, res_im + 1.0) } else { complex(-res_re, res_im - 1.0) })
}

fn variation3(v, p) {
    let scale = 2.0 * 0.6366197723675814;
    let p_mag2 = max(1.0e-20, v.x * v.x + v.y * v.y);
    let w_re = v.x / p_mag2;
    let w_im = -v.y / p_mag2;
    let a_re = w_re + 1.0;
    let a_im = w_im;
    let b_re = w_re - 1.0;
    let b_im = w_im;
    let a_r = sqrt(a_re * a_re + a_im * a_im);
    let sa_re = sqrt(0.5 * (a_r + a_re));
    let sa_im_mag = sqrt(0.5 * max(0.0, a_r - a_re));
    let sa_im = (if a_im < 0.0 { -sa_im_mag } else { sa_im_mag });
    let b_r = sqrt(b_re * b_re + b_im * b_im);
    let sb_re = sqrt(0.5 * (b_r + b_re));
    let sb_im_mag = sqrt(0.5 * max(0.0, b_r - b_re));
    let sb_im = (if b_im < 0.0 { -sb_im_mag } else { sb_im_mag });
    let prod_re = sa_re * sb_re - sa_im * sb_im;
    let prod_im = sa_re * sb_im + sa_im * sb_re;
    let arg_re = w_re + prod_re;
    let arg_im = w_im + prod_im;
    let log_re = 0.5 * ln(arg_re * arg_re + arg_im * arg_im + 1.1754944e-38);
    let log_im = atan2(arg_im, arg_re);
    let res_re = scale * log_re;
    let res_im = scale * log_im;
    let xy = (if res_im < 0.0 { complex(res_re, res_im + 1.0) } else { complex(-res_re, res_im - 1.0) });
    vec3(xy.re, xy.im, 0.0)
}
