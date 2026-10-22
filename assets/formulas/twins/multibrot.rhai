// The built-in Multibrot, z^p + c, as a formula of your own (the Escape
// panel's Edit as Formula). Any power renders and deep-zooms.

const PARAMS = #{
    power: #{ value: 3.0, min: 2.0, max: 12.0, caption: "Power", hint: "Exponent p in z^p + c. 2 is the Mandelbrot set; higher powers grow more lobes." },
    bailout: #{ value: 4.0, min: 1.0, caption: "Bailout", hint: "The squared radius past which a pixel has escaped" },
};

const INFO = #{ title: "Multibrot", center: complex(0.0, 0.0), maxiter: 256 };

fn start(pixel, p, ctx) {
    if ctx.julia {
        #{ z: pixel, c: ctx.c }
    } else {
        #{ z: complex(0.0, 0.0), c: pixel }
    }
}

fn step(s, p) {
    s.z = s.z ** p.power + s.c;
    s
}

fn escaped(s, p) {
    s.z.norm_sqr() > p.bailout
}

fn degree(p) {
    p.power
}
