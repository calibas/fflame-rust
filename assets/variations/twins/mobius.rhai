/// Möbius transformation in the complex plane — `(Az + B) / (Cz + D)`. The
/// eight real/imaginary coefficients (A, B, C, D) control the conformal
/// warping; classic hyperbolic-geometry effect.
/// 
/// # Authors
/// - eralex61

// Mobius, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    re_a: #{
        value: 0.1, min: -1.0, max: 1.0, free: true,
        caption: "Re A",
        hint: "Real component of complex coefficient A in `(Az + B)/(Cz + D)`.",
    },
    im_a: #{
        value: 0.2, min: -1.0, max: 1.0, free: true,
        caption: "Im A",
        hint: "Imaginary component of complex coefficient A.",
    },
    re_b: #{
        value: 0.2, min: -1.0, max: 1.0, free: true,
        caption: "Re B",
        hint: "Real component of complex coefficient B.",
    },
    im_b: #{
        value: -0.12, min: -1.0, max: 1.0, free: true,
        caption: "Im B",
        hint: "Imaginary component of complex coefficient B.",
    },
    re_c: #{
        value: -0.15, min: -1.0, max: 1.0, free: true,
        caption: "Re C",
        hint: "Real component of complex coefficient C.",
    },
    im_c: #{
        value: -0.15, min: -1.0, max: 1.0, free: true,
        caption: "Im C",
        hint: "Imaginary component of complex coefficient C.",
    },
    re_d: #{
        value: 0.21, min: -1.0, max: 1.0, free: true,
        caption: "Re D",
        hint: "Real component of complex coefficient D.",
    },
    im_d: #{
        value: 0.1, min: -1.0, max: 1.0, free: true,
        caption: "Im D",
        hint: "Imaginary component of complex coefficient D.",
    },
};

const INFO = #{ title: "Mobius", name: "rhai_mobius", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let re_a = p.re_a;
    let im_a = p.im_a;
    let re_b = p.re_b;
    let im_b = p.im_b;
    let re_c = p.re_c;
    let im_c = p.im_c;
    let re_d = p.re_d;
    let im_d = p.im_d;
    let re_u = re_a * z.re - im_a * z.im + re_b;
    let im_u = re_a * z.im + im_a * z.re + im_b;
    let re_v = re_c * z.re - im_c * z.im + re_d;
    let im_v = re_c * z.im + im_c * z.re + im_d;
    let v_denom = re_v * re_v + im_v * im_v + 1.0e-10;
    complex((re_u * re_v + im_u * im_v) / v_denom, (im_u * re_v - re_u * im_v) / v_denom)
}
