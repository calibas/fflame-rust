/// Generalized Julia variation with a chosen integer power. Splits the
/// angle into `power` equally-spaced branches and randomly picks one each
/// iteration. With power = 2 it reduces to the classic Julia.
///
/// # Authors
/// - Scott Draves

// JuliaN, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    power: #{
        value: 2, min: -10, max: 10, free: true,
        caption: "Power",
        hint: "Number of branches the output is split into. Higher = more arms; negative values flip the rotation direction.",
    },
    dist: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Distance",
        hint: "Stretches or compresses each arm radially. 1.0 is balanced; larger pushes arms outward, smaller pulls them in.",
    },
};

const INFO = #{ title: "JuliaN", name: "rhai_julian", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let power = to_float(p.power);
    let dist = p.dist;
    #{ cpower: dist / max(1.0e-30, abs(power)) / 2.0 }
}

fn variation(z, p) {
    let power = to_float(p.power);
    let cpower = p.cpower;
    let r2 = z.norm_sqr();
    let r = pow(r2, cpower);
    let theta = atan2(z.im, z.re);
    let trunc_val = to_float(floor(abs(power) * rand()));
    let t = (theta + 6.28318530718 * trunc_val) / power;
    complex(r * cos(t), r * sin(t))
}

fn inverse(z, p, branch) {
    let power = to_float(p.power);
    let rr = pow(z.abs(), abs(power) / p.dist);
    let a = power * atan2(z.im, z.re);
    complex(rr * cos(a), rr * sin(a))
}
