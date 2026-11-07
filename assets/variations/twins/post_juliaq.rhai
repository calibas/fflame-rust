/// Post-phase version of JuliaQ — applies the rational-power Julia
/// branching after all other variations have run.
///
/// # Authors
/// - Zueuk

// Post JuliaQ, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    power: #{
        value: 3, min: -50, max: 50,
        caption: "Power",
        hint: "Number of Julia branches in the rational power.",
    },
    divisor: #{
        value: 2, min: -50, max: 50,
        caption: "Divisor",
        hint: "Rational-power divisor. Combined with `power` lets you pick non-integer branch counts (e.g. power=3, divisor=2 → 1.5 branches).",
    },
};

const INFO = #{ title: "Post JuliaQ", name: "rhai_post_juliaq", category: "advanced_2d", phase: "post" };

fn prepare(p) {
    let power = to_float(p.power);
    let divisor = to_float(p.divisor);
    let safe_power = (if power == 0.0 { 1.0e-30 } else { power });
    let two_pi = 6.28318530717959;
    #{ half_inv_power: 0.5 * divisor / safe_power, inv_power: divisor / safe_power, inv_power_2pi: two_pi / safe_power }
}

fn variation(z, p, ctx) {
    let power = to_float(p.power);
    let half_inv_power = p.half_inv_power;
    let inv_power = p.inv_power;
    let inv_power_2pi = p.inv_power_2pi;
    let w = ctx.weight;
    let n = to_float(floor(rand() * power));
    let a = atan2(z.im, z.re) * inv_power + n * inv_power_2pi;
    let r = w * pow(max(1.0e-30, z.re * z.re + z.im * z.im), half_inv_power);
    complex(r * cos(a), r * sin(a))
}
