/// Fractal disc — combines an inverse-radius cosine wave `cos(2π / (r +
/// ashift) + xshift)`, an angular term `(atan2(y,x)/π + rshift) / 2`, and
/// four blending weights. Each `term_i` controls a different contribution
/// to the output (pure-prx, x-scaled-prx, x-scaled-pr, pass-through),
/// producing rich layered disc patterns.
///
/// # Authors
/// - CozyG

// F-Disc, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    ashift: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "A shift",
        hint: "Radial denominator offset added to `sqrt(x² + y²)`.",
    },
    rshift: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "R shift",
        hint: "Angular offset added to `atan2(y, x) / π`.",
    },
    xshift: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "X shift",
        hint: "Phase offset on the X cosine wave.",
    },
    yshift: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Y shift",
        hint: "Phase offset on the Y sine wave.",
    },
    term1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Term 1",
        hint: "Weight on the pure `prx`/`pry` (radius × axis-wave) contribution.",
    },
    term2: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Term 2",
        hint: "Weight on the input-scaled `x·prx`/`y·pry` contribution.",
    },
    term3: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Term 3",
        hint: "Weight on the `x·pr`/`y·pr` (input-scaled radius) contribution.",
    },
    term4: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Term 4",
        hint: "Weight on the pass-through `x`/`y` contribution.",
    },
};

const INFO = #{ title: "F-Disc", name: "rhai_fdisc", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let ashift = p.ashift;
    let rshift = p.rshift;
    let xshift = p.xshift;
    let yshift = p.yshift;
    let term1 = p.term1;
    let term2 = p.term2;
    let term3 = p.term3;
    let term4 = p.term4;
    let two_pi = 6.28318530717959;
    let inv_pi = 0.31830988618379;
    let s = sqrt(z.re * z.re + z.im * z.im) + 1.0e-30;
    let afactor = two_pi / (s + ashift);
    let r = (atan2(z.im, z.re) * inv_pi + rshift) * 0.5;
    let xfactor = cos(afactor + xshift);
    let yfactor = sin(afactor + yshift);
    let pr = r;
    let prx = pr * xfactor;
    let pry = pr * yfactor;
    let fx = term1 * prx + term2 * z.re * prx + term3 * z.re * pr + term4 * z.re;
    let fy = term1 * pry + term2 * z.im * pry + term3 * z.im * pr + term4 * z.im;
    complex(fx, fy)
}
