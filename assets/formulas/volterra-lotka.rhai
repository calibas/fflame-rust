// Volterra-Lotka, Fractint's type after The Beauty of Fractals (Peitgen
// and Richter, pp. 125-127): the predator-prey equations x' = x - xy,
// y' = -y + xy, stepped by a two-stage scheme of step h and weight p.
// Ported from Xfractint 20.04p16 (common/fractals.c, VLfpFractal; VLSetup
// in common/frasetup.c clamps both parameters to 0..1; the table entry in
// common/fractalp.c gives the defaults, the view and the bailout).
//
// The pixel is the starting population (x, y); the count of passes until
// it leaves the bailout's circle is the picture.

const PARAMS = #{
    h: #{ value: 0.739, min: 0.0, max: 1.0, caption: "h", hint: "The step (Fractint's first parameter), 0 to 1" },
    p: #{ value: 0.739, min: 0.0, max: 1.0, caption: "p", hint: "The scheme's weight (Fractint's second parameter), 0 to 1" },
    bailout: #{ value: 256.0, min: 4.0, caption: "Bailout", hint: "The squared radius past which a pixel has escaped (Fractint's 256)" },
};

const INFO = #{ title: "Volterra-Lotka", center: complex(3.0, 2.25), magn: 0.8888888888888888, maxiter: 150 };

fn start(pixel, p, ctx) {
    #{ z: pixel }
}

fn step(s, p) {
    // VLSetup's clamps.
    let h = if p.h < 0.0 { 0.0 } else if p.h > 1.0 { 1.0 } else { p.h };
    let k = if p.p < 0.0 { 0.0 } else if p.p > 1.0 { 1.0 } else { p.p };
    let half = h / 2.0;
    let x = s.z.re;
    let y = s.z.im;
    let xy = x * y;
    let u = x - xy;
    let w = -y + xy;
    let a = x + k * u;
    let b = y + k * w;
    let ab = a * b;
    s.z = complex(x + half * (u + (a - ab)), y + half * (w + (-b + ab)));
    s
}

fn escaped(s, p) {
    s.z.norm_sqr() > p.bailout
}
