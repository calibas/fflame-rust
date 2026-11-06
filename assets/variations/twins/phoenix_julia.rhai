/// Phoenix-Julia N-th-root sampler with X/Y distortion — distorts the input
/// by `(1 + x_distort, 1 + y_distort)`, scales the resulting angle by
/// `1/power`, adds a random branch `n · 2π/power` (with `n` random from `0`
/// to `power − 1`), and emits at radius `r^(dist/power)` in the new angular
/// direction.
///
/// # Authors
/// - TyrantWave

// Phoenix Julia, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    power: #{
        value: 3.0, min: -50.0, max: 50.0, free: true,
        caption: "Power",
        hint: "Number of angular branches — `floor(rand · power)` picks the branch each iteration.",
    },
    dist: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Distance",
        hint: "Radial-power exponent — output magnitude is `r^(dist/power)`.",
    },
    x_distort: #{
        value: -0.5, min: -10.0, max: 10.0, free: true,
        caption: "X distort",
        hint: "X-axis pre-rotation distortion (multiplier on X before angle computation, offset by +1).",
    },
    y_distort: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Y distort",
        hint: "Y-axis pre-rotation distortion (offset by +1).",
    },
};

const INFO = #{ title: "Phoenix Julia", name: "rhai_phoenix_julia", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let power = p.power;
    let dist = p.dist;
    let safe_power = (if power == 0.0 { 1.0e-30 } else { power });
    let two_pi = 6.28318530717959;
    #{ invn: dist / safe_power, inv_2pi_n: two_pi / safe_power, cn: dist / safe_power * 0.5 }
}

fn variation(z, p) {
    let power = p.power;
    let x_distort = p.x_distort;
    let y_distort = p.y_distort;
    let invn = p.invn;
    let inv_2pi_n = p.inv_2pi_n;
    let cn = p.cn;
    let pre_x = z.re * (x_distort + 1.0);
    let pre_y = z.im * (y_distort + 1.0);
    let n = to_float(floor(rand() * power));
    let a = atan2(pre_y, pre_x) * invn + n * inv_2pi_n;
    let r = pow(max(1.0e-30, z.re * z.re + z.im * z.im), cn);
    complex(r * cos(a), r * sin(a))
}
