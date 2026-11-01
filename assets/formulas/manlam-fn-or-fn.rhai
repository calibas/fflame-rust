// manlam(fn||fn), Fractint's: a function switched by the size of z,
//   z' = L fn(z) with fn1 while |z|^2 < Shift, fn2 from there,
// L the pixel, from z = pixel + Z0 (the Mandelbrot plane of
// lambda(fn||fn)). Ported from Iterated Dynamics (the GPL Fractint
// successor; libid/fractals/fn_or_fn.cpp and the per-pixel setup in
// engine/fractals.cpp, its defaults and bailout of 64 from fractalp.cpp).
// The function menus start at the program's defaults, sin and sqr; the
// switch compares |z|^2, as the program's cmplx_mod does. Fractint counts
// the start as its first iteration, so its counts run one above these.

const PARAMS = #{
    z0: #{ value: complex(0.0, 0.0), caption: "Z0", hint: "Added to the pixel for the start" },
    shift: #{ value: 10.0, caption: "Shift", hint: "The squared size of z at which fn1 gives way to fn2" },
    fn1: #{ function: "sin", caption: "fn1", hint: "The function while |z|^2 is under the Shift (the program's default, sin)" },
    fn2: #{ function: "sqr", caption: "fn2", hint: "The function from the Shift on (the program's default, sqr)" },
};

const INFO = #{ title: "manlam(fn||fn)", center: complex(0.0, 0.0), magn: 1.0, maxiter: 150 };

fn start(pixel, p, ctx) {
    #{ z: pixel + p.z0, k: pixel }
}

fn step(s, p) {
    let f = if s.z.norm_sqr() < p.shift { p.fn1(s.z) } else { p.fn2(s.z) };
    s.z = s.k * f;
    s
}

fn escaped(s, p) {
    s.z.norm_sqr() >= 64.0
}
