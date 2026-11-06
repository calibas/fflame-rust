// Harmonograph (Sosa), the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    seed: #{
        value: 1234, min: 0, max: 999999,
        caption: "Seed",
        hint: "Declared but unused in this port — upstream cpp re-seeds `GOODRAND` per Prepare; we use the per-thread RNG instead. Kept in the parameter list for cpp/Java interface parity.",
    },
    time: #{
        value: 100, min: 1, max: 1000,
        caption: "Time",
        hint: "Maximum value of the random sampling time `t ∈ [0, time]`. Larger values let the damped exponential `e^(-d·t)` decay further, attenuating contributions from later samples.",
    },
    a1: #{
        value: 0, min: 0, max: 360,
        caption: "A1",
        hint: "X-axis oscillator 1: amplitude. Builds `a1·sin(t·f1 + p1°)·e^(-d1·t)` — one of two terms summed to make the X output. Declared `int` to match the upstream parameter type.",
    },
    f1: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "F1",
        hint: "X-axis oscillator 1: frequency.",
    },
    p1: #{
        value: 0, min: 0, max: 360,
        caption: "P1",
        hint: "X-axis oscillator 1: phase offset in degrees (converted to radians internally).",
    },
    d1: #{
        value: 0.0, min: 0.0, max: 1.0, free: true,
        caption: "D1",
        hint: "X-axis oscillator 1: exponential damping rate (0 = no damping; larger values attenuate later samples faster).",
    },
    a2: #{
        value: 0, min: 0, max: 360,
        caption: "A2",
        hint: "X-axis oscillator 2: amplitude. Second term of the X-axis sum.",
    },
    f2: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "F2",
        hint: "X-axis oscillator 2: frequency.",
    },
    p2: #{
        value: 0, min: 0, max: 360,
        caption: "P2",
        hint: "X-axis oscillator 2: phase offset in degrees.",
    },
    d2: #{
        value: 0.0, min: 0.0, max: 1.0, free: true,
        caption: "D2",
        hint: "X-axis oscillator 2: exponential damping rate.",
    },
    a3: #{
        value: 0, min: 0, max: 360,
        caption: "A3",
        hint: "Y-axis oscillator 1: amplitude. Builds `a3·sin(t·f3 + (p3+90)°)·e^(-d3·t)` — first term of the Y-axis sum, with an extra `+90°` phase shift.",
    },
    f3: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "F3",
        hint: "Y-axis oscillator 1: frequency.",
    },
    p3: #{
        value: 0, min: 0, max: 360,
        caption: "P3",
        hint: "Y-axis oscillator 1: phase offset in degrees. The body adds `+90°` before the `π/180` conversion.",
    },
    d3: #{
        value: 0.0, min: 0.0, max: 1.0, free: true,
        caption: "D3",
        hint: "Y-axis oscillator 1: exponential damping rate.",
    },
    a4: #{
        value: 0, min: 0, max: 360,
        caption: "A4",
        hint: "Y-axis oscillator 2: amplitude. Second term of the Y-axis sum (also gets the `+90°` phase shift on `p4`).",
    },
    f4: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "F4",
        hint: "Y-axis oscillator 2: frequency.",
    },
    p4: #{
        value: 0, min: 0, max: 360,
        caption: "P4",
        hint: "Y-axis oscillator 2: phase offset in degrees (with `+90°` shift).",
    },
    d4: #{
        value: 0.0, min: 0.0, max: 1.0, free: true,
        caption: "D4",
        hint: "Y-axis oscillator 2: exponential damping rate.",
    },
};

const INFO = #{ title: "Harmonograph (Sosa)", name: "rhai_harmonograph_js" };

fn mo_harmonograph(t, a1, f1, p1deg, d1, a2, f2, p2deg, d2) {
    let pi = 3.14159265358979;
    let p1 = p1deg * pi / 180.0;
    let p2 = p2deg * pi / 180.0;
    a1 * sin(t * f1 + p1) * exp(-d1 * t) + a2 * sin(t * f2 + p2) * exp(-d2 * t)
}

fn variation(z, p) {
    let time = to_float(p.time);
    let a1 = to_float(p.a1);
    let f1 = p.f1;
    let p1 = to_float(p.p1);
    let d1 = p.d1;
    let a2 = to_float(p.a2);
    let f2 = p.f2;
    let p2 = to_float(p.p2);
    let d2 = p.d2;
    let a3 = to_float(p.a3);
    let f3 = p.f3;
    let p3 = to_float(p.p3);
    let d3 = p.d3;
    let a4 = to_float(p.a4);
    let f4 = p.f4;
    let p4 = to_float(p.p4);
    let d4 = p.d4;
    let t = rand() * time;
    let x = mo_harmonograph(t, a1, f1, p1, d1, a2, f2, p2, d2) / 180.0;
    let y = mo_harmonograph(t, a3, f3, p3 + 90.0, d3, a4, f4, p4 + 90.0, d4) / 180.0;
    complex(x, y)
}
