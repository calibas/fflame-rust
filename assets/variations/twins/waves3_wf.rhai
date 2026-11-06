/// WF waves3 (squared-trig perturbation) — same structure as `waves2_wf`
/// but the trig terms are squared (`trig²`), producing always-positive
/// waves with double the apparent frequency.
///
/// # Authors
/// - Joel Faber

// Waves 3 WF, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    scalex: #{
        value: 0.25, min: -10.0, max: 10.0, free: true,
        caption: "Scale X",
        hint: "X-axis wave amplitude.",
    },
    scaley: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "Scale Y",
        hint: "Y-axis wave amplitude.",
    },
    freqx: #{
        value: 1.5707964, min: -100.0, max: 100.0, free: true,
        caption: "Freq X",
        hint: "X-axis wave frequency (applied to the Y input).",
    },
    freqy: #{
        value: 0.7853982, min: -100.0, max: 100.0, free: true,
        caption: "Freq Y",
        hint: "Y-axis wave frequency (applied to the X input).",
    },
    use_cos_x: #{
        value: true,
        caption: "Use Cos X",
        hint: "When on, use cosine on the X axis. When off, sine.",
    },
    use_cos_y: #{
        value: false,
        caption: "Use Cos Y",
        hint: "When on, use cosine on the Y axis. When off, sine.",
    },
    dampx: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Damp X",
        hint: "X-axis exponential damping factor — the output is multiplied by `exp(dampx)` (or 1 when `|dampx|` is tiny).",
    },
    dampy: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Damp Y",
        hint: "Y-axis exponential damping factor — same as dampx but for Y.",
    },
};

const INFO = #{ title: "Waves 3 WF", name: "rhai_waves3_wf", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    #{ dx: (if abs(p.dampx) < 1.0e-6 { 1.0 } else { exp(p.dampx) }), dy: (if abs(p.dampy) < 1.0e-6 { 1.0 } else { exp(p.dampy) }) }
}

fn variation(z, p) {
    let scalex = p.scalex;
    let scaley = p.scaley;
    let freqx = p.freqx;
    let freqy = p.freqy;
    let use_cos_x = to_int(to_float(p.use_cos_x));
    let use_cos_y = to_int(to_float(p.use_cos_y));
    let dx = p.dx;
    let dy = p.dy;
    let tx_base = (if use_cos_x == 1 { cos(z.im * freqx) } else { sin(z.im * freqx) });
    let ty_base = (if use_cos_y == 1 { cos(z.re * freqy) } else { sin(z.re * freqy) });
    let tx = tx_base * tx_base;
    let ty = ty_base * ty_base;
    let nx = (z.re + dx * scalex * tx) * dx;
    let ny = (z.im + dy * scaley * ty) * dy;
    complex(nx, ny)
}
