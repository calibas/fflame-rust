/// Conformal log-spiral mapping inspired by M. C. Escher's prints. Tunes
/// between pure scaling and pure rotation via the beta angle.
///
/// # Authors
/// - Apophysis Plugin Pack

// Escher, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    beta: #{
        value: 0.0, min: -180.0, max: 180.0, type: "angle",
        caption: "Beta",
        hint: "Balance between scaling and rotation. At 0 degrees the map is pure scaling; near +/-90 degrees it's pure rotation. Sweep this to get spiraling effects.",
    },
};

const INFO = #{ title: "Escher", name: "rhai_escher", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let beta = p.beta * 3.14159265358979 / 180.0;
    #{ c: 0.5 * (1.0 + cos(beta)), d: 0.5 * sin(beta) }
}

fn variation(z, p) {
    let c = p.c;
    let d = p.d;
    let a = atan2(z.im, z.re);
    let lnr = 0.5 * ln(z.norm_sqr());
    let m = exp(c * lnr - d * a);
    let angle = c * a + d * lnr;
    complex(m * cos(angle), m * sin(angle))
}
