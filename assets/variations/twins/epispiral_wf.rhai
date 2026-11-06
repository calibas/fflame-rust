/// Epispiral curve — emits a point on the epispiral `r = 0.5 / cos(waves ·
/// a)`. The curve produces `waves` symmetric petals radiating from the
/// origin. Returns `(0, 0)` at the cos-zero singularities.

// Epispiral WF, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    waves: #{
        value: 4.0, min: -50.0, max: 50.0, free: true,
        caption: "Waves",
        hint: "Number of petals (frequency of the cosine denominator).",
    },
};

const INFO = #{ title: "Epispiral WF", name: "rhai_epispiral_wf", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let waves = p.waves;
    let a = atan2(z.re, z.im);
    let d = cos(waves * a);
    if (abs(d) < 1.0e-30) {
        return complex(0.0, 0.0);
    }
    let r = 0.5 / d;
    complex(sin(a) * r, cos(a) * r)
}
