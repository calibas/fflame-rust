/// Raises the complex point to a complex power (real + imaginary parts of
/// the exponent both adjustable). Produces logarithmic spirals with `power`
/// arms.
///
/// # Authors
/// - Apophysis Plugin Pack

// CPow, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    r: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "R",
        hint: "Real component of the complex exponent. Controls scaling and how tightly the spiral winds.",
    },
    i: #{
        value: 0.1, min: -10.0, max: 10.0, free: true,
        caption: "I",
        hint: "Imaginary component of the complex exponent. Controls how much the spiral rotates.",
    },
    power: #{
        value: 1.5, min: -10.0, max: 10.0, free: true,
        caption: "Power",
        hint: "Number of branches in the result. Like JuliaN's `power` - more = more arms.",
    },
};

const INFO = #{ title: "CPow", name: "rhai_cpow", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let r_param = p.r;
    let i_param = p.i;
    let power = p.power;
    let safe_power = (if power == 0.0 { 1.0e-30 } else { power });
    #{ va: 6.28318530717959 / safe_power, vc: r_param / safe_power, vd: i_param / safe_power }
}

fn variation(z, p) {
    let power = p.power;
    let va = p.va;
    let vc = p.vc;
    let vd = p.vd;
    let a = atan2(z.im, z.re);
    let lnr = 0.5 * ln(z.norm_sqr() + 1.0e-20);
    let ang = vc * a + vd * lnr + va * to_float(floor(power * rand()));
    let m = exp(vc * lnr - vd * a);
    complex(m * cos(ang), m * sin(ang))
}
