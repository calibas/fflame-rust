// DivideBrot5, Jim Muth's: z^2 divided by a power of z, plus c,
//   z' = z^2 / (z^e + b) + c,   e = 2 - trunc(a),
// from z = 0. Ported from Iterated Dynamics (the GPL Fractint successor;
// libid/fractals/divide_brot.cpp, its defaults, view and bailout of 16
// from fractalp.cpp). The program's comment writes z^a, but its code
// raises z to 2 - a, a cut to a whole number: this does as the code
// does, so at the default a = 2 the divisor is 1 and the set is the
// Mandelbrot's. Fractint's log of 0 is 0, so z^e is 1 at z = 0 (the
// first pass); a tiny 1e-20 joins b, as there.

const PARAMS = #{
    a: #{ value: 2.0, caption: "a", hint: "The power, as 2 - a cut to a whole number, that z^2 is divided by" },
    b: #{ value: 0.0, caption: "b", hint: "Added to the power before dividing" },
};

const INFO = #{ title: "DivideBrot5", center: complex(-0.5, 0.0), magn: 1.3333333333333333, maxiter: 150 };

fn start(pixel, p, ctx) {
    #{ z: complex(0.0, 0.0), c: pixel }
}

fn step(s, p) {
    let e = 2.0 - trunc(p.a);
    let lz = if s.z.norm_sqr() == 0.0 { complex(0.0, 0.0) } else { ln(s.z) };
    let d = exp(lz * e) + complex(p.b + 1.0e-20, 0.0);
    s.z = s.z * s.z / d + s.c;
    s
}

fn escaped(s, p) {
    s.z.norm_sqr() >= 16.0
}
