// The built-in Mandelbrot, as a formula of your own. The Escape panel's
// Edit as Formula opens it here with the picture as it was; change
// anything, and with Live on the picture follows. The panel's Bailout
// and Julia toggle still drive it.

const PARAMS = #{
    bailout: #{ value: 4.0, min: 1.0, caption: "Bailout", hint: "The squared radius past which a pixel has escaped" },
};

const INFO = #{ title: "Mandelbrot", center: complex(0.0, 0.0), maxiter: 512 };

fn start(pixel, p, ctx) {
    // On the Julia plane the pixel starts the orbit and c is the
    // panel's constant; on the parameter plane z starts at 0.
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

// How fast it grows past the bailout, |z| to the power 2: what the
// smooth colourings divide by.
fn degree(p) {
    2.0
}
