// The Mandelbrot set: the formula language's plainest formula.

const PARAMS = #{
    bailout: #{ value: 4.0, min: 1.0, caption: "Bailout", hint: "The squared radius past which a point has escaped" },
};

const INFO = #{ title: "Mandelbrot", center: complex(-0.5, 0.0), magn: 1.0, maxiter: 500 };

fn start(pixel, p) {
    #{ z: complex(0.0, 0.0), c: pixel }
}

fn step(s, p) {
    s.z = s.z * s.z + s.c;
    s
}

fn escaped(s, p) {
    s.z.norm_sqr() > p.bailout
}
