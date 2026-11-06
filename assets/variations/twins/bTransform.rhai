/// Bipolar transform — extends the bipolar-coords mapping `(τ, σ)` with
/// adjustable `power`, `move`, `rotate`, and `split` (asymmetric τ offset
/// based on the sign of x). Picks a random angular branch from `floor(rand
/// · power)`, producing a generalized bipolar warp with N-fold rotational
/// structure.
///
/// # Authors
/// - Michael Faber

// B-Transform, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    rotate: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Rotate",
        hint: "Additive rotation on the bipolar σ coordinate.",
    },
    power: #{
        value: 1, min: 1, max: 100,
        caption: "Power",
        hint: "Number of angular branches (≥ 1).",
    },
    move: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Move",
        hint: "Additive offset on τ.",
    },
    split: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Split",
        hint: "Asymmetric τ offset: added when x ≥ 0, subtracted when x < 0.",
    },
};

const INFO = #{ title: "B-Transform", name: "rhai_btransform", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let rotate = p.rotate;
    let power = max(1.0, to_float(p.power));
    let move_p = p.move;
    let split = p.split;
    let pi = 3.14159265358979;
    let two_pi = 6.28318530717959;
    let xp1 = z.re + 1.0;
    let xm1 = z.re - 1.0;
    let arg_p = max(1.0e-30, xp1 * xp1 + z.im * z.im);
    let arg_m = max(1.0e-30, xm1 * xm1 + z.im * z.im);
    let tau = 0.5 * (ln(arg_p) - ln(arg_m)) / power + move_p;
    let sigma = pi - atan2(z.im, xp1) - atan2(z.im, 1.0 - z.re) + rotate;
    sigma = sigma / power + two_pi / power * to_float(floor(rand() * power));
    if (z.re >= 0.0) {
        tau = tau + split;
    } else {
        tau = tau - split;
    }
    let sinht = sinh(tau);
    let cosht = cosh(tau);
    let sins = sin(sigma);
    let coss = cos(sigma);
    let temp = cosht - coss;
    let safe_temp = (if abs(temp) < 1.0e-30 { 1.0e-30 } else { temp });
    complex(sinht / safe_temp, sins / safe_temp)
}
