/// Lissajous-figure sampler — picks a random `t` and emits `(sin(a·t + d),
/// sin(b·t))`. Adds linear drift `c·t` and noise `e·y` for thicker curves.
///
/// # Authors
/// - Jed Kelsey

// Lissajous, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    tmin: #{
        value: -3.1415927, min: -100.0, max: 100.0, free: true,
        caption: "T Min",
        hint: "Minimum value of the random parameter `t`.",
    },
    tmax: #{
        value: 3.1415927, min: -100.0, max: 100.0, free: true,
        caption: "T Max",
        hint: "Maximum value of the random parameter `t`.",
    },
    a: #{
        value: 3.0, min: -20.0, max: 20.0, free: true,
        caption: "A",
        hint: "X-axis frequency multiplier.",
    },
    b: #{
        value: 2.0, min: -20.0, max: 20.0, free: true,
        caption: "B",
        hint: "Y-axis frequency multiplier.",
    },
    c: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "C",
        hint: "Linear drift added to both axes.",
    },
    d: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "D",
        hint: "Phase offset on the X axis.",
    },
    e: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "E",
        hint: "Noise scale added to both axes.",
    },
};

const INFO = #{ title: "Lissajous", name: "rhai_lissajous", category: "advanced_2d", phase: "any", z: "preserve_z" };

fn variation(z, p) {
    let tmin = p.tmin;
    let tmax = p.tmax;
    let a = p.a;
    let b = p.b;
    let c = p.c;
    let d = p.d;
    let e = p.e;
    let t = (tmax - tmin) * rand() + tmin;
    let y = rand() - 0.5;
    let x1 = sin(a * t + d);
    let y1 = sin(b * t);
    complex(x1 + c * t + e * y, y1 + c * t + e * y)
}

fn variation3(v, p) {
    let tmin = p.tmin;
    let tmax = p.tmax;
    let a = p.a;
    let b = p.b;
    let c = p.c;
    let d = p.d;
    let e = p.e;
    let t = (tmax - tmin) * rand() + tmin;
    let y = rand() - 0.5;
    let x1 = sin(a * t + d);
    let y1 = sin(b * t);
    vec3(x1 + c * t + e * y, y1 + c * t + e * y, v.z)
}
