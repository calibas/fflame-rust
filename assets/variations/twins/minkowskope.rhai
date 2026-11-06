/// Minkowski-question-mark oscilloscope — like
/// `oscilloscope`/`oscilloscope2` but uses Minkowski's question-mark
/// function as the wave shape instead of a cosine. Builds a wave `t(x) =
/// amplitude · minkowski_cosine(0.5·freqx·x + perturbation ·
/// minkowski_sine(0.5·freqy·y)) [· exp(−|x|·damping)] + separation`, then
/// flips both X and Y signs when `|y| ≤ t(x)`. Negative `frequencyx`
/// enables an alternative wave variant where the Minkowski result is offset
/// by its input.
///
/// # Authors
/// - Apophysis Plugin Pack
/// - DarkBeam

// Minkowskope, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    separation: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "Separation",
        hint: "Vertical offset of the band threshold.",
    },
    frequencyx: #{
        value: 1.5, min: -10.0, max: 10.0, free: true,
        caption: "Frequency X",
        hint: "X-axis Minkowski-cosine frequency (multiplied by 0.5 internally). Negative values enable the `alt_wave` variant.",
    },
    frequencyy: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Frequency Y",
        hint: "Y-axis Minkowski-sine frequency for the perturbation term (multiplied by 0.5 internally).",
    },
    amplitude: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "Amplitude",
        hint: "Wave amplitude.",
    },
    perturbation: #{
        value: 0.3, min: -10.0, max: 10.0, free: true,
        caption: "Perturbation",
        hint: "Magnitude of the Y-driven X-perturbation.",
    },
    damping: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Damping",
        hint: "Exponential damping rate. Near-zero disables the damping term.",
    },
};

const INFO = #{ title: "Minkowskope", name: "rhai_minkowskope", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    #{ tpf: 0.5 * p.frequencyx, tpf2: 0.5 * p.frequencyy }
}

fn minkowskope_mink(xv) {
    let p = 0.0;
    let q = 1.0;
    let r = p + 1.0;
    let s = 1.0;
    let d = 1.0;
    let y = p;
    for it in 0..20 {
        d = d * 0.5;
        let m = p + r;
        let n = q + s;
        let safe_n = (if abs(n) < 1.0e-30 { 1.0e-30 } else { n });
        if (xv < (m / safe_n)) {
            r = m;
            s = n;
        } else {
            y = y + d;
            p = m;
            q = n;
        }
    }
    y + d
}

fn minkowskope_sine(xv, alt_wave) {
    let lp = abs(xv) - to_float(floor(abs(xv) * 0.25)) * 4.0;
    let p_val = abs(xv) - to_float(floor(abs(xv) * 0.5)) * 2.0;
    if (p_val > 1.0) {
        p_val = 2.0 - p_val;
    }
    let mink = 0.0;
    if (alt_wave) {
        mink = minkowskope_mink(p_val) - p_val;
    } else {
        mink = minkowskope_mink(p_val);
    }
    let cond_xor = (lp < 2.0) != (xv > 0.0);
    (if cond_xor { mink } else { -mink })
}

fn minkowskope_cosine(xv, alt_wave) {
    minkowskope_sine(xv - 1.0, alt_wave)
}

fn variation(z, p) {
    let separation = p.separation;
    let frequencyx = p.frequencyx;
    let amplitude = p.amplitude;
    let perturbation = p.perturbation;
    let damping = p.damping;
    let tpf = p.tpf;
    let tpf2 = p.tpf2;
    let no_damping = abs(damping) <= 1.0e-9;
    let alt_wave = frequencyx <= 0.0;
    let pt = perturbation * minkowskope_sine(tpf2 * z.im, alt_wave);
    let t = 0.0;
    if (no_damping) {
        t = amplitude * minkowskope_cosine(tpf * z.re + pt, alt_wave) + separation;
    } else {
        t = amplitude * exp(-abs(z.re) * damping) * minkowskope_cosine(tpf * z.re + pt, alt_wave) + separation;
    }
    let inside = abs(z.im) <= t;
    let s = (if inside { -1.0 } else { 1.0 });
    complex(s * z.re, s * z.im)
}
