// Escape Count, the built-in colouring's twin: the discrete escape
// count, the classic banded look, `n * scale`.

const PARAMS = #{
    scale: #{
        value: 0.05, min: 0.000001, max: 1.0, exponential: true,
        caption: "Scale",
        hint: "Palette distance per iteration band. Smaller = broader bands. Deep views need very small values -- the slider reaches 1e-6.",
    },
};

const INFO = #{ title: "Escape Count", region: "outside" };

fn finish(s, z, p, ctx) {
    to_float(ctx.numiter) * p.scale
}
