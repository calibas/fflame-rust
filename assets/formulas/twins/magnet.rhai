// The built-in Magnet I and II, rational maps from statistical
// mechanics, as a formula of your own (the Escape panel's Edit as
// Formula). An orbit ends as it escapes or as it settles (on the fixed
// point 1): the root colourings read which.

const PARAMS = #{
    variant: #{
        value: 0,
        caption: "Variant",
        hint: "0: Magnet I, 1: Magnet II.",
        choices: ["Magnet I", "Magnet II"],
    },
    bailout: #{ value: 10000.0, min: 1.0, caption: "Bailout", hint: "The squared radius past which a pixel has escaped" },
};

const INFO = #{ title: "Magnet", center: complex(1.0, 0.0), maxiter: 128 };

fn start(pixel, p, ctx) {
    if ctx.julia {
        #{ z: pixel, c: ctx.c, prev: pixel }
    } else {
        #{ z: complex(0.0, 0.0), c: pixel, prev: complex(0.0, 0.0) }
    }
}

// The built-in's division: a large value at a pole, which escapes.
fn cdiv(a, b) {
    if b.norm_sqr() < 1e-30 { complex(1e20, 0.0) } else { a / b }
}

fn step(s, p) {
    s.prev = s.z;
    let q = if p.variant == 1 {
        // ((z^3 + 3(c-1)z + (c-1)(c-2)) / (3z^2 + 3(c-2)z + (c-1)(c-2) + 1))^2
        let cm1 = s.c - 1.0;
        let cm2 = s.c - 2.0;
        let c12 = cm1 * cm2;
        let z2 = s.z * s.z;
        cdiv(z2 * s.z + 3.0 * (cm1 * s.z) + c12, 3.0 * z2 + 3.0 * (cm2 * s.z) + c12 + 1.0)
    } else {
        // ((z^2 + c - 1) / (2z + c - 2))^2
        cdiv(s.z * s.z + s.c - 1.0, 2.0 * s.z + s.c - 2.0)
    };
    s.z = q * q;
    s
}

fn escaped(s, p) {
    (s.z - s.prev).norm_sqr() < 1e-12 || s.z.norm_sqr() > p.bailout
}
