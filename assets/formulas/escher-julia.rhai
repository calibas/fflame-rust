// Escher-like Julia, Fractint's (after "The Science of Fractal Images",
// pages 185 and 187): the Julia map of c = 0, z' = z^2, with a second
// Julia set as a target. Each pass, 15 z is run through the Constant's
// Julia map, up to the iteration limit; a pixel whose point stays in
// that target set stops there, and one whose point leaves goes on, under
// the usual bailout. So the plain z^2 picture holds copies of the
// target set.
// Ported from Iterated Dynamics (the GPL Fractint successor;
// libid/fractals/escher.cpp, its defaults and view from fractalp.cpp).
// Fractint colours a pixel stopped in the target by 3n mod 255 + 1, n
// its count, for distinct bands; here the colouring sees the count.

const PARAMS = #{
    constant: #{ value: complex(0.32, 0.043), caption: "Constant", hint: "The target Julia set's c" },
};

const INFO = #{ title: "Escher-like Julia", center: complex(0.0, 0.0), magn: 1.6666666666666667, maxiter: 150 };

fn start(pixel, p, ctx) {
    #{ z: pixel, target: false }
}

fn step(s, p, ctx) {
    s.z = s.z * s.z;
    let t = s.z * 15.0;
    let size = 0.0;
    let k = 0;
    while size <= 4.0 && k < ctx.maxiter {
        t = t * t + p.constant;
        size = t.norm_sqr();
        k += 1;
    }
    s.target = size <= 4.0;
    s
}

fn escaped(s, p) {
    s.target || s.z.norm_sqr() >= 4.0
}
