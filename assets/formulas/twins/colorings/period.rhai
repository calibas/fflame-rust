// Period, the built-in colouring's twin: a pixel whose orbit settles
// into a cycle by the cycle's length, an escaped one by its escape count,
// an undetected interior at the palette's origin.

const PARAMS = #{
    scale: #{
        value: 0.1, min: 0.000001, max: 1.0, exponential: true,
        caption: "Period scale",
        hint: "Palette distance per unit of detected cycle length.",
    },
    escape_scale: #{
        value: 0.02, min: 0.0, max: 1.0,
        caption: "Escape scale",
        hint: "Palette distance per iteration for pixels that escape instead of cycling.",
    },
};

const INFO = #{ title: "Period" };

fn finish(s, z, p, ctx) {
    if ctx.period > 0 {
        to_float(ctx.period) * p.scale
    } else if ctx.escaped {
        to_float(ctx.numiter) * p.escape_scale
    } else {
        0.0
    }
}
