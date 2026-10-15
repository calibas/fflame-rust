// A Rhai colouring using every construct a colouring has: state from
// `start`, a `step` with the formula's z, `finish` ending in an `if`
// whose arms give a map and an index, the pixel and the iteration count
// from the context, `prepare`, a helper, a loop.

const PARAMS = #{
    radius: #{ value: 0.5, min: 0.0, caption: "Radius" },
    k: #{ value: 3, choices: ["One", "Two", "Three", "Four"] },
    edge: #{ value: true },
};

const INFO = #{ title: "Every colouring construct", region: "both" };

fn prepare(p) {
    #{ r2: p.radius * p.radius }
}

fn ring(w, r) {
    (w.abs() - r).abs()
}

fn start(z, p, ctx) {
    let s = #{ near: ring(z, p.radius), sum: 0.0, first: z + ctx.pixel * 0.0 };
    s.count = 0;
    s
}

fn step(s, z, p) {
    let d = ring(z, p.radius);
    if d < s.near {
        s.near = d;
    }
    let i = 0;
    while i < p.k {
        s.sum += z.norm_sqr() * 0.001;
        i += 1;
    }
    s.count += 1;
    s
}

fn finish(s, z, p, ctx) {
    let t = s.sum / to_float(s.count + 1) + to_float(ctx.numiter) * 0.01;
    if p.edge && s.near < 0.01 && z.norm_sqr() > p.r2 {
        #{ index: t, solid: s.first.re > 10.0 }
    } else if ctx.pixel.im > 0.0 {
        t + s.near
    } else {
        #{ solid: z.norm_sqr() < p.r2 }
    }
}
