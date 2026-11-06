/// Random-blend spherical/linear — each iteration flips a weighted coin:
/// with probability `d_spher_weight` applies a spherical inversion `(x, y)
/// / (x²+y²)`, otherwise passes through unchanged. Smoothly blends the two
/// effects across many samples.
///
/// # Authors
/// - Tatyana Zabanova

// D-Spherical, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    d_spher_weight: #{
        value: 0.5, min: 0.0, max: 1.0, free: true,
        caption: "D Spher Weight",
        hint: "Probability of applying the spherical inversion (vs linear pass-through).",
    },
};

const INFO = #{ title: "D-Spherical", name: "rhai_d_spherical", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let d_spher_weight = p.d_spher_weight;
    if (rand() < d_spher_weight) {
        let r2 = max(1.0e-30, z.re * z.re + z.im * z.im);
        return complex(z.re / r2, z.im / r2);
    }
    z
}
