/// Bipolar coordinates with 9 user-tunable scaling/offset knobs — much more
/// configurable than the basic Bipolar.
///
/// # Authors
/// - Apophysis Plugin Pack
/// - Brad Stefanov

// Bipolar 2, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    shift: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "Shift",
        hint: "Vertical offset added to the bipolar angle output.",
    },
    a: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "A",
        hint: "Inner offset for the radius² term.",
    },
    b: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "B",
        hint: "X scaling for the log term's numerator/denominator.",
    },
    c: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "C",
        hint: "Scaling on the angle output.",
    },
    d: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "D",
        hint: "Offset inside the atan2 denominator.",
    },
    e: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "E",
        hint: "Y scaling inside the atan2 numerator.",
    },
    f1: #{
        value: 0.25, min: -10.0, max: 10.0, free: true,
        caption: "F",
        hint: "Output X scaling factor.",
    },
    g1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "G",
        hint: "Outer scaling on the squared radius.",
    },
    h: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "H",
        hint: "Output Y scaling factor.",
    },
};

const INFO = #{ title: "Bipolar 2", name: "rhai_bipolar2", category: "advanced_2d", phase: "any", z: "preserve_z" };

fn variation(z, p) {
    let shift = p.shift;
    let a = p.a;
    let b = p.b;
    let c = p.c;
    let d = p.d;
    let e = p.e;
    let f1 = p.f1;
    let g1 = p.g1;
    let h = p.h;
    let pi = 3.14159265358979;
    let halfpi = 1.5707963267948966;
    let two_over_pi = 0.6366197723675813;
    let x2y2 = (z.re * z.re + z.im * z.im) * g1;
    let t = x2y2 + a;
    let x2 = b * z.re;
    let ps = -halfpi * shift;
    let y = c * atan2(e * z.im, x2y2 - d) + ps;
    if (y > halfpi) {
        y = -halfpi + (y + halfpi - to_float(floor((y + halfpi) / pi)) * pi);
    } else if (y < -halfpi) {
        y = halfpi - (halfpi - y - to_float(floor((halfpi - y) / pi)) * pi);
    }
    let f = t + x2;
    let g = t - x2;
    if (g == 0.0 || f / g <= 0.0) {
        return complex(0.0, 0.0);
    }
    complex(f1 * two_over_pi * ln(f / g), two_over_pi * y * h)
}

fn variation3(v, p) {
    let shift = p.shift;
    let a = p.a;
    let b = p.b;
    let c = p.c;
    let d = p.d;
    let e = p.e;
    let f1 = p.f1;
    let g1 = p.g1;
    let h = p.h;
    let pi = 3.14159265358979;
    let halfpi = 1.5707963267948966;
    let two_over_pi = 0.6366197723675813;
    let x2y2 = (v.x * v.x + v.y * v.y) * g1;
    let t = x2y2 + a;
    let x2 = b * v.x;
    let ps = -halfpi * shift;
    let y = c * atan2(e * v.y, x2y2 - d) + ps;
    if (y > halfpi) {
        y = -halfpi + (y + halfpi - to_float(floor((y + halfpi) / pi)) * pi);
    } else if (y < -halfpi) {
        y = halfpi - (halfpi - y - to_float(floor((halfpi - y) / pi)) * pi);
    }
    let f = t + x2;
    let g = t - x2;
    if (g == 0.0 || f / g <= 0.0) {
        return vec3(0.0, 0.0, v.z);
    }
    vec3(f1 * two_over_pi * ln(f / g), two_over_pi * y * h, v.z)
}
