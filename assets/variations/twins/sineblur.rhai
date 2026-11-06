/// Radial sine-density blur — samples points uniformly on the unit disc
/// with `acos`-based radial distribution. `power` controls how the density
/// falls off.
///
/// # Authors
/// - Zy0rg

// Sine Blur, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    power: #{
        value: 1.0, min: 0.0, max: 10.0, free: true,
        caption: "Power",
        hint: "Density power. 1.0 gives uniform-on-disc; higher values concentrate density near the edge.",
    },
};

const INFO = #{ title: "Sine Blur", name: "rhai_sineblur", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let power_in = p.power;
    let power = max(0.0, power_in);
    let inv_pi = 0.3183098861837907;
    let ang = rand() * 6.28318530717959;
    let r = 0.0;
    if (power == 1.0) {
        r = acos(rand() * 2.0 - 1.0) * inv_pi;
    } else {
        let u = max(1.0e-30, rand());
        r = acos(exp(ln(u) * power) * 2.0 - 1.0) * inv_pi;
    }
    complex(r * cos(ang), r * sin(ang))
}
