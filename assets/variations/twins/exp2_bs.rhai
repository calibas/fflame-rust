/// Parameterized complex exponential — `e^(x·x1)` modulated by `sin(y·y1)`
/// and `cos(y·y2)`. At defaults (1.0), reduces to the unparameterized
/// complex exp.
///
/// # Authors
/// - cothe
/// - Brad Stefanov

// Exp2 BS, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    x1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "X1",
        hint: "Scales the X exponent — output uses `exp(x · x1)`.",
    },
    y1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Y1",
        hint: "Scales the argument of `sin(y)`.",
    },
    y2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Y2",
        hint: "Scales the argument of `cos(y)`.",
    },
};

const INFO = #{ title: "Exp2 BS", name: "rhai_exp2_bs", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let x1 = p.x1;
    let y1 = p.y1;
    let y2 = p.y2;
    let e = exp(z.re * x1);
    let s = sin(z.im * y1);
    let c = cos(z.im * y2);
    complex(e * c, e * s)
}
