// The built-in Tricorn, conj(z)^p + c, as a formula of your own (the
// Escape panel's Edit as Formula). At power 2 the step squares exactly;
// any other power takes its polar form. Deep zoom follows both.

const PARAMS = #{
    power: #{ value: 2.0, min: 2.0, max: 12.0, caption: "Power", hint: "Exponent p in conj(z)^p + c. 2 is the Tricorn; higher powers are the multicorns." },
    bailout: #{ value: 4.0, min: 1.0, caption: "Bailout", hint: "The squared radius past which a pixel has escaped" },
};

const INFO = #{ title: "Tricorn", center: complex(-0.3, 0.0), maxiter: 256 };

fn start(pixel, p, ctx) {
    if ctx.julia {
        #{ z: pixel, c: ctx.c }
    } else {
        #{ z: complex(0.0, 0.0), c: pixel }
    }
}

fn step(s, p) {
    let w = conj(s.z);
    if p.power == 2.0 {
        s.z = w * w + s.c;
    } else {
        s.z = w ** p.power + s.c;
    }
    s
}

fn escaped(s, p) {
    s.z.norm_sqr() > p.bailout
}

fn degree(p) {
    p.power
}
