/// Circular with user-tunable hash multipliers — same shape as Circular but
/// exposes the `(12.9898, 78.233)` magic numbers as `xx` / `yy` parameters.
///
/// # Authors
/// - Tatyana Zabanova
/// - Brad Stefanov

// Circular 2, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    angle: #{
        value: 90.0, min: -360.0, max: 360.0, type: "angle",
        caption: "Angle",
        hint: "Maximum rotation per iteration (degrees).",
    },
    seed: #{
        value: 0.0, min: -100.0, max: 100.0, free: true,
        caption: "Seed",
        hint: "Random seed for the hash term — change to vary the pattern.",
    },
    xx: #{
        value: 12.9898, min: -100.0, max: 100.0, free: true,
        caption: "X mul",
        hint: "X-axis multiplier for the hash. Default 12.9898 matches the standard Circular.",
    },
    yy: #{
        value: 78.233, min: -100.0, max: 100.0, free: true,
        caption: "Y mul",
        hint: "Y-axis multiplier for the hash. Default 78.233 matches the standard Circular.",
    },
};

const INFO = #{ title: "Circular 2", name: "rhai_circular2", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let angle_deg = p.angle;
    let seed = p.seed;
    let xx = p.xx;
    let yy = p.yy;
    let c_a = angle_deg * 3.14159265358979 / 180.0;
    let h = sin(z.re * xx + z.im * yy + seed) * 43758.5453;
    let aux = h - to_float(trunc(h));
    let rnd = (2.0 * (rand() + aux) - 2.0) * c_a;
    let rad = sqrt(z.re * z.re + z.im * z.im);
    let ang = atan2(z.im, z.re);
    complex(cos(ang + rnd) * rad, sin(ang + rnd) * rad)
}
