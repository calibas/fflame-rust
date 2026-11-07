/// 2D extension of `oscilloscope` — adds a Y-driven sinusoidal perturbation
/// `perturbation · sin(2π·freqy·y)` to the X-axis cosine argument, and
/// flips BOTH X and Y when the input falls inside the band (vs
/// `oscilloscope`, which only flips Y). Produces a 2D oscilloscope-trace
/// masking pattern.
///
/// # Authors
/// - Apophysis Plugin Pack
/// - DarkBeam

// Oscilloscope 2, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    separation: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Separation",
        hint: "Vertical offset of the band threshold.",
    },
    frequencyx: #{
        value: 3.1415927, min: -100.0, max: 100.0, free: true,
        caption: "Frequency X",
        hint: "X-axis cosine frequency (multiplied by 2π internally).",
    },
    frequencyy: #{
        value: 3.1415927, min: -100.0, max: 100.0, free: true,
        caption: "Frequency Y",
        hint: "Y-axis sine frequency for the perturbation term (multiplied by 2π).",
    },
    amplitude: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Amplitude",
        hint: "Cosine amplitude.",
    },
    perturbation: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Perturbation",
        hint: "Magnitude of the Y-driven X-perturbation.",
    },
    damping: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Damping",
        hint: "Exponential damping rate. Values near zero disable the damping term.",
    },
};

const INFO = #{ title: "Oscilloscope 2", name: "rhai_oscilloscope2", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let two_pi = 6.28318530717959;
    #{ tpf: two_pi * p.frequencyx, tpf2: two_pi * p.frequencyy }
}

fn variation(z, p) {
    let separation = p.separation;
    let amplitude = p.amplitude;
    let perturbation = p.perturbation;
    let damping = p.damping;
    let tpf = p.tpf;
    let tpf2 = p.tpf2;
    let no_damping = abs(damping) <= 1.0e-6;
    let pt = perturbation * sin(tpf2 * z.im);
    let t = 0.0;
    if (no_damping) {
        t = amplitude * cos(tpf * z.re + pt) + separation;
    } else {
        t = amplitude * exp(-abs(z.re) * damping) * cos(tpf * z.re + pt) + separation;
    }
    if (abs(z.im) <= t) {
        return complex(-z.re, -z.im);
    }
    z
}
