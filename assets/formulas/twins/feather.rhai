// The built-in Feather, z^p / (1 + x^2 - i y^2) + c, as a formula of your
// own (the Escape panel's Edit as Formula). The denominator's real part
// is at least 1, so it never vanishes.

const PARAMS = #{
    power: #{ value: 3.0, min: 2.0, max: 8.0, caption: "Power", hint: "Exponent p in the numerator z^p. The classic feather uses 3." },
    bailout: #{ value: 4.0, min: 1.0, caption: "Bailout", hint: "The squared radius past which a pixel has escaped" },
};

const INFO = #{ title: "Feather", center: complex(-0.35, 0.0), maxiter: 256 };

fn start(pixel, p, ctx) {
    if ctx.julia {
        #{ z: pixel, c: ctx.c }
    } else {
        #{ z: complex(0.0, 0.0), c: pixel }
    }
}

fn step(s, p) {
    let x = s.z.re;
    let y = s.z.im;
    s.z = s.z ** p.power / complex(1.0 + x * x, -(y * y)) + s.c;
    s
}

fn escaped(s, p) {
    s.z.norm_sqr() > p.bailout
}

// At infinity z^p / z^2.
fn degree(p) {
    p.power - 2.0
}
