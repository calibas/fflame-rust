// Orbit Trap, the built-in colouring's twin: how close the orbit comes
// to a trap -- a point, the axes' cross, the unit circle, a logarithmic
// spiral (golden by default), or a ring, box, line or diamond of the
// given radius -- at the trap's centre and rotation, reduced over the
// orbit: the closest approach, the farthest, the mean, or the first
// approach within a threshold.

const PARAMS = #{
    shape: #{
        value: 0,
        choices: ["Point", "Cross (axes)", "Unit circle", "Logarithmic spiral", "Ring", "Box", "Line", "Diamond"],
        caption: "Trap shape",
        hint: "The shape the orbit is measured against, at the trap's centre and rotation: a point, the axes' cross, the unit circle, a logarithmic spiral (golden by default), or a ring, box, line or diamond of the given radius.",
    },
    scale: #{
        value: 1.0, min: 0.01, max: 20.0,
        caption: "Scale",
        hint: "Palette distance per unit of trap distance.",
    },
    growth: #{
        value: 1.618034, min: 1.05, max: 8.0,
        caption: "Spiral growth",
        hint: "Spiral shape 3 only: the factor the spiral widens by per QUARTER turn. The default is the golden ratio, which makes it the golden spiral; 2 gives a doubling-per-quarter-turn spiral, and values near 1 wind tightly.",
    },
    center_re: #{
        value: 0.0, min: -4.0, max: 4.0,
        caption: "Centre (re)",
        hint: "Where the trap sits in the plane.",
    },
    center_im: #{
        value: 0.0, min: -4.0, max: 4.0,
        caption: "Centre (im)",
        hint: "Where the trap sits in the plane.",
    },
    rotation: #{
        value: 0.0, min: -180.0, max: 180.0,
        caption: "Rotation",
        hint: "The trap's rotation in degrees, about its centre.",
    },
    mode: #{
        value: 0, choices: ["Closest", "Farthest", "Average", "First within threshold"],
        caption: "Reduction",
        hint: "Which of the orbit's distances colours the pixel: the closest approach, the farthest, their mean, or the first that came within the threshold (the closest if none did).",
    },
    threshold: #{
        value: 0.1, min: 0.0001, max: 4.0,
        caption: "Threshold",
        hint: "How close an iterate must come to count as caught, for the First reduction.",
    },
    radius: #{
        value: 1.0, min: 0.001, max: 4.0,
        caption: "Radius",
        hint: "The size of the ring, box and diamond, and the line's offset from the centre.",
    },
};

const INFO = #{ title: "Orbit Trap" };

// The closest, the farthest, the sum, and the first within the
// threshold (-1 until one comes).
fn start(z, p) {
    #{ closest: 1e30, farthest: 0.0, total: 0.0, first: -1.0 }
}

fn step(s, z, p) {
    // Into the trap's frame: about its centre, turned by its rotation.
    // Unturned at rotation 0, not turned by sin 0 = 0: an infinite
    // iterate times zero is NaN, and a saved trap draws as it did.
    let a = -p.rotation * 0.017453292;
    let ca = cos(a);
    let sa = sin(a);
    let zc = z - complex(p.center_re, p.center_im);
    let w = if p.rotation == 0.0 { zc } else { complex(zc.re * ca - zc.im * sa, zc.re * sa + zc.im * ca) };
    let rad = p.radius;
    let d = if p.shape == 0 {
        w.abs()
    } else if p.shape == 1 {
        min(abs(w.re), abs(w.im))
    } else if p.shape == 2 {
        abs(w.abs() - 1.0)
    } else if p.shape == 4 {
        abs(w.abs() - rad)
    } else if p.shape == 5 {
        abs(max(abs(w.re), abs(w.im)) - rad)
    } else if p.shape == 6 {
        abs(w.im - rad)
    } else if p.shape == 7 {
        abs(abs(w.re) + abs(w.im) - rad)
    } else {
        // The logarithmic spiral: how many turns out along it the point
        // sits, its distance to the nearest arm doubled to span 0..1.
        // No finite distance at the origin, where it winds infinitely:
        // a huge one, which leaves every reduction untouched.
        let r2 = w.re * w.re + w.im * w.im;
        if r2 < 1e-30 {
            1e30
        } else {
            let g = max(p.growth, 1.05);
            let turns = ln(r2) / (8.0 * ln(g)) - w.arg() * 0.159154943;
            2.0 * abs(turns - round_even(turns))
        }
    };
    if d < 1e29 {
        if s.first < 0.0 && d < p.threshold {
            s.first = d;
        }
        s.closest = min(s.closest, d);
        s.farthest = max(s.farthest, d);
        s.total = s.total + d;
    }
    s
}

fn finish(s, z, p, ctx) {
    let d = if p.mode == 1 {
        s.farthest
    } else if p.mode == 2 {
        s.total / max(to_float(ctx.numiter), 1.0)
    } else if p.mode == 3 && s.first >= 0.0 {
        s.first
    } else {
        s.closest
    };
    d * p.scale
}
