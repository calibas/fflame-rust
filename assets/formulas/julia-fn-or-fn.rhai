// julia(fn||fn), Fractint's: a function switched by the size of z,
//   z' = fn(z) + c with fn1 while |z|^2 < Shift, fn2 from there,
// from z = pixel, c the Constant. Ported from Iterated Dynamics (the GPL
// Fractint successor; libid/fractals/fn_or_fn.cpp and the per-pixel setup
// in engine/fractals.cpp, its defaults and bailout of 64 from
// fractalp.cpp). The function menus start at the program's defaults, sin
// and sqr; the switch compares |z|^2, as the program's cmplx_mod does.

const PARAMS = #{
    constant: #{ value: complex(0.0, 0.0), caption: "Constant", hint: "c, the constant added" },
    shift: #{ value: 8.0, caption: "Shift", hint: "The squared size of z at which fn1 gives way to fn2" },
    fn1: #{ function: "sin", caption: "fn1", hint: "The function while |z|^2 is under the Shift (the program's default, sin)" },
    fn2: #{ function: "sqr", caption: "fn2", hint: "The function from the Shift on (the program's default, sqr)" },
};

const INFO = #{ title: "julia(fn||fn)", center: complex(0.0, 0.0), magn: 1.0, maxiter: 150 };

fn start(pixel, p, ctx) {
    #{ z: pixel, k: p.constant }
}

fn step(s, p) {
    let f = if s.z.norm_sqr() < p.shift { p.fn1(s.z) } else { p.fn2(s.z) };
    s.z = f + s.k;
    s
}

fn escaped(s, p) {
    s.z.norm_sqr() >= 64.0
}
