/// N-power spherical with random branch — combines a spherical inversion
/// `1/r^dist` with a random angular shift `n · 2π / ⌊|power|⌋` where `n =
/// ⌊power · rand⌋`. Equivalent to spherical-inverting the input and then
/// rotating by one of `⌊|power|⌋` random angles.
/// 
/// # Authors
/// - eralex61

// Spherical N, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    power: #{
        value: 1.0, min: -50.0, max: 50.0, free: true,
        caption: "Power",
        hint: "Number of angular branches — `⌊|power|⌋` determines the spoke count, and each iteration picks one at random.",
    },
    dist: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Distance",
        hint: "Radial-power exponent — output radius is `1 / r^dist`. `dist = 1` reduces to a standard spherical inversion.",
    },
};

const INFO = #{ title: "Spherical N", name: "rhai_sphericaln", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let power = p.power;
    let dist = p.dist;
    let two_pi = 6.28318530717959;
    let safe_power_floor = max(to_float(floor(abs(power))), 1.0);
    let r0 = sqrt(z.re * z.re + z.im * z.im);
    let R = pow(max(r0, 1.0e-30), dist);
    let n = to_float(floor(power * rand()));
    let alpha = atan2(z.im, z.re) + n * two_pi / safe_power_floor;
    complex(cos(alpha) / max(R, 1.0e-30), sin(alpha) / max(R, 1.0e-30))
}
