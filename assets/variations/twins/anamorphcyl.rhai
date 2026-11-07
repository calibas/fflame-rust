/// Anamorphic cylinder warp — wraps the input around a cylinder. Outputs
/// `(f·cos(k·x), f·sin(k·x))` where `f = a·(y + b)`. The X coordinate
/// drives the angular position (× frequency `k`), and Y drives the radial
/// position (offset by `b`, scaled by `a`).
///
/// # Authors
/// - Jesus Sosa

// Anamorph Cyl, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    a: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "A",
        hint: "Radial scale (multiplies the entire output magnitude).",
    },
    b: #{
        value: 1.3, min: -10.0, max: 10.0, free: true,
        caption: "B",
        hint: "Y offset added before the radial multiplication.",
    },
    k: #{
        value: 3.0, min: -50.0, max: 50.0, free: true,
        caption: "K",
        hint: "Angular frequency of the X coordinate.",
    },
};

const INFO = #{ title: "Anamorph Cyl", name: "rhai_anamorphcyl", category: "advanced_2d", phase: "any", replace: true };

fn variation(z, p) {
    let a = p.a;
    let b = p.b;
    let k = p.k;
    let f = a * (z.im + b);
    complex(f * cos(k * z.re), f * sin(k * z.re))
}
