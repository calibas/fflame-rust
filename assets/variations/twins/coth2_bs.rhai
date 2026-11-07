/// Parameterized Coth. At defaults (2.0), reduces to Coth.
///
/// # Authors
/// - cothe
/// - Brad Stefanov

// Coth2 BS, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    x1: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "X1",
        hint: "Scales the argument of `sin(x)` in the internal computation.",
    },
    x2: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "X2",
        hint: "Scales the argument of `cos(x)` in the internal computation.",
    },
    y1: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "Y1",
        hint: "Scales the argument of `sinh(y)` in the internal computation.",
    },
    y2: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "Y2",
        hint: "Scales the argument of `cosh(y)` in the internal computation.",
    },
};

const INFO = #{ title: "Coth2 BS", name: "rhai_coth2_bs", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let x1 = p.x1;
    let x2 = p.x2;
    let y1 = p.y1;
    let y2 = p.y2;
    let s = sin(y1 * z.im);
    let c = cos(y2 * z.im);
    let sh = sinh(x1 * z.re);
    let ch = cosh(x2 * z.re);
    let d = ch - c;
    let inv = 1.0 / (if d == 0.0 { 1.0e-20 } else { d });
    complex(inv * sh, inv * s)
}
