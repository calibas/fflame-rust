/// Hypotrochoid curve sampler — picks a random parameter `t` and emits a
/// point on the classic spirograph curve. `a`/`b` are the outer/inner wheel
/// radii; `c1`/`c2` give per-axis amplitudes; `d` is the pen-arm length.

// Spirograph, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    a: #{
        value: 3.0, min: -10.0, max: 10.0, free: true,
        caption: "A",
        hint: "Outer-wheel radius.",
    },
    b: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "B",
        hint: "Inner-wheel radius.",
    },
    d: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "D",
        hint: "Pen-arm length added to the output.",
    },
    c1: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "C1",
        hint: "X-axis amplitude of the inner-wheel cosine term.",
    },
    c2: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "C2",
        hint: "Y-axis amplitude of the inner-wheel sine term.",
    },
    tmin: #{
        value: -1.0, min: -100.0, max: 100.0, free: true,
        caption: "T Min",
        hint: "Minimum value of the random parameter `t`.",
    },
    tmax: #{
        value: 1.0, min: -100.0, max: 100.0, free: true,
        caption: "T Max",
        hint: "Maximum value of the random parameter `t`.",
    },
    ymin: #{
        value: -1.0, min: -100.0, max: 100.0, free: true,
        caption: "Y Min",
        hint: "Minimum random Y offset added to both output coordinates.",
    },
    ymax: #{
        value: 1.0, min: -100.0, max: 100.0, free: true,
        caption: "Y Max",
        hint: "Maximum random Y offset added to both output coordinates.",
    },
};

const INFO = #{ title: "Spirograph", name: "rhai_spirograph", category: "advanced_2d", phase: "any", z: "preserve_z" };

fn variation(z, p) {
    let a = p.a;
    let b = p.b;
    let d = p.d;
    let c1 = p.c1;
    let c2 = p.c2;
    let tmin = p.tmin;
    let tmax = p.tmax;
    let ymin = p.ymin;
    let ymax = p.ymax;
    let t = (tmax - tmin) * rand() + tmin;
    let y_off = (ymax - ymin) * rand() + ymin;
    let safe_b = (if b == 0.0 { 1.0e-30 } else { b });
    let ratio = (a + b) / safe_b;
    let x1 = (a + b) * cos(t) - c1 * cos(ratio * t);
    let y1 = (a + b) * sin(t) - c2 * sin(ratio * t);
    complex(x1 + d * cos(t) + y_off, y1 + d * sin(t) + y_off)
}

fn variation3(v, p) {
    let a = p.a;
    let b = p.b;
    let d = p.d;
    let c1 = p.c1;
    let c2 = p.c2;
    let tmin = p.tmin;
    let tmax = p.tmax;
    let ymin = p.ymin;
    let ymax = p.ymax;
    let t = (tmax - tmin) * rand() + tmin;
    let y_off = (ymax - ymin) * rand() + ymin;
    let safe_b = (if b == 0.0 { 1.0e-30 } else { b });
    let ratio = (a + b) / safe_b;
    let x1 = (a + b) * cos(t) - c1 * cos(ratio * t);
    let y1 = (a + b) * sin(t) - c2 * sin(ratio * t);
    vec3(x1 + d * cos(t) + y_off, y1 + d * sin(t) + y_off, v.z)
}
