/// Johnny Svensson attractor — iterates `(d·sin(a·x) − sin(b·y), c·cos(a·x)
/// + cos(b·y))`. 4-parameter strange attractor from Paul Bourke's
/// collection.
///
/// # Authors
/// - Jesus Sosa

// Svensson (JS), the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    a: #{
        value: 1.4, min: -10.0, max: 10.0, free: true,
        caption: "A",
        hint: "Svensson attractor parameter `a` (X-axis sin/cos frequency).",
    },
    b: #{
        value: 1.56, min: -10.0, max: 10.0, free: true,
        caption: "B",
        hint: "Svensson attractor parameter `b` (Y-axis sin/cos frequency).",
    },
    c: #{
        value: 1.4, min: -10.0, max: 10.0, free: true,
        caption: "C",
        hint: "Svensson attractor parameter `c` — cosine amplitude on Y output.",
    },
    d: #{
        value: -6.56, min: -10.0, max: 10.0, free: true,
        caption: "D",
        hint: "Svensson attractor parameter `d` — sine amplitude on X output.",
    },
};

const INFO = #{ title: "Svensson (JS)", name: "rhai_svensson_js", category: "advanced_2d", phase: "any", replace: true };

fn variation(z, p) {
    let a = p.a;
    let b = p.b;
    let c = p.c;
    let d = p.d;
    let x = d * sin(a * z.re) - sin(b * z.im);
    let y = c * cos(a * z.re) + cos(b * z.im);
    complex(x, y)
}
