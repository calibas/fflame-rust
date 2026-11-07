/// Lozi strange attractor — `(c − a·|x| + y, b·x)` iteration. Like Hénon
/// but with `|x|` instead of `x²`.
///
/// # Authors
/// - TyrantWave

// Lozi, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    a: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "A",
        hint: "Coefficient on `|x|` — controls the fold strength.",
    },
    b: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "B",
        hint: "Coefficient scaling X to the Y output.",
    },
    c: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "C",
        hint: "Constant offset added to X output.",
    },
};

const INFO = #{ title: "Lozi", name: "rhai_lozi", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let a = p.a;
    let b = p.b;
    let c = p.c;
    complex(c - a * abs(z.re) + z.im, b * z.re)
}
