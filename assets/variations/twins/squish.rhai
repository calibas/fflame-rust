/// Square map with cell mod power — extends `squarize` with a random cell
/// selection: adds an `8·s·floor(power · rand)` offset to the perimeter
/// parameter before dispatch, then divides by `power`. Produces `power`
/// discrete tiles of the squarize pattern.
///
/// # Authors
/// - Michael Faber

// Squish, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    power: #{
        value: 2, min: 2, max: 100,
        caption: "Power",
        hint: "Number of discrete tiles the squarize pattern is divided into (≥ 2).",
    },
};

const INFO = #{ title: "Squish", name: "rhai_squish", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let power = max(2.0, to_float(p.power));
    #{ inv_power: 1.0 / power }
}

fn variation(z, p) {
    let power = max(2.0, to_float(p.power));
    let inv_power = p.inv_power;
    let ax = abs(z.re);
    let ay = abs(z.im);
    let s = 0.0;
    let q = 0.0;
    if (ax > ay) {
        s = ax;
        if (z.re > 0.0) {
            q = z.im;
        } else {
            q = 4.0 * s - z.im;
        }
    } else {
        s = ay;
        if (z.im > 0.0) {
            q = 2.0 * s - z.re;
        } else {
            q = 6.0 * s + z.re;
        }
    }
    q = inv_power * (q + 8.0 * s * to_float(floor(power * rand())));
    if (q <= s) {
        complex(s, q)
    } else if (q <= 3.0 * s) {
        complex(2.0 * s - q, s)
    } else if (q <= 5.0 * s) {
        complex(-s, 4.0 * s - q)
    } else if (q <= 7.0 * s) {
        complex(-(6.0 * s - q), -s)
    } else {
        complex(s, -(8.0 * s - q))
    }
}
