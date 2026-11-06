/// Range-clamped variant of CPow — combines a complex-power Julia (with `re
/// + ai` exponent set via the `r` and `a` parameters) with random branch
/// selection across `range` rotational sectors.
///
/// # Authors
/// - Zueuk

// CPow2, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    r: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "R",
        hint: "Magnitude of the complex exponent.",
    },
    a: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "A",
        hint: "Argument (angle) of the complex exponent — multiplied by π/2 internally, so `a = 1` gives a 90° rotation.",
    },
    divisor: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Divisor",
        hint: "Number of rotational branches.",
    },
    range: #{
        value: 1, min: 1, max: 64,
        caption: "Range",
        hint: "Random branch range — each iteration picks an integer in `[0, range)` to shift the angle.",
    },
};

const INFO = #{ title: "CPow2", name: "rhai_cpow2", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let r_p = p.r;
    let a_p = p.a;
    let divisor = p.divisor;
    let range_p = max(1.0, to_float(p.range));
    let safe_div = (if divisor == 0.0 { 1.0e-30 } else { divisor });
    let c = r_p * cos(1.5707963267948966 * a_p) / safe_div;
    let d = r_p * sin(1.5707963267948966 * a_p) / safe_div;
    #{ ang: 6.28318530717959 / safe_div, c: c, d: d, half_c: c * 0.5, half_d: d * 0.5, inv_range: 0.5 / range_p, full_range: 6.28318530717959 * range_p }
}

fn variation(z, p) {
    let two_pi = 6.28318530717959;
    let divisor = p.divisor;
    let range_p = max(1.0, to_float(p.range));
    let ang = p.ang;
    let c = p.c;
    let d = p.d;
    let half_c = p.half_c;
    let half_d = p.half_d;
    let inv_range = p.inv_range;
    let full_range = p.full_range;
    let a = atan2(z.re, z.im);
    let n = to_int(rand_u32() % u32(range_p));
    if (a < 0.0) {
        n = n + 1;
    }
    a = a + two_pi * to_float(n);
    if (cos(a * inv_range) < rand() * 2.0 - 1.0) {
        a = a - full_range;
    }
    let lnr2 = ln(max(1.0e-30, z.re * z.re + z.im * z.im));
    let r_out = exp(half_c * lnr2 - d * a);
    let th = c * a + half_d * lnr2 + ang * to_float(floor(divisor * rand()));
    complex(r_out * cos(th), r_out * sin(th))
}
