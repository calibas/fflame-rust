// Curvature Average, the built-in colouring's twin (techmatt): the mean
// turn between consecutive steps of the orbit, |arg of the one step over
// the one before| / pi, in 0..1.

const PARAMS = #{
    scale: #{
        value: 1.0, min: 0.01, max: 20.0,
        caption: "Scale",
        hint: "Palette distance per unit of averaged turn (a full reversal is 1).",
    },
    deband: #{
        value: 0, choices: ["Off", "On"],
        caption: "Deband",
        hint: "Blends the mean with and without the orbit's last term by how far through its last step the orbit escaped, so the colour moves smoothly across an iteration boundary instead of stepping.",
    },
};

const INFO = #{ title: "Curvature Average", bailout: 1.0e4, pick: #{ deband: 1 } };

// The sum of the turns, the last of them, and the iterate two steps back
// (none yet: 1e30).
fn start(z, p) {
    #{ total: 0.0, last: 0.0, before: complex(1e30, 0.0) }
}

fn step(s, z, p, ctx) {
    let zp = ctx.prev;
    if !(s.before.re > 1e29) {
        let a = z - zp;
        let b = zp - s.before;
        // A step of zero has no direction.
        if !(a.re * a.re + a.im * a.im < 1e-30 || b.re * b.re + b.im * b.im < 1e-30) {
            // a conj(b): its angle is the turn from b to a.
            let q = complex(a.re * b.re + a.im * b.im, a.im * b.re - a.re * b.im);
            let t = abs(q.arg()) * 0.31830988;
            s.total = s.total + t;
            s.last = t;
        }
    }
    s.before = zp;
    s
}

fn finish(s, z, p, ctx) {
    // A turn at every step but the first: n - 1 of them.
    let count = max(to_float(ctx.numiter) - 1.0, 1.0);
    let mean = s.total / count;
    let mean = if p.deband == 1 && count > 1.5 {
        let prev = (s.total - s.last) / (count - 1.0);
        let f = ctx.escape_fraction;
        prev + (mean - prev) * f
    } else {
        mean
    };
    mean * p.scale
}
