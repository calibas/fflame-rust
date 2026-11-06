/// Bipolar swirl — adds `τ·out + in/τ` to the bipolar σ coordinate. The
/// combination of an additive (`out`) and reciprocal (`in`) term produces a
/// hyperbolic-style swirl pattern aligned with the bipolar grid.
///
/// # Authors
/// - Michael Faber

// BSwirl, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    in_p: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "In",
        hint: "Reciprocal-τ swirl coefficient — adds `in/τ` to σ.",
    },
    out_p: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Out",
        hint: "Linear-τ swirl coefficient — adds `τ·out` to σ.",
    },
};

const INFO = #{ title: "BSwirl", name: "rhai_bswirl", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let in_p = p.in_p;
    let out_p = p.out_p;
    let pi = 3.14159265358979;
    let tau = 0.5 * (ln(max((z.re + 1.0) * (z.re + 1.0) + z.im * z.im, 1.0e-30)) - ln(max((z.re - 1.0) * (z.re - 1.0) + z.im * z.im, 1.0e-30)));
    let safe_tau = (if abs(tau) < 1.0e-30 { 1.0e-30 } else { tau });
    let sigma = pi - atan2(z.im, z.re + 1.0) - atan2(z.im, 1.0 - z.re);
    sigma = sigma + tau * out_p + in_p / safe_tau;
    let temp = cosh(tau) - cos(sigma);
    if (temp == 0.0) {
        return complex(0.0, 0.0);
    }
    complex(sinh(tau) / temp, sin(sigma) / temp)
}
