// The built-in Nova, the Newton step for z^p - 1 plus c, as a formula
// of your own (the Escape panel's Edit as Formula). On the parameter
// plane it starts at the critical point 1. An orbit ends as it escapes
// or as it settles.

const PARAMS = #{
    power: #{ value: 3.0, min: 2.0, max: 12.0, caption: "Power", hint: "Power of the underlying z^p - 1." },
    relax_re: #{ value: 1.0, min: -3.0, max: 3.0, caption: "Relaxation (re)", hint: "Complex relaxation R multiplying the Newton step." },
    relax_im: #{ value: 0.0, min: -3.0, max: 3.0, caption: "Relaxation (im)", hint: "Imaginary part of the relaxation." },
    bailout: #{ value: 10000.0, min: 1.0, caption: "Bailout", hint: "The squared radius past which a pixel has escaped" },
};

const INFO = #{ title: "Nova", center: complex(-0.3, 0.0), maxiter: 128 };

fn start(pixel, p, ctx) {
    if ctx.julia {
        #{ z: pixel, c: ctx.c, prev: pixel }
    } else {
        #{ z: complex(1.0, 0.0), c: pixel, prev: complex(1.0, 0.0) }
    }
}

// The built-in's division: a large value at a pole, which escapes.
fn cdiv(a, b) {
    if b.norm_sqr() < 1e-30 { complex(1e20, 0.0) } else { a / b }
}

fn step(s, p) {
    s.prev = s.z;
    let f = s.z ** p.power - 1.0;
    let fp = p.power * s.z ** (p.power - 1.0);
    s.z = s.z - complex(p.relax_re, p.relax_im) * cdiv(f, fp) + s.c;
    s
}

fn escaped(s, p) {
    (s.z - s.prev).norm_sqr() < 1e-12 || s.z.norm_sqr() > p.bailout
}
