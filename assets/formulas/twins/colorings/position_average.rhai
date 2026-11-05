// Position Average, the built-in colouring's twin: the orbit's average
// position, as its angle or its distance from the origin.

const PARAMS = #{
    mode: #{
        value: 0, choices: ["Angle", "Distance"],
        caption: "Component",
        hint: "0: angle of the average position (carries the fold seams), 1: its distance from the origin.",
    },
    scale: #{
        value: 1.0, min: 0.01, max: 64.0,
        caption: "Scale",
        hint: "Palette distance per unit. A whole turn of the angle spans the palette once at 1.0, but the average position often sweeps only a narrow arc across a given view -- a twentieth of a turn is typical on Origami -- so raising this is how the structure becomes visible.",
    },
    deband: #{
        value: 0, choices: ["Off", "On"],
        caption: "Deband",
        hint: "Blends the mean with and without the orbit's last term by how far through its last step the orbit escaped, so the colour moves smoothly across an iteration boundary instead of stepping (Ultra Fractal's smoothing of its Triangle Inequality Average). Works with the escape radius: the larger the bailout, the more terms there are to blend.",
    },
};

const INFO = #{ title: "Position Average", pick: #{ deband: 1 } };

// The running sum of the orbit's positions, and the last of them.
fn start(z, p) {
    #{ total: complex(0.0, 0.0), last: complex(0.0, 0.0) }
}

fn step(s, z, p) {
    s.total = s.total + z;
    s.last = z;
    s
}

fn finish(s, z, p, ctx) {
    let n = max(to_float(ctx.numiter), 1.0);
    let avg = complex(s.total.re / n, s.total.im / n);
    let avg = if p.deband == 1 && n > 1.5 {
        let prev = complex((s.total.re - s.last.re) / (n - 1.0), (s.total.im - s.last.im) / (n - 1.0));
        let f = ctx.escape_fraction;
        complex(prev.re + (avg.re - prev.re) * f, prev.im + (avg.im - prev.im) * f)
    } else {
        avg
    };
    if p.mode == 1 {
        avg.abs() * p.scale
    } else if avg.re * avg.re + avg.im * avg.im < 1e-30 {
        // An average of exactly zero has no angle.
        0.0
    } else {
        (avg.arg() * 0.159154943 + 0.5) * p.scale
    }
}
