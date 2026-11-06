/// Randomized fractal-carpet warp — splits the input into a fractional part
/// `frac(|coord|)` (toward-zero rounding) and adds a per-axis random `±1`
/// cell offset, then passes through the transform's pre-affine `(a·x + b·y
/// + e, c·x + d·y + f)`. Produces a recursive Sierpinski-carpet-style
/// scatter pattern when combined with self-similar transforms.
///
/// Direct-color: blends the previous color register toward the random cell
/// parity, weighted by `H = 0.1·origin` (Java: `color = fmod(|color·0.5·(1
/// + h) + x0_xor_y0·(1 − h)·0.5|, 1)` where `x0_xor_y0` is the *integer*
/// XOR of the two ±1 cell signs — 0 when equal, −2 when different).

// DC Carpet, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    origin: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Origin",
        hint: "Color-blend strength: `H = 0.1·origin` weights how strongly the random cell parity pulls the color register. Visible color requires the transform's Direct Color slider > 0.",
    },
    iterations: #{
        value: 5, min: 1, max: 100,
        caption: "Iterations",
        hint: "Unused in the body — preserved as a parameter for cpp parity.",
    },
};

const INFO = #{ title: "DC Carpet", name: "rhai_dc_carpet", category: "advanced_2d", phase: "any" };

fn dc_carpet_signum(v) {
    if (v < 0.0) {
        return -1.0;
    }
    1.0
}

fn dc_carpet_rndsign() {
    if (rand() < 0.5) {
        return -1.0;
    }
    1.0
}

fn variation(z, p, ctx) {
    let color = ctx.color;
    let x0 = dc_carpet_rndsign();
    let y0 = dc_carpet_rndsign();
    let fx = abs(z.re) - to_float(trunc(abs(z.re)));
    let fy = abs(z.im) - to_float(trunc(abs(z.im)));
    let x = dc_carpet_signum(z.re) * fx + x0;
    let y = dc_carpet_signum(z.im) * fy + y0;
    let xor_f = (if x0 == y0 { 0.0 } else { -2.0 });
    let h_cap = 0.1 * p.origin;
    let h = -h_cap + (1.0 - xor_f) * h_cap;
    let craw = abs(color * 0.5 * (1.0 + h) + xor_f * (1.0 - h) * 0.5);
    color = craw - to_float(floor(craw));
    #{ z: complex(ctx.a * x + ctx.b * y + ctx.e, ctx.c * x + ctx.d * y + ctx.f), color: color }
}
