// The Mandelbrot set, as a Rhai formula.
//
// A formula is three functions. `start` returns the state each pixel
// begins with -- here the iterate `z` and `c` -- and `step` returns it
// one pass on. `escaped` says when a pixel has left. `PARAMS` are the
// controls the panel shows, which animation and scripts can also set.

const PARAMS = #{
    bailout: #{ value: 4.0, min: 1.0, caption: "Bailout", hint: "The squared radius past which a pixel has escaped" },
};

const INFO = #{ title: "Mandelbrot", center: complex(-0.5, 0.0), maxiter: 500 };

fn start(pixel, p, ctx) {
    // The panel's Julia toggle: on the Julia plane the pixel starts the
    // orbit, and c is the Julia constant.
    if ctx.julia {
        #{ z: pixel, c: ctx.c }
    } else {
        #{ z: complex(0.0, 0.0), c: pixel }
    }
}

fn step(s, p) {
    s.z = s.z * s.z + s.c;
    s
}

fn escaped(s, p) {
    s.z.norm_sqr() > p.bailout
}
