/// Rational-power Julia — like JuliaN but with separate `power` and
/// `divisor`, allowing fractional/rational branch counts (e.g. 3/2 gives
/// 1.5 branches).
///
/// # Authors
/// - Zueuk

// JuliaQ, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    power: #{
        value: 3, min: 1, max: 64,
        caption: "Power",
        hint: "Number of Julia branches in the rational power.",
    },
    divisor: #{
        value: 2, min: 1, max: 64,
        caption: "Divisor",
        hint: "Rational-power divisor. Combined with `power` lets you pick non-integer branch counts (e.g. power=3, divisor=2 → 1.5 branches).",
    },
};

const INFO = #{ title: "JuliaQ", name: "rhai_juliaq", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let power = max(1.0, to_float(p.power));
    let divisor = to_float(p.divisor);
    let inv_power = divisor / power;
    #{ inv_power: inv_power, inv_power_2pi: 6.28318530717959 / power, half_inv_power: 0.5 * inv_power }
}

fn juliaq_branch(power_param) {
    let m = max(1.0, power_param);
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
    let power_param = to_float(p.power);
    let inv_power = p.inv_power;
    let inv_power_2pi = p.inv_power_2pi;
    let half_inv_power = p.half_inv_power;
    let a = atan2(z.im, z.re) * inv_power + juliaq_branch(power_param) * inv_power_2pi;
    let r = pow(max(1.0e-30, z.re * z.re + z.im * z.im), half_inv_power);
    complex(r * cos(a), r * sin(a))
}
