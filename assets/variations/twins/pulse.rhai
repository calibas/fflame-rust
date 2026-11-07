/// Sine-wave additive distortion — adds `scale · sin(coord · freq)` to each
/// axis. Per-axis frequency and amplitude controls.

// Pulse, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    freqx: #{
        value: 2.0, min: -50.0, max: 50.0, free: true,
        caption: "Freq X",
        hint: "X-axis sine frequency.",
    },
    freqy: #{
        value: 2.0, min: -50.0, max: 50.0, free: true,
        caption: "Freq Y",
        hint: "Y-axis sine frequency.",
    },
    scalex: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Scale X",
        hint: "X-axis sine amplitude.",
    },
    scaley: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Scale Y",
        hint: "Y-axis sine amplitude.",
    },
};

const INFO = #{ title: "Pulse", name: "rhai_pulse", category: "advanced_2d", phase: "any", z: "preserve_z" };

fn variation(z, p) {
    let freqx = p.freqx;
    let freqy = p.freqy;
    let scalex = p.scalex;
    let scaley = p.scaley;
    complex(z.re + scalex * sin(z.re * freqx), z.im + scaley * sin(z.im * freqy))
}

fn variation3(v, p) {
    let freqx = p.freqx;
    let freqy = p.freqy;
    let scalex = p.scalex;
    let scaley = p.scaley;
    vec3(v.x + scalex * sin(v.x * freqx), v.y + scaley * sin(v.y * freqy), v.z)
}
