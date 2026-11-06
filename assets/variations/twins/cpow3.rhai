/// Logarithm-shifted variant of CPow. Replaces the `i` parameter with `d`
/// (taken through a log transform) and adds a `spread` slider for log-
/// distributed angle perturbation.
///
/// # Authors
/// - Zueuk

// CPow3, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    r: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "R",
        hint: "Magnitude of the complex exponent.",
    },
    d: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "D",
        hint: "Logarithmic argument scaling. Negative values are absorbed via `-log(-d)`.",
    },
    divisor: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Divisor",
        hint: "Number of rotational branches.",
    },
    spread: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Spread",
        hint: "Logarithmic spread of the random angle perturbation.",
    },
};

const INFO = #{ title: "CPow3", name: "rhai_cpow3", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let r_p = p.r;
    let d_p = p.d;
    let divisor = p.divisor;
    let spread = p.spread;
    let safe_div = (if divisor == 0.0 { 1.0e-30 } else { divisor });
    let log_d = (if d_p < 0.0 { -ln(max(1.0e-30, -d_p)) } else { ln(max(1.0e-30, d_p)) });
    let p_a = atan2(log_d * r_p, 6.28318530717959);
    let cos_pa = cos(p_a);
    let c = cos_pa * r_p * cos(p_a) / safe_div;
    let d_calc = cos_pa * r_p * sin(p_a) / safe_div;
    #{ ang: 6.28318530717959 / safe_div, c: c, d_calc: d_calc, half_c: c * 0.5, half_d: d_calc * 0.5, coeff: (if d_calc == 0.0 { 0.0 } else { -0.095 * spread / d_calc }) }
}

fn variation(z, p) {
    let two_pi = 6.28318530717959;
    let divisor = p.divisor;
    let ang = p.ang;
    let c = p.c;
    let d_calc = p.d_calc;
    let half_c = p.half_c;
    let half_d = p.half_d;
    let coeff = p.coeff;
    let a = atan2(z.re, z.im);
    if (a < 0.0) {
        a = a + two_pi;
    }
    if (cos(a * 0.5) < rand() * 2.0 - 1.0) {
        a = a - two_pi;
    }
    let sign_step = (if rand() < 0.5 { 1.0 } else { -1.0 });
    a = a + sign_step * two_pi * round_even(ln(max(1.0e-30, rand())) * coeff);
    let lnr2 = ln(max(1.0e-30, z.re * z.re + z.im * z.im));
    let r_out = exp(half_c * lnr2 - d_calc * a);
    let th = c * a + half_d * lnr2 + ang * to_float(floor(divisor * rand()));
    complex(r_out * cos(th), r_out * sin(th))
}
