/// 10-shape radial hole — applies one of 10 user-selectable radial shape
/// formulas to create a hole pattern. `shape` picks which formula; `inside`
/// toggles whether the hole inverts (`w/r`) or scales (`w·r`).
///
/// # Authors
/// - Michael Faber
/// - Brad Stefanov
/// - Rick Sidwell

// Hole 2, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    a: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "A",
        hint: "Power for the angle-derived scaling factor.",
    },
    b: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "B",
        hint: "Angular wave frequency — used by shapes 2, 6, 7, 8, 9 to vary their angular modulation.",
    },
    c: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "C",
        hint: "Multiplier on the angle-derived scaling factor.",
    },
    d: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "D",
        hint: "Angle multiplier on the input.",
    },
    inside: #{
        value: false,
        caption: "Inside",
        hint: "When on, inverts the radial formula (`w/r`) instead of scaling (`w·r`).",
    },
    shape: #{
        value: 0, choices: ["Linear", "Constant", "Petals", "Dipolar", "Quadratic", "Spiked", "Cardioid", "Half-Petals", "Nested Sin", "Harmonic"],
        caption: "Shape",
        hint: "Radial-formula family. Each option applies a different geometric term inside the `sqrt(rhosq + …)` that determines the output radius.",
    },
};

const INFO = #{ title: "Hole 2", name: "rhai_hole2", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let a = p.a;
    let b = p.b;
    let c = p.c;
    let d = p.d;
    let inside = to_float(p.inside);
    let shape = to_float(p.shape);
    let pi = 3.14159265358979;
    let inv_pi = 0.3183098861837907;
    let half_pi = 1.5707963267948966;
    let rhosq = z.re * z.re + z.im * z.im;
    let theta = atan2(z.im, z.re) * d;
    let delta = pow(max(1.0e-30, theta * inv_pi + 1.0), a) * c;
    let shape_i = to_int(shape);
    let r1 = 1.0;
    if (shape_i == 0) {
        r1 = sqrt(max(0.0, rhosq)) + delta;
    } else if (shape_i == 1) {
        r1 = sqrt(max(0.0, rhosq + delta));
    } else if (shape_i == 2) {
        r1 = sqrt(max(0.0, rhosq + sin(b * theta) + delta));
    } else if (shape_i == 3) {
        r1 = sqrt(max(0.0, rhosq + sin(theta) + delta));
    } else if (shape_i == 4) {
        r1 = sqrt(max(0.0, rhosq + theta * theta - delta + 1.0));
    } else if (shape_i == 5) {
        r1 = sqrt(max(0.0, rhosq + abs(tan(theta)) + delta));
    } else if (shape_i == 6) {
        r1 = sqrt(max(0.0, rhosq * (1.0 + sin(b * theta)) + delta));
    } else if (shape_i == 7) {
        r1 = sqrt(max(0.0, rhosq + abs(sin(0.5 * b * theta)) + delta));
    } else if (shape_i == 8) {
        r1 = sqrt(max(0.0, rhosq + sin(pi * sin(b * theta)) + delta));
    } else {
        r1 = sqrt(max(0.0, rhosq + (sin(b * theta) + sin(2.0 * b * theta + half_pi)) * 0.5 + delta));
    }
    if (inside != 0.0) {
        r1 = 1.0 / max(1.0e-30, r1);
    }
    complex(r1 * cos(theta), r1 * sin(theta))
}
