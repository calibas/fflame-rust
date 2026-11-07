/// Generalized N-th root of `(x²+y²)^p` with random branch — computes the
/// input angle, scales it by `1/denominator`, adds a random `root · 2π ·
/// k/denominator` branch (with `k` random from `0` to `denominator − 1`),
/// multiplies the whole angle by `numerator`, and emits at radius governed
/// by `numerator / (2·denominator) · (correctd/correctn)`.
///
/// # Authors
/// - cothe
/// - DarkBeam

// Pow Block, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    numerator: #{
        value: 3.0, min: -50.0, max: 50.0, free: true,
        caption: "Numerator",
        hint: "Output-angle multiplier and effective power numerator.",
    },
    denominator: #{
        value: 2.0, min: -50.0, max: 50.0, free: true,
        caption: "Denominator",
        hint: "Branch count — `floor(rand · denominator)` picks the branch.",
    },
    correctn: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Correct N",
        hint: "Power-correction numerator (divides the effective exponent).",
    },
    correctd: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Correct D",
        hint: "Power-correction denominator (multiplies the effective exponent).",
    },
    root: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Root",
        hint: "Per-branch angular offset multiplier (× 2π).",
    },
};

const INFO = #{ title: "Pow Block", name: "rhai_pow_block", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let numerator = p.numerator;
    let denominator = p.denominator;
    let correctn = p.correctn;
    let correctd = p.correctd;
    let abs_cd = max(1.0e-30, abs(correctd));
    let pwr = denominator * correctn / abs_cd;
    if (abs(pwr) <= 1.0e-30) {
        pwr = 1.0e-30;
    }
    pwr = numerator * 0.5 / pwr;
    let abs_d = max(1.0e-30, abs(denominator));
    #{ pwr: pwr, deneps: 1.0 / abs_d }
}

fn variation(z, p) {
    let numerator = p.numerator;
    let denominator = p.denominator;
    let root = p.root;
    let pwr = p.pwr;
    let deneps = p.deneps;
    let two_pi = 6.28318530717959;
    let theta = atan2(z.re, z.im);
    let r2 = pow(max(1.0e-30, z.re * z.re + z.im * z.im), pwr);
    let ran = (theta * deneps + root * two_pi * to_float(floor(rand() * denominator)) * deneps) * numerator;
    complex(r2 * cos(ran), r2 * sin(ran))
}
