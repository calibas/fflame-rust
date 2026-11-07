/// Y-flip inside a damped cosine band — computes a (possibly damped)
/// cosine envelope `t(x) = amp · cos(2π·freq·x) · exp(−|x|·damping) +
/// separation`. Points whose `|y| ≤ t(x)` get their Y coordinate
/// flipped; others pass through. Produces an oscilloscope-trace-like
/// masking pattern.
///
/// # Authors
/// - Apophysis Plugin Pack

// Oscilloscope, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    separation: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Separation",
        hint: "Vertical offset of the band threshold, added to the cosine envelope.",
    },
    frequency: #{
        value: 3.1415927, min: -100.0, max: 100.0, free: true,
        caption: "Frequency",
        hint: "Cosine frequency (multiplied by 2π internally).",
    },
    amplitude: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Amplitude",
        hint: "Cosine amplitude.",
    },
    damping: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Damping",
        hint: "Exponential damping rate. Values near zero disable the damping term.",
    },
};

const INFO = #{ title: "Oscilloscope", name: "rhai_oscilloscope", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    #{ tpf: 6.28318530717959 * p.frequency }
}

fn variation(z, p) {
    let separation = p.separation;
    let amplitude = p.amplitude;
    let damping = p.damping;
    let tpf = p.tpf;
    let no_damping = abs(damping) <= 1.0e-6;
    let t = 0.0;
    if (no_damping) {
        t = amplitude * cos(tpf * z.re) + separation;
    } else {
        t = amplitude * exp(-abs(z.re) * damping) * cos(tpf * z.re) + separation;
    }
    if (abs(z.im) <= t) {
        return complex(z.re, -z.im);
    }
    z
}
