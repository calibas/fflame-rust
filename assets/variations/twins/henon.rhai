/// Classic Hénon strange attractor — iteration of `(c − a·x² + y, b·x)`.
/// Maps trajectories onto the famous Hénon curve.
///
/// # Authors
/// - TyrantWave

// Henon, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    a: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "A",
        hint: "Quadratic coefficient — controls the strength of the parabolic fold.",
    },
    b: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "B",
        hint: "Linear coefficient on X — scales the Y output.",
    },
    c: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "C",
        hint: "Constant offset added to X output.",
    },
};

const INFO = #{ title: "Henon", name: "rhai_henon", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let a = p.a;
    let b = p.b;
    let c = p.c;
    complex(c - a * z.re * z.re + z.im, b * z.re)
}
