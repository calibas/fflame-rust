/// Clifford strange attractor — iterates `(sin(a·y) + c·cos(a·x), sin(b·x)
/// + d·cos(b·y))`. Classic 4-parameter chaotic attractor with
/// characteristic Lissajous-like curves and folds. From Paul Bourke's
/// collection.
///
/// # Authors
/// - Jesus Sosa

// Clifford (JS), the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    a: #{
        value: -1.4, min: -10.0, max: 10.0, free: true,
        caption: "A",
        hint: "Clifford attractor parameter `a` (in `sin(a·y) + c·cos(a·x)`).",
    },
    b: #{
        value: 1.6, min: -10.0, max: 10.0, free: true,
        caption: "B",
        hint: "Clifford attractor parameter `b` (in `sin(b·x) + d·cos(b·y)`).",
    },
    c: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "C",
        hint: "Clifford attractor parameter `c` — cosine coefficient on the X output.",
    },
    d: #{
        value: 0.7, min: -10.0, max: 10.0, free: true,
        caption: "D",
        hint: "Clifford attractor parameter `d` — cosine coefficient on the Y output.",
    },
};

const INFO = #{ title: "Clifford (JS)", name: "rhai_clifford_js", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let a = p.a;
    let b = p.b;
    let c = p.c;
    let d = p.d;
    let x = sin(a * z.im) + c * cos(a * z.re);
    let y = sin(b * z.re) + d * cos(b * z.im);
    complex(x, y)
}
