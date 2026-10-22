// The built-in Tetration, z <- c^z, as a formula of your own (the Escape
// panel's Edit as Formula). On the parameter plane the orbit starts at
// c; it escapes when Re z passes the bailout. Written e^(z ln c), which
// deep zoom derives (a power whose exponent varies has no form).

const PARAMS = #{
    bailout: #{ value: 50.0, min: 1.0, caption: "Bailout", hint: "The real part past which a pixel has escaped" },
};

const INFO = #{ title: "Tetration", center: complex(0.0, 0.0), maxiter: 128 };

fn start(pixel, p, ctx) {
    if ctx.julia {
        #{ z: pixel, c: ctx.c }
    } else {
        #{ z: pixel, c: pixel }
    }
}

fn step(s, p) {
    let w = s.z * ln(s.c);
    // The built-in's guard: the real part held at 80 before exp.
    let x = if w.re > 80.0 { 80.0 } else { w.re };
    s.z = exp(complex(x, w.im));
    s
}

fn escaped(s, p) {
    s.z.re > p.bailout
}
