// Rose WF, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    amp: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "Amplitude",
        hint: "Radial amplitude (multiplies the cosine).",
    },
    waves: #{
        value: 4, min: -50, max: 50,
        caption: "Waves",
        hint: "Number of petals (`waves` petals if odd, `2·waves` if even).",
    },
    filled: #{
        value: false,
        caption: "Filled",
        hint: "1 = fill the curve interior; 0 = trace only the outline.",
    },
};

const INFO = #{ title: "Rose WF", name: "rhai_rose_wf" };

fn variation(z, p) {
    let amp = p.amp;
    let waves = to_float(p.waves);
    let filled = to_int(to_float(p.filled));
    let a = atan2(z.re, z.im);
    let r = amp * cos(waves * a);
    if (filled == 1) {
        r = r * rand();
    }
    complex(sin(a) * r, cos(a) * r)
}
