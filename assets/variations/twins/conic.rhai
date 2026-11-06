// Conic, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    eccentricity: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Eccentricity",
        hint: "Conic eccentricity. 0 = circle, 1 = parabola, > 1 = hyperbola.",
    },
    holes: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Holes",
        hint: "Random shift offset subtracted from the per-iteration random. Larger = sparser pattern.",
    },
};

const INFO = #{ title: "Conic", name: "rhai_conic" };

fn variation(z, p, ctx) {
    let ecc = p.eccentricity;
    let holes = p.holes;
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let s = sqrt(z.re * z.re + z.im * z.im) + 1.0e-30;
    let ct = z.re / s;
    let denom_a = 1.0 + ecc * ct;
    let safe_denom = (if abs(denom_a) < 1.0e-30 { 1.0e-30 } else { denom_a });
    let r = w * (rand() - holes) * ecc / safe_denom / s;
    complex(r * z.re * inv_w, r * z.im * inv_w)
}
