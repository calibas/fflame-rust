// A Mandelbrot bent each step by a variation.
//
// `vary("name", z)` applies a variation you wrote in Rhai -- here the
// Whorl example, variation-whorl.rhai -- to a point. Its parameters are
// given in a map, `#{ inside: p.turn }`, or left at their defaults. The
// config carries the variation's source beside the formula, and deep
// zoom works through it as through the rest of the step.

const PARAMS = #{
    bailout: #{ value: 4.0, min: 1.0, caption: "Bailout", hint: "The squared radius past which a pixel has escaped" },
    turn: #{ value: 0.6, min: -3.0, max: 3.0, caption: "Turn", hint: "How far the whorl turns points inside the unit circle" },
};

const INFO = #{ title: "Whorled Mandelbrot", center: complex(-0.1, 0.0), maxiter: 500 };

fn start(pixel, p, ctx) {
    if ctx.julia {
        #{ z: pixel, c: ctx.c }
    } else {
        #{ z: complex(0.0, 0.0), c: pixel }
    }
}

fn step(s, p) {
    s.z = vary("rhai_whorl", s.z * s.z + s.c, #{ inside: p.turn });
    s
}

fn escaped(s, p) {
    s.z.norm_sqr() > p.bailout
}
