/// Alternates two angular rotations on concentric log-spaced rings — points
/// in 'even' rings rotate by `even`, points in 'odd' rings by `odd`.
/// Creates a target-like pattern of rotation bands.
///
/// # Authors
/// - Michael Faber

// Target, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    even: #{
        value: 0.0, min: -360.0, max: 360.0, type: "angle",
        caption: "Even",
        hint: "Rotation angle (degrees) applied to even-numbered rings.",
    },
    odd: #{
        value: 0.0, min: -360.0, max: 360.0, type: "angle",
        caption: "Odd",
        hint: "Rotation angle (degrees) applied to odd-numbered rings.",
    },
    size: #{
        value: 1.0, min: 0.01, max: 10.0, free: true,
        caption: "Size",
        hint: "Ring spacing — controls how thick each ring is in log-radius space.",
    },
};

const INFO = #{ title: "Target", name: "rhai_target", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    #{ t_size_2: 0.5 * p.size }
}

fn variation(z, p) {
    let even_p = p.even;
    let odd_p = p.odd;
    let size = max(1.0e-6, p.size);
    let t_size_2 = p.t_size_2;
    let a = atan2(z.im, z.re);
    let r = sqrt(z.re * z.re + z.im * z.im);
    let t = ln(max(1.0e-30, r));
    if (t < 0.0) {
        t = t - t_size_2;
    }
    t = abs(t) - to_float(floor(abs(t) / size)) * size;
    if (t < t_size_2) {
        a = a + even_p;
    } else {
        a = a + odd_p;
    }
    complex(r * cos(a), r * sin(a))
}
