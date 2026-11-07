/// Randomized complex-power warp (CPow3 family) — combines a complex-power
/// radial-angular mapping with discrete and continuous angular spreads,
/// plus a secondary multiplicative random offset on the final angle. Per-
/// iteration: random shift of the angle index `n` (truncated to integer if
/// `discrete_spread = 1`), a probabilistic angle subtraction, and a
/// `spread2 · rand + offset2` random multiplier on the output angle.
///
/// # Authors
/// - CozyG

// CPow3 WF, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    r: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "R",
        hint: "Complex-power magnitude.",
    },
    a: #{
        value: 0.1, min: -10.0, max: 10.0, free: true,
        caption: "A",
        hint: "Complex-power angle (scaled by π/2 internally).",
    },
    divisor: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Divisor",
        hint: "Divides both r and a contributions; controls how the power scales.",
    },
    spread: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Spread",
        hint: "Range of the random angle index `n`. Each iteration picks `n` uniformly in `[0, spread)`.",
    },
    discrete_spread: #{
        value: 1.0, min: 0.0, max: 1.0, free: true,
        caption: "Discrete Spread",
        hint: "≥ 1 = truncate `n` to an integer (discrete angular branches); < 1 = continuous angle.",
    },
    spread2: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Spread 2",
        hint: "Range of the secondary multiplicative random offset on the final angle.",
    },
    offset2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Offset 2",
        hint: "Constant offset added to the secondary random multiplier.",
    },
};

const INFO = #{ title: "CPow3 WF", name: "rhai_cpow3_wf", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let pi = 3.14159265358979;
    let two_pi = 6.28318530717959;
    let r = p.r;
    let a = p.a;
    let divisor = p.divisor;
    let spread = p.spread;
    let safe_div = (if abs(divisor) < 1.0e-30 { 1.0e-30 } else { divisor });
    let safe_spread = (if abs(spread) < 1.0e-30 { 1.0e-30 } else { spread });
    let c = r * cos(pi * 0.5 * a) / safe_div;
    let d = r * sin(pi * 0.5 * a) / safe_div;
    #{ c: c, d: d, half_c: c * 0.5, half_d: d * 0.5, ang: two_pi / safe_div, inv_spread: 0.5 / safe_spread, full_spread: two_pi * spread }
}

fn variation(z, p) {
    let spread = p.spread;
    let discrete_spread = p.discrete_spread;
    let spread2 = p.spread2;
    let offset2 = p.offset2;
    let c = p.c;
    let d = p.d;
    let half_c = p.half_c;
    let half_d = p.half_d;
    let ang = p.ang;
    let inv_spread = p.inv_spread;
    let full_spread = p.full_spread;
    let two_pi = 6.28318530717959;
    let ai = atan2(z.im, z.re);
    let n = rand() * spread;
    if (discrete_spread >= 1.0) {
        n = to_float(trunc(n));
    }
    if (ai < 0.0) {
        n = n + 1.0;
    }
    ai = ai + two_pi * n;
    if (cos(ai * inv_spread) < rand() * 2.0 - 1.0) {
        ai = ai - full_spread;
    }
    let r2 = z.re * z.re + z.im * z.im;
    let lnr2 = ln(max(1.0e-30, r2));
    let ri = exp(half_c * lnr2 - d * ai);
    let ang2 = c * ai * half_d * lnr2 * ang * (rand() * spread2 + offset2);
    complex(ri * cos(ang2), ri * sin(ang2))
}
