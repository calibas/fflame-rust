// Orbit Average, the built-in colouring's twin: the Kali glow, the mean
// over the orbit of the distance to the axes, min(|Re z|, |Im z|).

const PARAMS = #{
    scale: #{
        value: 1.0, min: 0.01, max: 20.0,
        caption: "Scale",
        hint: "Palette distance per unit of averaged trap value.",
    },
    deband: #{
        value: 0, choices: ["Off", "On"],
        caption: "Deband",
        hint: "Blends the mean with and without the orbit's last term by how far through its last step the orbit escaped, so the colour moves smoothly across an iteration boundary instead of stepping (Ultra Fractal's smoothing of its Triangle Inequality Average). Works with the escape radius: the larger the bailout, the more terms there are to blend.",
    },
};

const INFO = #{ title: "Orbit Average", pick: #{ deband: 1 } };

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

fn step(s, z, p) {
    let t = min(abs(z.re), abs(z.im));
    s.total = s.total + t;
    s.count = s.count + 1.0;
    s.last = t;
    s
}

fn finish(s, z, p, ctx) {
    debanded(s.total, s.count, s.last, p.deband == 1, ctx.escape_fraction) * p.scale
}
