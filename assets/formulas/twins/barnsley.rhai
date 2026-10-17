// The built-in Barnsley, as a formula of your own (the Escape panel's
// Edit as Formula): Barnsley's three maps, each choosing its step by a
// test on the iterate.

const PARAMS = #{
    variant: #{
        value: 0,
        caption: "Variant",
        hint: "0: M1 (fold on Re z), 1: M2 (fold on a bilinear test), 2: M3 (quadratic with conditional c term).",
        choices: ["M1 (fold on Re z)", "M2 (bilinear test)", "M3 (quadratic)"],
    },
    bailout: #{ value: 4.0, min: 1.0, caption: "Bailout", hint: "The squared radius past which a pixel has escaped" },
};

const INFO = #{ title: "Barnsley", center: complex(0.3, 0.0), maxiter: 128 };

fn start(pixel, p, ctx) {
    if ctx.julia {
        #{ z: pixel, c: ctx.c }
    } else {
        #{ z: pixel, c: pixel }
    }
}

fn step(s, p) {
    let z = s.z;
    let c = s.c;
    s.z = if p.variant == 0 {
        // M1: the side of Re z.
        if z.re >= 0.0 { (z - 1.0) * c } else { (z + 1.0) * c }
    } else if p.variant == 1 {
        // M2: the side of Im(z c).
        if z.re * c.im + c.re * z.im >= 0.0 { (z - 1.0) * c } else { (z + 1.0) * c }
    } else {
        // M3: z^2 - 1, plus c Re z on the left.
        let sq = complex(z.re * z.re - z.im * z.im - 1.0, 2.0 * z.re * z.im);
        if z.re > 0.0 { sq } else { sq + c * z.re }
    };
    s
}

fn escaped(s, p) {
    s.z.norm_sqr() > p.bailout
}
