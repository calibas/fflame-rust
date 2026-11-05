// Magnitude Average, the built-in colouring's twin: the mean of |z|
// over the orbit, less an offset -- the Ducks colouring.

const PARAMS = #{
    scale: #{
        value: 1.0, min: 0.01, max: 100.0,
        caption: "Scale",
        hint: "Palette distance per unit of averaged |z| (above the offset).",
    },
    offset: #{
        value: 0.0, min: -10.0, max: 10.0,
        caption: "Offset",
        hint: "Baseline subtracted from the mean before scaling. A Ducks julia field can span only ~0.2 around a large mean -- offset to the field's floor, then scale up, to stretch that range across the palette (the reference images normalize contrast this way).",
    },
    deband: #{
        value: 0, choices: ["Off", "On"],
        caption: "Deband",
        hint: "Blends the mean with and without the orbit's last term by how far through its last step the orbit escaped, so the colour moves smoothly across an iteration boundary instead of stepping (Ultra Fractal's smoothing of its Triangle Inequality Average). Works with the escape radius: the larger the bailout, the more terms there are to blend.",
    },
};

const INFO = #{ title: "Magnitude Average" };

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
    let t = z.abs();
    s.total = s.total + t;
    s.count = s.count + 1.0;
    s.last = t;
    s
}

fn finish(s, z, p, ctx) {
    (debanded(s.total, s.count, s.last, p.deband == 1, ctx.escape_fraction) - p.offset) * p.scale
}
