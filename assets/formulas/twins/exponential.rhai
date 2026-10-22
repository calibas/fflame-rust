// The built-in Exponential, e^z + c, as a formula of your own (the
// Escape panel's Edit as Formula). It escapes when Re z passes the
// bailout -- |e^z| = e^(Re z) -- so the bailout is near 50, not 4.

const PARAMS = #{
    bailout: #{ value: 50.0, min: 1.0, caption: "Bailout", hint: "The real part past which a pixel has escaped" },
};

const INFO = #{ title: "Exponential", center: complex(-1.0, 0.0), maxiter: 128 };

fn start(pixel, p, ctx) {
    if ctx.julia {
        #{ z: pixel, c: ctx.c }
    } else {
        #{ z: complex(0.0, 0.0), c: pixel }
    }
}

fn step(s, p) {
    // The built-in's guard: the real part held at 80 before exp, which
    // a bailout under 80 never reaches.
    let x = if s.z.re > 80.0 { 80.0 } else { s.z.re };
    s.z = exp(complex(x, s.z.im)) + s.c;
    s
}

fn escaped(s, p) {
    s.z.re > p.bailout
}
