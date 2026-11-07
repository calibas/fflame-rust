/// Complex-power Julia — `power = re + i·im`. Like CPow but with a `dist`
/// parameter that scales the log-of-radius term separately.
///
/// # Authors
/// - David Young

// JuliaC, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    re: #{
        value: 3.0, min: -10.0, max: 10.0, free: true,
        caption: "Re",
        hint: "Real part of the complex power.",
    },
    im: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "Im",
        hint: "Imaginary part of the complex power.",
    },
    dist: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Dist",
        hint: "Distance scaling on the log-of-radius term — affects how rapidly the spiral grows outward.",
    },
};

const INFO = #{ title: "JuliaC", name: "rhai_juliac", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let re_param = p.re;
    let im_param = p.im;
    #{ re_recip: 1.0 / (re_param + 1.0e-30), im_scaled: im_param / 100.0 }
}

fn juliac_branch(re_param) {
    let m = max(1.0e-6, abs(re_param));
    let n = rand_u32() & u32(0x7FFFFFFF);
    let n_hi = to_float(n >> u32(16));
    let n_lo = to_float(n & u32(0xFFFF));
    let r16 = 65536.0 - m * to_float(floor(65536.0 / m));
    let t1 = n_hi * r16;
    let t2 = t1 - m * to_float(floor(t1 / m));
    let t3 = n_lo - m * to_float(floor(n_lo / m));
    let s = t2 + t3;
    s - m * to_float(floor(s / m))
}

fn variation(z, p) {
    let two_pi = 6.28318530717959;
    let re_param = p.re;
    let dist = p.dist;
    let re_recip = p.re_recip;
    let im_scaled = p.im_scaled;
    let arg = atan2(z.im, z.re) + juliac_branch(re_param) * two_pi;
    let lnmod = dist * 0.5 * ln(max(1.0e-30, z.re * z.re + z.im * z.im));
    let a = arg * re_recip + lnmod * im_scaled;
    let mod2 = exp(lnmod * re_recip - arg * im_scaled);
    complex(mod2 * cos(a), mod2 * sin(a))
}
