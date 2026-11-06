/// N-power Möbius — transforms into `z^power` space, applies a 2×2 complex
/// Möbius transformation `(Az + B)/(Cz + D)`, then transforms back via a
/// random branch of the N-th root.
///
/// # Authors
/// - eralex61

// MobiusN, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    re_a: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Re A",
        hint: "Real part of complex coefficient A (numerator multiplier).",
    },
    re_b: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Re B",
        hint: "Real part of complex coefficient B (numerator offset).",
    },
    re_c: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Re C",
        hint: "Real part of complex coefficient C (denominator multiplier).",
    },
    re_d: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Re D",
        hint: "Real part of complex coefficient D (denominator offset).",
    },
    im_a: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Im A",
        hint: "Imaginary part of complex coefficient A (numerator multiplier).",
    },
    im_b: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Im B",
        hint: "Imaginary part of complex coefficient B (numerator offset).",
    },
    im_c: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Im C",
        hint: "Imaginary part of complex coefficient C (denominator multiplier).",
    },
    im_d: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Im D",
        hint: "Imaginary part of complex coefficient D (denominator offset).",
    },
    power: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Power",
        hint: "Exponent for the `z^power` transform that wraps the Möbius operation. Higher values create more arms in the output.",
    },
    dist: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Distance",
        hint: "Scales the radial component of the wrapping transform.",
    },
};

const INFO = #{ title: "MobiusN", name: "rhai_mobiusn", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let re_a = p.re_a;
    let re_b = p.re_b;
    let re_c = p.re_c;
    let re_d = p.re_d;
    let im_a = p.im_a;
    let im_b = p.im_b;
    let im_c = p.im_c;
    let im_d = p.im_d;
    let power_raw = p.power;
    let dist = p.dist;
    let power = power_raw;
    if (abs(power) < 1.0) {
        power = 1.0;
    }
    let z_exp = 4.0 * dist / power;
    let r0 = sqrt(z.re * z.re + z.im * z.im) + 1.0e-6;
    let r1 = pow(r0, z_exp);
    let alpha = atan2(z.im, z.re) * power;
    let x1 = r1 * cos(alpha);
    let y1 = r1 * sin(alpha);
    let real_u = re_a * x1 - im_a * y1 + re_b;
    let imag_u = re_a * y1 + im_a * x1 + im_b;
    let real_v = re_c * x1 - im_c * y1 + re_d;
    let imag_v = re_c * y1 + im_c * x1 + im_d;
    let rad_v = max(real_v * real_v + imag_v * imag_v, 1.0e-30);
    let x2 = (real_u * real_v + imag_u * imag_v) / rad_v;
    let y2 = (imag_u * real_v - real_u * imag_v) / rad_v;
    let z_inv = 1.0 / z_exp;
    let r2 = pow(sqrt(x2 * x2 + y2 * y2), z_inv);
    let fp = to_float(floor(power));
    let safe_fp = (if fp == 0.0 { 1.0 } else { fp });
    let n = to_float(floor(power * rand()));
    let alpha2 = (atan2(y2, x2) + n * 6.28318530717959) / safe_fp;
    complex(r2 * cos(alpha2), r2 * sin(alpha2))
}
