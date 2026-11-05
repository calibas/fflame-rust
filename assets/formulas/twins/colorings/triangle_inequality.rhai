// Triangle Inequality, the built-in colouring's twin: at each step,
// where |z| falls between the triangle inequality's bounds built from
// |z - c| and |c|, averaged over the orbit.

const PARAMS = #{
    scale: #{
        value: 1.0, min: 0.01, max: 20.0,
        caption: "Scale",
        hint: "Palette distance per unit of averaged TIA value.",
    },
    deband: #{
        value: 0, choices: ["Off", "On"],
        caption: "Deband",
        hint: "Blends the mean with and without the orbit's last term by how far through its last step the orbit escaped, so the colour moves smoothly across an iteration boundary instead of stepping (Ultra Fractal's smoothing of its Triangle Inequality Average). Works with the escape radius: the larger the bailout, the more terms there are to blend.",
    },
};

const INFO = #{ title: "Triangle Inequality" };

// The mean of the terms, or with Deband the template's debanded mean:
// the mean without the last term and with it, blended by how far through
// its last step the orbit escaped.
fn debanded(total, count, last, on, f) {
    let mean = total / max(count, 1.0);
    if on && !(count < 1.5) {
        let prev = (total - last) / (count - 1.0);
        prev + (mean - prev) * f
    } else {
        mean
    }
}

// The sum of the terms, how many, and the last of them.
fn start(z, p) {
    #{ total: 0.0, count: 0.0, last: 0.0 }
}

fn step(s, z, p, ctx) {
    let c = ctx.step_c;
    let x = (z - c).abs();
    let mc = c.abs();
    let lo = abs(x - mc);
    let span = x + mc - lo;
    if !(span < 1e-12) {
        let t = min(max((z.abs() - lo) / span, 0.0), 1.0);
        s.total = s.total + t;
        s.count = s.count + 1.0;
        s.last = t;
    }
    s
}

fn finish(s, z, p, ctx) {
    debanded(s.total, s.count, s.last, p.deband == 1, ctx.escape_fraction) * p.scale
}
