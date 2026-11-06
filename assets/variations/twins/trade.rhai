/// Two-disc swap — defines two discs (one at `(r1+d1, 0)` with radius `r1`,
/// one at `(-(r2+d2), 0)` with radius `r2`). Points inside the right disc
/// get warped to the corresponding position in the left disc, and vice
/// versa; points outside both pass through.
///
/// # Authors
/// - Michael Faber

// Trade, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    r1: #{
        value: 1.0, min: 0.001, max: 10.0, free: true,
        caption: "R1",
        hint: "Right-disc radius.",
    },
    d1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "D1",
        hint: "Right-disc center offset from origin — center sits at `(r1 + d1, 0)`.",
    },
    r2: #{
        value: 1.0, min: 0.001, max: 10.0, free: true,
        caption: "R2",
        hint: "Left-disc radius.",
    },
    d2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "D2",
        hint: "Left-disc center offset from origin — center sits at `(-(r2 + d2), 0)`.",
    },
};

const INFO = #{ title: "Trade", name: "rhai_trade", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    #{ c1: p.r1 + p.d1, c2: p.r2 + p.d2 }
}

fn variation(z, p) {
    let r1 = p.r1;
    let r2 = p.r2;
    let c1 = p.c1;
    let c2 = p.c2;
    if (z.re > 0.0) {
        let r = sqrt((c1 - z.re) * (c1 - z.re) + z.im * z.im);
        if (r <= r1) {
            r = r * r2 / max(1.0e-30, r1);
            let a = atan2(z.im, c1 - z.re);
            return complex(r * cos(a) - c2, r * sin(a));
        }
    } else {
        let r = sqrt((-c2 - z.re) * (-c2 - z.re) + z.im * z.im);
        if (r <= r2) {
            r = r * r1 / max(1.0e-30, r2);
            let a = atan2(z.im, -c2 - z.re);
            return complex(r * cos(a) + c1, r * sin(a));
        }
    }
    z
}
