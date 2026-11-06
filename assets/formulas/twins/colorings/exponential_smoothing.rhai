// Exponential Smoothing, the built-in colouring's twin (Ultra Fractal):
// the sum of exp(-|z|) over a diverging orbit, of exp(-1/|z - z_prev|)
// over a converging one. Each term vanishes as the orbit runs away or
// settles, so the sum is continuous with no bands.

const PARAMS = #{
    mode: #{
        value: 2, choices: ["Diverging", "Converging", "Both"],
        caption: "Orbits",
        hint: "Which sum to keep: diverging orbits (exp(-|z|)), converging ones (exp(-1/|z - z_prev|)), or both, for formulas that do either.",
    },
    scale: #{
        value: 0.5, min: 0.001, max: 20.0,
        caption: "Scale",
        hint: "Palette distance per unit of the sum.",
    },
};

const INFO = #{ title: "Exponential Smoothing" };

fn start(z, p) {
    #{ total: 0.0 }
}

fn step(s, z, p, ctx) {
    let t = 0.0;
    if p.mode != 1 {
        t = t + exp(-z.abs());
    }
    if p.mode != 0 {
        // A settled step is 1/0 away from counting: keep it finite and
        // let exp take it to zero.
        t = t + exp(-1.0 / max((z - ctx.prev).abs(), 1e-30));
    }
    s.total = s.total + t;
    s
}

fn finish(s, z, p) {
    s.total * p.scale
}
