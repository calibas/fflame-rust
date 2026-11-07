/// Per-axis arctangent warp with mode selector — applies
/// `(2/π)·atan(stretch · coord)` to one or both axes (controlled by
/// `mode`). Saturates the input toward ±1, with `stretch` controlling how
/// quickly the saturation kicks in.
///
/// # Authors
/// - FractalDesire
/// - Brad Stefanov

// Atan, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    mode: #{
        value: 0, choices: ["Y Only", "X Only", "Both"],
        caption: "Mode",
        hint: "Which axes get the arctangent transform.",
    },
    stretch: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Stretch",
        hint: "Pre-atan input scaling. Higher = sharper saturation toward ±1.",
    },
};

const INFO = #{ title: "Atan", name: "rhai_atan", category: "advanced_2d", phase: "any", z: "preserve_z" };

fn variation(z, p) {
    let mode = to_float(p.mode);
    let stretch = p.stretch;
    let two_over_pi = 0.6366197723675813;
    let mi = to_int(mode);
    if (mi == 0) {
        return complex(z.re, two_over_pi * atan(stretch * z.im));
    } else if (mi == 1) {
        return complex(two_over_pi * atan(stretch * z.re), z.im);
    }
    complex(two_over_pi * atan(stretch * z.re), two_over_pi * atan(stretch * z.im))
}

fn variation3(v, p) {
    let mode = to_float(p.mode);
    let stretch = p.stretch;
    let two_over_pi = 0.6366197723675813;
    let mi = to_int(mode);
    if (mi == 0) {
        return vec3(v.x, two_over_pi * atan(stretch * v.y), v.z);
    } else if (mi == 1) {
        return vec3(two_over_pi * atan(stretch * v.x), v.y, v.z);
    }
    vec3(two_over_pi * atan(stretch * v.x), two_over_pi * atan(stretch * v.y), v.z)
}
