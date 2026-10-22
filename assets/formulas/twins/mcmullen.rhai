// The built-in McMullen, z^n + c / z^m, as a formula of your own (the
// Escape panel's Edit as Formula). n = 2, m = 3 is the classic
// Sierpinski-carpet family; the orbit starts at the pixel on the
// parameter plane.

const PARAMS = #{
    n: #{ value: 2.0, min: 2.0, max: 8.0, caption: "n (numerator power)", hint: "Power of the polynomial term z^n." },
    m: #{ value: 3.0, min: 1.0, max: 8.0, caption: "m (pole power)", hint: "Power of the pole term c / z^m. n=2, m=3 is the classic Sierpinski-carpet family." },
    bailout: #{ value: 4.0, min: 1.0, caption: "Bailout", hint: "The squared radius past which a pixel has escaped" },
};

const INFO = #{ title: "McMullen", center: complex(0.0, 0.0), maxiter: 128 };

fn start(pixel, p, ctx) {
    if ctx.julia {
        #{ z: pixel, c: ctx.c }
    } else {
        #{ z: pixel, c: pixel }
    }
}

fn step(s, p) {
    // The built-in's guard at the pole: a value that escapes next.
    s.z = if s.z.norm_sqr() < 1e-20 {
        complex(1e20, 0.0)
    } else {
        s.z ** p.n + s.c * s.z ** (-p.m)
    };
    s
}

fn escaped(s, p) {
    s.z.norm_sqr() > p.bailout
}

fn degree(p) {
    p.n
}
