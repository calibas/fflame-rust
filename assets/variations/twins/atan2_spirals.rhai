/// 14-parameter atan2-driven spiral generator — combines two `atan2` calls
/// (one over per-power radial terms, one over per-divisor xy positions) and
/// a sine wrap, with separate weights and offsets per component. X output
/// is mirrored across `±π` based on the sign of x.
///
/// # Authors
/// - Whittaker Courtney

// Atan2 Spirals, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    r_mult: #{
        value: 1.5, min: -10.0, max: 10.0, free: true,
        caption: "R Mult",
        hint: "Multiplier on the first atan2's numerator (r-term).",
    },
    r_add: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "R Add",
        hint: "Offset on the first atan2's numerator.",
    },
    xy2_mult: #{
        value: 1.125, min: -10.0, max: 10.0, free: true,
        caption: "XY² Mult",
        hint: "Multiplier on the first atan2's denominator (xy²-term).",
    },
    xy2_add: #{
        value: 0.132, min: -10.0, max: 10.0, free: true,
        caption: "XY² Add",
        hint: "Offset on the first atan2's denominator.",
    },
    x_mult: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "X Mult",
        hint: "Output multiplier on the first atan2.",
    },
    x_add: #{
        value: 0.25, min: -10.0, max: 10.0, free: true,
        caption: "X Add",
        hint: "Output additive offset on X.",
    },
    yx_div: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "YX Div",
        hint: "Divisor on x in the second atan2.",
    },
    yx_add: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "YX Add",
        hint: "Additive offset on x in the second atan2.",
    },
    yy_div: #{
        value: 1.25, min: -10.0, max: 10.0, free: true,
        caption: "YY Div",
        hint: "Divisor on y in the second atan2.",
    },
    yy_add: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "YY Add",
        hint: "Additive offset on y in the second atan2.",
    },
    sin_add: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Sin Add",
        hint: "Phase offset on the sine wrap around the second atan2.",
    },
    y_mult: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Y Mult",
        hint: "Output multiplier on Y.",
    },
    r_power: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "R Power",
        hint: "Exponent on the radial term `r = (x²+y²)^r_power`.",
    },
    x2y2_pow: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "X²Y² Pow",
        hint: "Exponent on the xy² term `xy² = (x²+y²)^x2y2_pow`.",
    },
};

const INFO = #{ title: "Atan2 Spirals", name: "rhai_atan2_spirals", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let r_mult = p.r_mult;
    let r_add = p.r_add;
    let xy2_mult = p.xy2_mult;
    let xy2_add = p.xy2_add;
    let x_mult = p.x_mult;
    let x_add = p.x_add;
    let yx_div = p.yx_div;
    let yx_add = p.yx_add;
    let yy_div = p.yy_div;
    let yy_add = p.yy_add;
    let sin_add = p.sin_add;
    let y_mult = p.y_mult;
    let r_power = p.r_power;
    let x2y2_pow = p.x2y2_pow;
    let pi = 3.14159265358979;
    let r2 = max(z.re * z.re + z.im * z.im, 1.0e-30);
    let xy2 = pow(r2, x2y2_pow);
    let r = pow(r2, r_power);
    let fx = x_mult * atan2(r * r_mult + r_add, xy2 * xy2_mult + xy2_add) + x_add;
    let safe_yy = (if abs(yy_div) < 1.0e-30 { 1.0e-30 } else { yy_div });
    let safe_yx = (if abs(yx_div) < 1.0e-30 { 1.0e-30 } else { yx_div });
    let fy = sin(atan2(z.im / safe_yy + yy_add, z.re / safe_yx + yx_add) + sin_add) * y_mult;
    let out_x = (if z.re >= 0.0 { -fx + pi } else { fx - pi });
    complex(out_x, fy)
}
