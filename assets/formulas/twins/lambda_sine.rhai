// The built-in Lambda Sine, lambda sin z, as a formula of your own (the
// Escape panel's Edit as Formula). On the parameter plane the orbit
// starts at sin's critical point, pi/2; it escapes when |Im z| passes
// the bailout.

const PARAMS = #{
    bailout: #{ value: 50.0, min: 1.0, caption: "Bailout", hint: "The imaginary part's size past which a pixel has escaped" },
};

const INFO = #{ title: "Lambda Sine (λ·sin z)", center: complex(0.0, 0.0), maxiter: 300 };

fn start(pixel, p, ctx) {
    if ctx.julia {
        #{ z: pixel, c: ctx.c }
    } else {
        #{ z: complex(PI() / 2.0, 0.0), c: pixel }
    }
}

fn step(s, p) {
    s.z = s.c * sin(s.z);
    s
}

fn escaped(s, p) {
    s.z.im.abs() > p.bailout
}
