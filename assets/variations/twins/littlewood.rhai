/// Littlewood-polynomial IFS attractor A(λ) of the pair z ↦ (z±1)/λ
/// (Barnsley–Harrington).
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5

// Littlewood, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    lambda_re: #{
        value: 1.0, min: -3.0, max: 3.0, free: true,
        caption: "Lambda re",
        hint: "Real part of λ in the digit maps z ↦ (z + digit)/λ. Contracting (a true attractor) for |λ| > 1, ratio 1/|λ|. Default λ = 1+i is the twin dragon; real λ gives Bernoulli-convolution Cantor sets on the axis; |λ| → 1 fills densely, large |λ| shatters to dust. λ is a root of a ±1 power series exactly when the attractor contains the origin — animate λ to walk the Littlewood root portrait.",
    },
    lambda_im: #{
        value: 1.0, min: -3.0, max: 3.0, free: true,
        caption: "Lambda im",
        hint: "Imaginary part of λ.",
    },
    coeffs: #{
        value: 0, choices: ["±1 (Littlewood)", "0, ±1 (Borwein)", "±1, ±i (Gaussian)"],
        caption: "Coefficients",
        hint: "The polynomial coefficient set = the IFS digit set. ±1 is the Littlewood pair (2 maps); 0,±1 the Borwein triple (3 maps); ±1,±i the Gaussian quadruple (4 maps, 4-fold symmetric attractors).",
    },
    w1: #{
        value: 1.0, min: 0.0, max: 1.0,
        caption: "Digit 1",
        hint: "Selection weight of digit +1. Unequal weights render the multifractal (biased-coin) measure on the same attractor; zeroing a digit selects a sub-self-affine subset.",
    },
    w2: #{
        value: 1.0, min: 0.0, max: 1.0,
        caption: "Digit 2",
        hint: "Selection weight of digit −1.",
    },
    w3: #{
        value: 1.0, min: 0.0, max: 1.0,
        caption: "Digit 3",
        hint: "Selection weight of digit 0 (Borwein) or +i (Gaussian). Ignored by the ±1 set.",
    },
    w4: #{
        value: 1.0, min: 0.0, max: 1.0,
        caption: "Digit 4",
        hint: "Selection weight of digit −i (Gaussian only).",
    },
    dc_mode: #{
        value: 0, choices: ["Off", "Branch", "Branch Blend"],
        caption: "Color Mode",
        hint: "Direct-color source (needs the transform's Direct Color > 0). Branch: which digit map was applied this call. Branch Blend: persistent register pulled toward each digit's palette slot — colors the attractor by its digit itinerary (the binary/ternary address), the classic two-tone dragon dissection.",
    },
    dc_scale: #{
        value: 1.0, min: 0.1, max: 8.0,
        caption: "Color Scale",
        hint: "Palette-index multiplier for the color modes.",
    },
    color_speed: #{
        value: 0.3, min: 0.01, max: 1.0,
        caption: "Color Speed",
        hint: "Blend rate of the Branch Blend register: low = deep address history (coarse self-affine pieces), high = recent digits only (fine dissection).",
    },
};

const INFO = #{ title: "Littlewood", name: "rhai_littlewood", category: "advanced_2d", z: "preserve_z" };

fn init(p) {
    #{ creg: 0.0 }
}

fn cmul(a, b) {
    complex(a.re * b.re - a.im * b.im, a.re * b.im + a.im * b.re)
}

fn cconj(a) {
    complex(a.re, -a.im)
}

fn cmag2(a) {
    a.re * a.re + a.im * a.im
}

fn cdiv(a, b) {
    let denom = cmag2(b);
    let safe_denom = (if denom < 1.0e-30 { 1.0e-30 } else { denom });
    cmul(a, cconj(b)) / safe_denom
}

fn lw_digit(dset, k) {
    if (dset == u32(1)) {
        if (k == u32(0)) {
            return complex(1.0, 0.0);
        }
        if (k == u32(1)) {
            return complex(-1.0, 0.0);
        }
        return complex(0.0, 0.0);
    }
    if (dset == u32(2)) {
        if (k == u32(0)) {
            return complex(1.0, 0.0);
        }
        if (k == u32(1)) {
            return complex(-1.0, 0.0);
        }
        if (k == u32(2)) {
            return complex(0.0, 1.0);
        }
        return complex(0.0, -1.0);
    }
    if (k == u32(0)) {
        return complex(1.0, 0.0);
    }
    complex(-1.0, 0.0)
}

fn lw_pick(r, w1, w2, w3, w4, cnt) {
    let tot = w1 + w2;
    if (cnt >= u32(3)) {
        tot = tot + w3;
    }
    if (cnt >= u32(4)) {
        tot = tot + w4;
    }
    let x = r * max(1.0e-9, tot);
    if (x < w1) {
        return u32(0);
    }
    if (x < w1 + w2) {
        return u32(1);
    }
    if (cnt >= u32(4) && x >= w1 + w2 + w3) {
        return u32(3);
    }
    u32(2)
}

fn variation(z, p, ctx) {
    let color = ctx.color;
    let lam = complex(p.lambda_re, p.lambda_im);
    let dset = u32(to_float(p.coeffs));
    let w1 = p.w1;
    let w2 = p.w2;
    let w3 = p.w3;
    let w4 = p.w4;
    let dc_mode = u32(to_float(p.dc_mode));
    let dc_scale = p.dc_scale;
    let cspeed = p.color_speed;
    let cnt = u32(2);
    if (dset == u32(1)) {
        cnt = u32(3);
    } else if (dset == u32(2)) {
        cnt = u32(4);
    }
    let k = lw_pick(rand(), w1, w2, w3, w4, cnt);
    let out = cdiv(z + lw_digit(dset, k), lam);
    if (dc_mode == u32(1)) {
        color = fract((to_float(k) + 0.5) / to_float(cnt) * dc_scale);
    } else if (dc_mode == u32(2)) {
        let creg = this.creg;
        creg = mix(creg, (to_float(k) + 0.5) / to_float(cnt), cspeed);
        this.creg = creg;
        color = fract(creg * dc_scale);
    }
    #{ z: out, color: color }
}

fn variation3(v, p, ctx) {
    let color = ctx.color;
    let lam = complex(p.lambda_re, p.lambda_im);
    let dset = u32(to_float(p.coeffs));
    let w1 = p.w1;
    let w2 = p.w2;
    let w3 = p.w3;
    let w4 = p.w4;
    let dc_mode = u32(to_float(p.dc_mode));
    let dc_scale = p.dc_scale;
    let cspeed = p.color_speed;
    let cnt = u32(2);
    if (dset == u32(1)) {
        cnt = u32(3);
    } else if (dset == u32(2)) {
        cnt = u32(4);
    }
    let k = lw_pick(rand(), w1, w2, w3, w4, cnt);
    let out = cdiv(complex(v.x, v.y) + lw_digit(dset, k), lam);
    if (dc_mode == u32(1)) {
        color = fract((to_float(k) + 0.5) / to_float(cnt) * dc_scale);
    } else if (dc_mode == u32(2)) {
        let creg = this.creg;
        creg = mix(creg, (to_float(k) + 0.5) / to_float(cnt), cspeed);
        this.creg = creg;
        color = fract(creg * dc_scale);
    }
    #{ z: vec3(out.re, out.im, v.z), color: color }
}
