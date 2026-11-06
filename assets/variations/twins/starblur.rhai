/// N-pointed star-shape blur — samples points uniformly within a star
/// polygon. Each arm is parameterized so the random distribution is area-
/// uniform across the whole star.
///
/// # Authors
/// - Zy0rg

// Star Blur, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    power: #{
        value: 5, min: 2, max: 50,
        caption: "Power",
        hint: "Number of star points.",
    },
    range: #{
        value: 0.40162283, min: 0.0, max: 1.0, free: true,
        caption: "Range",
        hint: "Inner-vertex radius — 0 gives a circle (no points), 1 gives sharp points.",
    },
};

const INFO = #{ title: "Star Blur", name: "rhai_starblur", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let power = max(1.0, to_float(p.power));
    let range = p.range;
    let alpha0 = 3.14159265358979 / power;
    let length = sqrt(max(1.0e-30, 1.0 + range * range - 2.0 * range * cos(alpha0)));
    #{ alpha: asin(sin(alpha0) * range / length), length: length }
}

fn variation(w, p) {
    let power = to_float(p.power);
    let alpha = p.alpha;
    let length = p.length;
    let two_pi = 6.28318530717959;
    let half_pi = 1.5707963267948966;
    let safe_power = max(1.0, power);
    let f = rand() * safe_power * 2.0;
    let arm = to_float(trunc(f));
    f = f - arm;
    let x = f * length;
    let z0 = sqrt(max(1.0e-30, 1.0 + x * x - 2.0 * x * cos(alpha)));
    let arm_i = to_int(arm);
    let arm_pair = to_float(arm_i / 2);
    let ang_pt = 0.0;
    if (arm_i % 2 == 0) {
        ang_pt = two_pi / safe_power * arm_pair + asin(sin(alpha) * x / z0);
    } else {
        ang_pt = two_pi / safe_power * arm_pair - asin(sin(alpha) * x / z0);
    }
    let z = z0 * sqrt(rand());
    let ang = ang_pt - half_pi;
    complex(z * cos(ang), z * sin(ang))
}
