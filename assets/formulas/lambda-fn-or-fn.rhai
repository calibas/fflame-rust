// lambda(fn||fn), Fractint's: a function switched by the size of z,
//   z' = L fn(z) with fn1 while |z|^2 < Shift, fn2 from there,
// from z = pixel, L the Lambda. Ported from Iterated Dynamics (the GPL
// Fractint successor; libid/fractals/fn_or_fn.cpp and the per-pixel setup
// in engine/fractals.cpp, its defaults and bailout of 64 from
// fractalp.cpp). The function menus start at the program's defaults, sin
// and sqr; the switch compares |z|^2, as the program's cmplx_mod does.

const PARAMS = #{
    lambda: #{ value: complex(1.0, 0.1), caption: "Lambda", hint: "L, the multiplier" },
    shift: #{ value: 1.0, caption: "Shift", hint: "The squared size of z at which fn1 gives way to fn2" },
    fn1: #{ function: "sin", caption: "fn1", hint: "The function while |z|^2 is under the Shift (the program's default, sin)" },
    fn2: #{ function: "sqr", caption: "fn2", hint: "The function from the Shift on (the program's default, sqr)" },
};

const INFO = #{ title: "lambda(fn||fn)", center: complex(0.0, 0.0), magn: 1.0, maxiter: 150 };

fn start(pixel, p, ctx) {
    #{ z: pixel, k: p.lambda }
}

fn step(s, p) {
    let f = if s.z.norm_sqr() < p.shift { p.fn1(s.z) } else { p.fn2(s.z) };
    s.z = s.k * f;
    s
}

fn escaped(s, p) {
    s.z.norm_sqr() >= 64.0
}
