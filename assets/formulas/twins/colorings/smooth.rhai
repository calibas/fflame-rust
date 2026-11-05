// Smooth Iteration, the built-in colouring's twin: the smooth (continuous)
// iteration count, mu = n + 1 - log_p(log2 |z|), the fraction a log in
// the formula's degree at infinity p, which cancels the banding of the
// discrete count.

const PARAMS = #{
    scale: #{
        value: 0.05, min: 0.000001, max: 1.0, exponential: true,
        caption: "Scale",
        hint: "Palette distance per iteration. Smaller = broader gradient. Deep views need very small values -- the slider reaches 1e-6.",
    },
};

const INFO = #{ title: "Smooth Iteration", region: "outside" };

fn finish(s, z, p, ctx) {
    // |z|^2 past a bailout below 1 is the corner the max guards.
    let r2 = max(z.re * z.re + z.im * z.im, 1.0000001);
    let ll = log2(0.5 * log2(r2));
    // Exactly the old count at degree 2: no divide by a log2(2.0).
    let frac = if ctx.degree == 2.0 { ll } else { ll / log2(ctx.degree) };
    (to_float(ctx.numiter) + 1.0 - frac) * p.scale
}
